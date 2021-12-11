use crate::InputEvent;
use crossbeam_channel::Receiver;
use glium::glutin::event::{ElementState, KeyboardInput, ModifiersState, VirtualKeyCode};
use std::collections::{HashMap, HashSet};

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

// const KEYBIND_ZOOM: Keybind = Keybind {
//     key: Key::Physical(0x2E),
//     modifiers: Some(NO_MODIFIERS),
// };

// const KEYBIND_EXIT: Keybind = Keybind {
//     key: Key::Virtual(VirtualKeyCode::Escape),
//     modifiers: Some(NO_MODIFIERS),
// };
// const KEYBIND_DEBUG: Keybind = Keybind {
//     key: Key::Virtual(VirtualKeyCode::B),
//     modifiers: Some(CTRL_MODIFIERS),
// };
// const KEYBIND_TOGGLE_WIREFRAME: Keybind = Keybind {
//     key: Key::Virtual(VirtualKeyCode::F),
//     modifiers: Some(CTRL_MODIFIERS),
// };
// const KEYBIND_INC_RENDER_DISTANCE: Keybind = Keybind {
//     key: Key::Virtual(VirtualKeyCode::RBracket),
//     modifiers: Some(CTRL_MODIFIERS),
// };
// const KEYBIND_DEC_RENDER_DISTANCE: Keybind = Keybind {
//     key: Key::Virtual(VirtualKeyCode::LBracket),
//     modifiers: Some(CTRL_MODIFIERS),
// };

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub struct Modifiers {
    shift: bool,
    ctrl: bool,
    alt: bool,
    sup: bool,
}

impl Modifiers {
    pub const ALT: Self = Modifiers {
        shift: false,
        ctrl: false,
        alt: true,
        sup: false,
    };
    pub const CTRL: Self = Modifiers {
        shift: false,
        ctrl: true,
        alt: false,
        sup: false,
    };
    pub const SHIFT: Self = Modifiers {
        shift: true,
        ctrl: false,
        alt: false,
        sup: false,
    };
    pub const SUPER: Self = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        sup: true,
    };
}

impl From<ModifiersState> for Modifiers {
    fn from(state: ModifiersState) -> Self {
        Modifiers {
            shift: state.shift(),
            ctrl: state.ctrl(),
            alt: state.alt(),
            sup: state.logo(),
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

#[derive(Clone, Debug)]
pub struct InputState {
    physical_map: HashMap<VirtualKeyCode, u32>,
    rising: HashSet<u32>,
    falling: HashSet<u32>,
    pressed: HashSet<u32>,
    current_modifiers: Modifiers,

    cursor_dx: f32,
    cursor_dy: f32,
    pub sensitivity: f32,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            physical_map: Default::default(),
            rising: Default::default(),
            falling: Default::default(),
            pressed: Default::default(),
            current_modifiers: Default::default(),

            cursor_dx: 0.0,
            cursor_dy: 0.0,
            sensitivity: 0.25,
        }
    }
}

impl InputState {
    fn update(&mut self) {
        self.rising.clear();
        self.falling.clear();

        self.cursor_dx = 0.0;
        self.cursor_dy = 0.0;
    }

    fn update_keyboard_inputs(&mut self, input: KeyboardInput) {
        self.current_modifiers = input.modifiers.into();

        if let Some(vkk) = input.virtual_keycode {
            if self.physical_map.insert(vkk, input.scancode).is_none() {
                log::debug!("Found physical mapping for `{:?}`: {}", vkk, input.scancode);
            }
        }

        match input.state {
            ElementState::Pressed => {
                if self.pressed.insert(input.scancode) {
                    log::trace!(
                        "Input transitioned to high: {} ({:?})",
                        input.scancode,
                        input.virtual_keycode
                    );
                    self.rising.insert(input.scancode);
                }
            }

            ElementState::Released => {
                if self.pressed.remove(&input.scancode) {
                    log::trace!(
                        "Input transitioned to low: {} ({:?})",
                        input.scancode,
                        input.virtual_keycode
                    );
                    self.falling.insert(input.scancode);
                }
            }
        }
    }

    fn modifiers_match(&self, modifiers: Option<Modifiers>) -> bool {
        modifiers.map_or(true, |modifiers| self.current_modifiers == modifiers)
    }

    pub fn cursor_delta(&self) -> (f32, f32) {
        (
            self.sensitivity * self.cursor_dx,
            self.sensitivity * self.cursor_dy,
        )
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

#[legion::system]
pub fn input_compiler(
    #[resource] state: &mut InputState,
    #[state] events: &mut Receiver<InputEvent>,
) {
    state.update();

    for event in events.try_iter() {
        match event {
            InputEvent::MouseMovement { dx, dy } => {
                state.cursor_dx = dx as f32;
                state.cursor_dy = dy as f32;
            }
            InputEvent::KayboardInput { input, .. } => {
                state.update_keyboard_inputs(input);
            }
        }
    }
}
