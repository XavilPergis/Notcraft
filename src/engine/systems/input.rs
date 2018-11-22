use cgmath::Deg;
use engine::{
    prelude::*,
    render::debug::{DebugAccumulator, Shape},
};
use glutin::{ElementState, Event, KeyboardInput, ModifiersState, VirtualKeyCode, WindowEvent};
use shrev::EventChannel;
use specs::shred::PanicHandler;

pub struct SmoothCamera;

impl<'a> System<'a> for SmoothCamera {
    type SystemData = (
        WriteStorage<'a, comp::Transform>,
        ReadStorage<'a, comp::LookTarget>,
        Read<'a, res::Dt>,
    );

    fn run(&mut self, (mut transforms, targets, dt): Self::SystemData) {
        for (tfm, target) in (&mut transforms, &targets).join() {
            use util::lerp_angle;
            tfm.orientation.x = lerp_angle(tfm.orientation.x, target.x, 12.0 * dt.as_secs());
            tfm.orientation.y = lerp_angle(tfm.orientation.y, target.y, 12.0 * dt.as_secs());
        }
    }
}

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

use gl_api::misc::*;

fn set_wireframe(wf: bool) {
    polygon_mode(if wf {
        PolygonMode::Line
    } else {
        PolygonMode::Fill
    });
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

use engine::components as comp;

#[derive(SystemData)]
pub struct ReadClientPlayer<'a> {
    client_controlled: ReadStorage<'a, comp::ClientControlled>,
    player_marker: ReadStorage<'a, comp::Player>,
    transform: ReadStorage<'a, comp::Transform>,
}

impl<'a> ReadClientPlayer<'a> {
    fn get_transform(&self) -> Option<&comp::Transform> {
        (
            &self.client_controlled,
            &self.player_marker,
            &self.transform,
        )
            .join()
            .next()
            .map(|(_, _, tfm)| tfm)
    }
}

impl<'a> System<'a> for InputHandler {
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        WriteStorage<'a, comp::MoveDelta>,
        Write<'a, res::StopGameLoop>,
        Write<'a, res::ActiveDirections>,
        Write<'a, res::ViewFrustum, PanicHandler>,
        Write<'a, res::ViewDistance, PanicHandler>,
        ReadClientPlayer<'a>,
        WriteExpect<'a, DebugAccumulator>,
    );

    fn run(
        &mut self,
        (
            window_events,
            mut move_deltas,
            mut stop_flag,
            mut active_directions,
            mut frustum,
            mut view_distance,
            player,
            mut debug,
        ): Self::SystemData,
    ) {
        for delta in (&mut move_deltas).join() {
            *delta = comp::MoveDelta::default();
        }

        for event in window_events.read(&mut self.events_handle) {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => {
                        stop_flag.0 = true;
                        break;
                    }

                    WindowEvent::KeyboardInput {
                        input:
                            input @ KeyboardInput {
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => {
                        if KEYBIND_FORWARDS.matches_input(*input) {
                            active_directions.front = true;
                        }
                        if KEYBIND_BACKWARDS.matches_input(*input) {
                            active_directions.back = true;
                        }
                        if KEYBIND_LEFT.matches_input(*input) {
                            active_directions.left = true;
                        }
                        if KEYBIND_RIGHT.matches_input(*input) {
                            active_directions.right = true;
                        }
                        if KEYBIND_UP.matches_input(*input) {
                            active_directions.up = true;
                        }
                        if KEYBIND_DOWN.matches_input(*input) {
                            active_directions.down = true;
                        }
                        if KEYBIND_ZOOM.matches_input(*input) {
                            frustum.fov = Deg(20.0);
                        }
                        if KEYBIND_EXIT.matches_input(*input) {
                            stop_flag.0 = true;
                            break;
                        }

                        if KEYBIND_INC_RENDER_DISTANCE.matches_input(*input) {
                            view_distance.0 += Vector3::new(1, 1, 1);
                        }
                        if KEYBIND_DEC_RENDER_DISTANCE.matches_input(*input) {
                            view_distance.0 -= Vector3::new(1, 1, 1);
                        }

                        // TODO: I don't like having arbitrary strings in my program like this,
                        // maybe I can find a cleaner way to do debug
                        // sections later, but it's fine for now.
                        for &(key, section) in &[
                            (VirtualKeyCode::C, "chunk grid"),
                            (VirtualKeyCode::T, "terrain generation"),
                            (VirtualKeyCode::M, "mesher"),
                            (VirtualKeyCode::P, "physics"),
                            (VirtualKeyCode::I, "interaction"),
                        ] {
                            if Keybind::new(
                                key,
                                Some(ModifiersState {
                                    shift: false,
                                    ctrl: true,
                                    alt: false,
                                    logo: false,
                                }),
                            )
                            .matches_input(*input)
                            {
                                debug.toggle(section.to_owned());
                            }
                        }

                        if KEYBIND_DEBUG.matches_input(*input) {
                            let tfm = player.get_transform().unwrap();
                            let bpos: BlockPos = WorldPos(tfm.position).into();
                            let (cpos, offset) = bpos.chunk_pos_offset();
                            debug!("client position: {:?}", tfm.position);
                            debug!("chunk/offset: {:?}/{:?}", cpos, offset);
                        }
                        if KEYBIND_TOGGLE_WIREFRAME.matches_input(*input) {
                            info!("Toggled wireframe rendering");
                            self.wireframe = !self.wireframe;
                            set_wireframe(self.wireframe);
                        }
                    }

                    WindowEvent::KeyboardInput {
                        input:
                            input @ KeyboardInput {
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    } => {
                        if KEYBIND_FORWARDS.matches_input(*input) {
                            active_directions.front = false;
                        }
                        if KEYBIND_BACKWARDS.matches_input(*input) {
                            active_directions.back = false;
                        }
                        if KEYBIND_LEFT.matches_input(*input) {
                            active_directions.left = false;
                        }
                        if KEYBIND_RIGHT.matches_input(*input) {
                            active_directions.right = false;
                        }
                        if KEYBIND_UP.matches_input(*input) {
                            active_directions.up = false;
                        }
                        if KEYBIND_DOWN.matches_input(*input) {
                            active_directions.down = false;
                        }
                        if KEYBIND_ZOOM.matches_input(*input) {
                            frustum.fov = Deg(80.0);
                        }
                    }

                    // Event::Key(Key::F3, _, Action::Press, _) => { self.capture_mouse =
                    // !self.capture_mouse; set_mouse_capture(&mut *self.window.lock().unwrap(),
                    // self.capture_mouse) }, Event::Key(Key::RightBracket, _,
                    // Action::Press, _) => { view_distance.0 += Vector3::new(1, 1, 1); },
                    _ => {}
                }
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
        ReadClientPlayer<'a>,
        ReadExpect<'a, DebugAccumulator>,
    );

    fn run(&mut self, (events, mut world, player, debug): Self::SystemData) {
        let mut section = debug.section("interaction");
        let transform = player.get_transform().unwrap();
        let ray = transform.camera_ray();
        section.draw(Shape::Ray(10.0, ray, Vector4::new(1.0, 0.0, 0.0, 1.0)));
        if let Some((block, normal)) = world.trace_block(transform.camera_ray(), 10.0, &mut section)
        {
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
                            world.trace_block(transform.camera_ray(), 10.0, &mut section)
                        {
                            world.set_block_id(block, ::engine::world::block::AIR);
                        }
                    }
                    3 => {
                        if let Some((block, Some(normal))) =
                            world.trace_block(transform.camera_ray(), 10.0, &mut section)
                        {
                            world.set_block_id(block.offset(normal), ::engine::world::block::STONE);
                        }
                    }
                    _ => {}
                },

                _ => (),
            }
        }
    }
}

pub struct LockCursor {
    reader: ReaderId<Event>,
}

impl LockCursor {
    pub fn new(events: &mut EventChannel<Event>) -> Self {
        LockCursor {
            reader: events.register_reader(),
        }
    }
}

use glutin::DeviceEvent;

impl<'a> System<'a> for LockCursor {
    type SystemData = (
        Read<'a, EventChannel<Event>>,
        WriteStorage<'a, comp::LookTarget>,
    );

    fn run(&mut self, (events, mut look_targets): Self::SystemData) {
        for event in events.read(&mut self.reader) {
            match event {
                &Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta: (dx, dy) },
                    ..
                } => {
                    for target in (&mut look_targets).join() {
                        // Ok, I know this looks weird, but `target` describes which *axis* should
                        // be rotated around. It just so happens that the Y
                        // coordinate of the mouse corresponds to a rotation around the X axis
                        // So that's why we add the change in x to the y component of the look
                        // target.
                        target.x = Deg(::util::clamp(target.x.0 + dy, -90.0, 90.0));
                        target.y += Deg(dx);
                    }
                }

                _ => (),
            }
        }
    }
}
