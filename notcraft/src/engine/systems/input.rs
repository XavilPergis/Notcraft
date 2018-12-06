use crate::engine::{
    camera::Camera,
    prelude::*,
    render::debug::{DebugAccumulator, Shape},
};
use cgmath::Deg;
use glium::glutin::{
    DeviceEvent, ElementState, Event, GlWindow, KeyboardInput, ModifiersState, VirtualKeyCode,
    WindowEvent,
};
use shrev::EventChannel;
use std::collections::{HashMap, HashSet};

pub struct InputHandler {
    events_handle: ReaderId<Event>,
    wireframe: bool,
    capture_mouse: bool,
}

impl InputHandler {
    pub fn new(event_channel: &mut EventChannel<Event>) -> Self {
        InputHandler {
            events_handle: event_channel.register_reader(),
            wireframe: false,
            capture_mouse: false,
        }
    }
}

pub enum Key {
    Physical(u32),
    Virtual(VirtualKeyCode),
}

impl From<VirtualKeyCode> for Key {
    fn from(vkk: VirtualKeyCode) -> Self {
        Key::Virtual(vkk)
    }
}

impl From<u32> for Key {
    fn from(sc: u32) -> Self {
        Key::Physical(sc)
    }
}

pub struct Keybind {
    key: Key,
    modifiers: Option<ModifiersState>,
}

impl Keybind {
    pub fn new<K: Into<Key>>(key: K, modifiers: Option<ModifiersState>) -> Self {
        Keybind {
            key: key.into(),
            modifiers,
        }
    }

    pub fn matches_input(&self, input: KeyboardInput) -> bool {
        self.modifiers
            .map_or(true, |modifiers| input.modifiers == modifiers)
            && match self.key {
                Key::Virtual(vkk) => input.virtual_keycode.map_or(false, |vkk_in| vkk_in == vkk),
                Key::Physical(scancode) => input.scancode == scancode,
            }
    }
}

const NO_MODIFIERS: ModifiersState = ModifiersState {
    shift: false,
    ctrl: false,
    alt: false,
    logo: false,
};

const CTRL_MODIFIERS: ModifiersState = ModifiersState {
    shift: false,
    ctrl: true,
    alt: false,
    logo: false,
};

const KEYBIND_FORWARDS: Keybind = Keybind {
    key: Key::Physical(0x11),
    modifiers: None,
};
const KEYBIND_BACKWARDS: Keybind = Keybind {
    key: Key::Physical(0x1F),
    modifiers: None,
};
const KEYBIND_LEFT: Keybind = Keybind {
    key: Key::Physical(0x1E),
    modifiers: None,
};
const KEYBIND_RIGHT: Keybind = Keybind {
    key: Key::Physical(0x20),
    modifiers: None,
};
const KEYBIND_UP: Keybind = Keybind {
    key: Key::Physical(0x39),
    modifiers: None,
};
const KEYBIND_DOWN: Keybind = Keybind {
    key: Key::Physical(0x2A),
    modifiers: None,
};
const KEYBIND_ZOOM: Keybind = Keybind {
    key: Key::Physical(0x2E),
    modifiers: Some(NO_MODIFIERS),
};

const KEYBIND_EXIT: Keybind = Keybind {
    key: Key::Virtual(VirtualKeyCode::Escape),
    modifiers: Some(NO_MODIFIERS),
};
const KEYBIND_DEBUG: Keybind = Keybind {
    key: Key::Virtual(VirtualKeyCode::B),
    modifiers: Some(CTRL_MODIFIERS),
};
const KEYBIND_TOGGLE_WIREFRAME: Keybind = Keybind {
    key: Key::Virtual(VirtualKeyCode::F),
    modifiers: Some(CTRL_MODIFIERS),
};
const KEYBIND_INC_RENDER_DISTANCE: Keybind = Keybind {
    key: Key::Virtual(VirtualKeyCode::RBracket),
    modifiers: Some(CTRL_MODIFIERS),
};
const KEYBIND_DEC_RENDER_DISTANCE: Keybind = Keybind {
    key: Key::Virtual(VirtualKeyCode::LBracket),
    modifiers: Some(CTRL_MODIFIERS),
};

use crate::engine::components as comp;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct Modifiers {
    shift: bool,
    ctrl: bool,
    alt: bool,
    sup: bool,
}

impl From<ModifiersState> for Modifiers {
    fn from(state: ModifiersState) -> Self {
        Modifiers {
            shift: state.shift,
            ctrl: state.ctrl,
            alt: state.alt,
            sup: state.logo,
        }
    }
}

use std::ops;

impl ops::BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, rhs: Self) -> Self {
        Modifiers {
            shift: self.shift | rhs.shift,
            ctrl: self.ctrl | rhs.ctrl,
            alt: self.alt | rhs.alt,
            sup: self.sup | rhs.sup,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    physical_map: HashMap<VirtualKeyCode, u32>,
    rising: HashSet<u32>,
    falling: HashSet<u32>,
    pressed: HashSet<u32>,
    current_modifiers: Modifiers,
}

impl InputState {
    fn update(&mut self, input: KeyboardInput) {
        self.current_modifiers = input.modifiers.into();

        if let Some(vkk) = input.virtual_keycode {
            self.physical_map.insert(vkk, input.scancode);
        }

        match input.state {
            ElementState::Pressed => {
                if self.pressed.insert(input.scancode) {
                    self.rising.insert(input.scancode);
                }
            }

            ElementState::Released => {
                if self.pressed.remove(&input.scancode) {
                    self.falling.insert(input.scancode);
                }
            }
        }
    }

    fn modifiers_match(&self, modifiers: Option<Modifiers>) -> bool {
        modifiers.map_or(true, |modifiers| self.current_modifiers == modifiers)
    }

    /// Returns true if `key` is being pressed.
    pub fn is_pressed<K, M>(&self, key: K, modifiers: M) -> bool
    where
        K: Into<Key>,
        M: Into<Option<Modifiers>>,
    {
        let key = match key.into() {
            // Try to look up the virtual key code in the key map. If the entry does not exist,
            // then either the vkk was not pressed yet or the vkk -> physical
            // mapping just doesn't exist (which I find very unlikely). In both
            // situations, though, we want to say that the key in not pressed
            // (return false).
            Key::Virtual(vkk) => self
                .physical_map
                .get(&vkk)
                .map(|code| self.pressed.contains(&code))
                .unwrap_or(false),
            Key::Physical(code) => self.pressed.contains(&code),
        };

        key && self.modifiers_match(modifiers.into())
    }

    /// Returns true if `key` was not pressed before and is now pressed.
    pub fn is_rising<K, M>(&self, key: K, modifiers: M) -> bool
    where
        K: Into<Key>,
        M: Into<Option<Modifiers>>,
    {
        let key = match key.into() {
            Key::Virtual(vkk) => self
                .physical_map
                .get(&vkk)
                .map(|code| self.rising.contains(&code))
                .unwrap_or(false),
            Key::Physical(code) => self.rising.contains(&code),
        };

        key && self.modifiers_match(modifiers.into())
    }

    /// Returns true if `key` was pressed before and is now no longer pressed.
    pub fn is_falling<K, M>(&self, key: K, modifiers: M) -> bool
    where
        K: Into<Key>,
        M: Into<Option<Modifiers>>,
    {
        let key = match key.into() {
            Key::Virtual(vkk) => self
                .physical_map
                .get(&vkk)
                .map(|code| self.falling.contains(&code))
                .unwrap_or(false),
            Key::Physical(code) => self.falling.contains(&code),
        };

        key && self.modifiers_match(modifiers.into())
    }
}

pub mod keys {
    pub const FORWARD: u32 = 0x11;
    pub const BACKWARD: u32 = 0x1F;
    pub const LEFT: u32 = 0x1E;
    pub const RIGHT: u32 = 0x20;
    pub const UP: u32 = 0x39;
    pub const DOWN: u32 = 0x2A;
}

// (VirtualKeyCode::C, "chunk grid"),
// (VirtualKeyCode::T, "terrain generation"),
// (VirtualKeyCode::M, "mesher"),
// (VirtualKeyCode::P, "physics"),
// (VirtualKeyCode::I, "interaction"),

impl<'a> System<'a> for InputHandler {
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        Write<'a, res::StopGameLoop>,
        Write<'a, InputState>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (window_events, mut stop_flag, mut input_state, mut debug): Self::SystemData,
    ) {
        for event in window_events.read(&mut self.events_handle) {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        stop_flag.0 = true;
                    }

                    WindowEvent::KeyboardInput { input, .. } => input_state.update(*input),

                    _ => (),
                },
                _ => (),
            }
        }
    }
}

pub struct BlockInteraction {
    reader: ReaderId<Event>,
}

impl BlockInteraction {
    pub fn new(events: &mut EventChannel<Event>) -> Self {
        BlockInteraction {
            reader: events.register_reader(),
        }
    }
}

impl<'a> System<'a> for BlockInteraction {
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        WriteExpect<'a, VoxelWorld>,
        ReadExpect<'a, Camera>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(&mut self, (events, mut world, camera, debug): Self::SystemData) {
        let mut section = debug.section("interaction");
        let ray = camera.camera_ray();
        section.draw(Shape::Ray(10.0, ray, Vector4::new(1.0, 0.0, 0.0, 1.0)));
        if let Some((block, normal)) = world.trace_block(camera.camera_ray(), 10.0, &mut section) {
            section.draw(Shape::Block(3.0, block, Vector4::new(1.0, 1.0, 1.0, 1.0)));
        }

        for event in events.read(&mut self.reader) {
            match event {
                &Event::DeviceEvent {
                    event:
                        DeviceEvent::Button {
                            button,
                            state: ElementState::Pressed,
                        },
                    ..
                } => match button {
                    1 => {
                        if let Some((block, _)) =
                            world.trace_block(camera.camera_ray(), 10.0, &mut section)
                        {
                            world.set_block_id(block, crate::engine::world::block::AIR);
                        }
                    }
                    3 => {
                        if let Some((block, Some(normal))) =
                            world.trace_block(camera.camera_ray(), 10.0, &mut section)
                        {
                            world.set_block_id(
                                block.offset(normal),
                                crate::engine::world::block::STONE,
                            );
                        }
                    }
                    _ => {}
                },

                _ => (),
            }
        }
    }
}

pub struct CameraRotationUpdater {
    reader: ReaderId<Event>,
}

impl CameraRotationUpdater {
    pub fn new(events: &mut EventChannel<Event>) -> Self {
        CameraRotationUpdater {
            reader: events.register_reader(),
        }
    }
}

impl<'a> System<'a> for CameraRotationUpdater {
    type SystemData = (Read<'a, EventChannel<Event>>, WriteExpect<'a, Camera>);

    fn run(&mut self, (events, mut camera): Self::SystemData) {
        for event in events.read(&mut self.reader) {
            match event {
                &Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta: (dx, dy) },
                    ..
                } => {
                    let sensitivity = 0.25;

                    let dx = sensitivity * dx as f32;
                    let dy = sensitivity * dy as f32;
                    // Ok, I know this looks weird, but `target` describes which *axis* should
                    // be rotated around. It just so happens that the Y
                    // coordinate of the mouse corresponds to a rotation around the X axis
                    // So that's why we add the change in x to the y component of the look
                    // target.
                    camera.orientation.x =
                        Deg(util::clamp(camera.orientation.x.0 + dy, -90.0, 90.0));
                    camera.orientation.y += Deg(dx);
                }

                _ => (),
            }
        }
    }
}
