use crossbeam_channel::Receiver;
use glium::{
    glutin::{
        event::{
            ButtonId, DeviceEvent, DeviceId, ElementState, KeyboardInput, ModifiersState,
            MouseScrollDelta, VirtualKeyCode,
        },
        window::Window,
    },
    Display,
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

// digital as in "on or off"
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DigitalInput {
    Physical(u32),
    Button(u32),
    Virtual(VirtualKeyCode),
}

impl From<VirtualKeyCode> for DigitalInput {
    fn from(vkk: VirtualKeyCode) -> Self {
        DigitalInput::Virtual(vkk)
    }
}

impl From<u32> for DigitalInput {
    fn from(sc: u32) -> Self {
        DigitalInput::Physical(sc)
    }
}

#[derive(Debug)]
pub struct InputState {
    physical_map: HashMap<VirtualKeyCode, u32>,

    rising_keys: HashSet<u32>,
    falling_keys: HashSet<u32>,
    pressed_keys: HashSet<u32>,

    rising_buttons: HashSet<u32>,
    falling_buttons: HashSet<u32>,
    pressed_buttons: HashSet<u32>,

    current_modifiers: ModifiersState,

    cursor_dx: f32,
    cursor_dy: f32,
    pub sensitivity: f32,

    cursor_currently_grabbed: bool,
    cursor_should_be_grabbed: AtomicBool,
    cursor_currently_hidden: bool,
    cursor_should_be_hidden: AtomicBool,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            physical_map: Default::default(),
            rising_keys: Default::default(),
            falling_keys: Default::default(),
            pressed_keys: Default::default(),
            rising_buttons: Default::default(),
            falling_buttons: Default::default(),
            pressed_buttons: Default::default(),

            current_modifiers: Default::default(),

            cursor_dx: 0.0,
            cursor_dy: 0.0,
            sensitivity: 0.10,

            cursor_currently_grabbed: false,
            cursor_should_be_grabbed: false.into(),
            cursor_currently_hidden: false,
            cursor_should_be_hidden: false.into(),
        }
    }
}

impl InputState {
    pub fn grab_cursor(&self, grab: bool) {
        self.cursor_should_be_grabbed.store(grab, Ordering::SeqCst);
    }

    pub fn hide_cursor(&self, hide: bool) {
        self.cursor_should_be_hidden.store(hide, Ordering::SeqCst);
    }

    pub fn is_cursor_grabbed(&self) -> bool {
        self.cursor_should_be_grabbed.load(Ordering::SeqCst)
    }

    pub fn is_cursor_hidden(&self) -> bool {
        self.cursor_should_be_hidden.load(Ordering::SeqCst)
    }

    pub fn cursor_delta(&self) -> nalgebra::Vector2<f32> {
        self.sensitivity * nalgebra::vector![self.cursor_dx, self.cursor_dy]
    }

    pub fn key<K: Into<DigitalInput>>(&self, key: K) -> KeyRef {
        KeyRef {
            state: self,
            key: key.into(),
            modifiers_to_match: None,
        }
    }

    pub fn ctrl(&self) -> bool {
        self.current_modifiers.ctrl()
    }

    pub fn alt(&self) -> bool {
        self.current_modifiers.alt()
    }

    pub fn shift(&self) -> bool {
        self.current_modifiers.shift()
    }

    pub fn logo(&self) -> bool {
        self.current_modifiers.logo()
    }

    fn modifiers_match(&self, modifiers: Option<ModifiersState>) -> bool {
        modifiers.map_or(true, |modifiers| self.current_modifiers == modifiers)
    }

    fn is_key_in_set<'s>(
        &'s self,
        key: DigitalInput,
        key_set: &'s HashSet<u32>,
        button_set: &'s HashSet<u32>,
    ) -> Option<bool> {
        Some(match key {
            DigitalInput::Button(id) => button_set.contains(&id),
            DigitalInput::Virtual(vkk) => key_set.contains(self.physical_map.get(&vkk)?),
            DigitalInput::Physical(code) => key_set.contains(&code),
        })
    }
}

pub struct KeyRef<'s> {
    state: &'s InputState,
    key: DigitalInput,
    modifiers_to_match: Option<ModifiersState>,
}

impl<'s> KeyRef<'s> {
    pub fn require_modifiers(self, modifiers: ModifiersState) -> Self {
        Self {
            modifiers_to_match: Some(modifiers),
            ..self
        }
    }

    pub fn is_pressed(&self) -> bool {
        let key = self
            .state
            .is_key_in_set(
                self.key,
                &self.state.pressed_keys,
                &self.state.pressed_buttons,
            )
            .unwrap_or(false);
        key && self.state.modifiers_match(self.modifiers_to_match)
    }

    pub fn is_rising(&self) -> bool {
        let key = self
            .state
            .is_key_in_set(
                self.key,
                &self.state.rising_keys,
                &self.state.rising_buttons,
            )
            .unwrap_or(false);
        key && self.state.modifiers_match(self.modifiers_to_match)
    }

    pub fn is_falling(&self) -> bool {
        let key = self
            .state
            .is_key_in_set(
                self.key,
                &self.state.falling_keys,
                &self.state.falling_buttons,
            )
            .unwrap_or(false);
        key && self.state.modifiers_match(self.modifiers_to_match)
    }
}

pub mod keys {
    use glium::glutin::event::VirtualKeyCode;

    pub const FORWARD: u32 = 0x11;
    pub const BACKWARD: u32 = 0x1F;
    pub const LEFT: u32 = 0x1E;
    pub const RIGHT: u32 = 0x20;
    pub const UP: u32 = 0x39;
    pub const DOWN: u32 = 0x2A;

    pub const ARROW_UP: VirtualKeyCode = VirtualKeyCode::Up;
    pub const ARROW_DOWN: VirtualKeyCode = VirtualKeyCode::Down;
    pub const ARROW_LEFT: VirtualKeyCode = VirtualKeyCode::Left;
    pub const ARROW_RIGHT: VirtualKeyCode = VirtualKeyCode::Right;
}

fn maintain_input_state(state: &mut InputState, window: &Window) {
    let should_grab = state.cursor_should_be_grabbed.load(Ordering::SeqCst);
    let should_hide = state.cursor_should_be_hidden.load(Ordering::SeqCst);

    if state.cursor_currently_grabbed != should_grab {
        state.cursor_currently_grabbed = should_grab;
        window.set_cursor_grab(should_grab).unwrap();
    }

    if state.cursor_currently_hidden != should_hide {
        state.cursor_currently_hidden = should_hide;
        window.set_cursor_visible(!should_hide);
    }

    state.rising_keys.clear();
    state.falling_keys.clear();

    state.rising_buttons.clear();
    state.falling_buttons.clear();

    state.cursor_dx = 0.0;
    state.cursor_dy = 0.0;
}

fn notify_keyboard_input(state: &mut InputState, input: KeyboardInput) {
    // update tracked modifier state
    let to_set = match input.virtual_keycode {
        Some(VirtualKeyCode::LShift) | Some(VirtualKeyCode::RShift) => ModifiersState::SHIFT,
        Some(VirtualKeyCode::LAlt) | Some(VirtualKeyCode::RAlt) => ModifiersState::ALT,
        Some(VirtualKeyCode::LControl) | Some(VirtualKeyCode::RControl) => ModifiersState::CTRL,
        Some(VirtualKeyCode::LWin) | Some(VirtualKeyCode::RWin) => ModifiersState::LOGO,
        _ => ModifiersState::empty(),
    };

    let pressed = matches!(input.state, ElementState::Pressed);
    state.current_modifiers.set(to_set, pressed);

    // add virtual keycode -> scancode mapping
    if let Some(vkk) = input.virtual_keycode {
        if state.physical_map.insert(vkk, input.scancode).is_none() {
            log::debug!("found physical mapping for '{:?}': {}", vkk, input.scancode);
        }
    }

    // update rising/falling sets
    if pressed && state.pressed_keys.insert(input.scancode) {
        state.rising_keys.insert(input.scancode);
    } else if !pressed && state.pressed_keys.remove(&input.scancode) {
        state.falling_keys.insert(input.scancode);
    }
}

fn notify_mouse_motion(state: &mut InputState, dx: f64, dy: f64) {
    state.cursor_dx += dx as f32;
    state.cursor_dy += dy as f32;
}

fn notify_mouse_scroll(_state: &mut InputState, _delta: MouseScrollDelta) {}

fn notify_mouse_click(state: &mut InputState, button: ButtonId, elem_state: ElementState) {
    let pressed = matches!(elem_state, ElementState::Pressed);

    // update rising/falling sets
    if pressed && state.pressed_buttons.insert(button) {
        state.rising_buttons.insert(button);
    } else if !pressed && state.pressed_buttons.remove(&button) {
        state.falling_buttons.insert(button);
    }
}

#[legion::system]
pub fn input_compiler(
    #[resource] state: &mut InputState,
    #[state] events: &mut Receiver<(DeviceId, DeviceEvent)>,
    #[state] display: &mut Rc<Display>,
) {
    maintain_input_state(state, display.gl_window().window());

    for (_device_id, event) in events.try_iter() {
        match event {
            DeviceEvent::MouseMotion { delta } => notify_mouse_motion(state, delta.0, delta.1),
            DeviceEvent::MouseWheel { delta } => notify_mouse_scroll(state, delta),
            DeviceEvent::Key(input) => notify_keyboard_input(state, input),
            DeviceEvent::Button {
                button,
                state: elem_state,
            } => notify_mouse_click(state, button, elem_state),

            // DeviceEvent::Motion { axis, value } => todo!(),
            // DeviceEvent::Text { codepoint } => todo!(),
            _ => {}
        }
    }
}
