use std::collections::{HashMap, HashSet};

use glam::{Vec2, vec2};
use winit::{event::{KeyEvent, WindowEvent}, keyboard::PhysicalKey};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum KeyInner {
    Code(KeyCode),
    Char(char),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    inner: KeyInner
}

impl From<KeyCode> for Key {
    fn from(value: KeyCode) -> Self {
        Self {
            inner: KeyInner::Code(value)
        }
    }
}

impl From<char> for Key {
    fn from(value: char) -> Self {
        Self {
            inner: KeyInner::Char(value.to_ascii_lowercase())
        }
    }
}

struct KeyboardState {
    keycodes: HashMap<KeyCode, ElementState>,
    chars: HashMap<char, ElementState>,
    down_keys: HashSet<Key>,
}

impl KeyboardState {
    fn new() -> Self {
        Self {
            keycodes: HashMap::new(),
            chars: HashMap::new(),
            down_keys: HashSet::new(),
        }
    }

    fn set_key_state(&mut self, code: KeyCode, value: Option<char>, state: ElementState) {
        self.keycodes.insert(code, state);
        self.set_key_down_state(code.into(), state);
        if let Some(value) = value {  
            let value = value.to_ascii_lowercase();
            self.set_key_down_state(value.into(), state);
            self.chars.insert(value, state);
        }
    }

    fn set_key_down_state(&mut self, key: Key, state: ElementState) {
        match state {
            ElementState::Pressed => self.down_keys.insert(key),
            ElementState::Released => self.down_keys.remove(&key),
        };
    }

    fn clear(&mut self) {
        self.keycodes.clear();
        self.chars.clear();
    }

    fn is_key_pressed(&self, key: Key) -> bool {
        let state_opt = match key.inner {
            KeyInner::Code(key_code) => self.keycodes.get(&key_code),
            KeyInner::Char(c) => self.chars.get(&c),
        };
        state_opt.is_some_and(|s| *s == ElementState::Pressed)
    }

    fn is_key_released(&self, key: Key) -> bool {
        let state_opt = match key.inner {
            KeyInner::Code(key_code) => self.keycodes.get(&key_code),
            KeyInner::Char(c) => self.chars.get(&c),
        };
        state_opt.is_some_and(|s| *s == ElementState::Released)
    }

    fn is_key_down(&self, key: Key) -> bool {
        self.down_keys.contains(&key)
    }
}

struct MouseState {
    mouse_buttons_state: HashMap<MouseButton, ElementState>,
    buttons_down: HashSet<MouseButton>
}

impl MouseState {
    fn new() -> Self {
        Self {
            mouse_buttons_state: HashMap::new(),
            buttons_down: HashSet::new(),
        }
    }

    fn set_button_state(&mut self, button: MouseButton, state: ElementState) {
        self.mouse_buttons_state.insert(button, state);
        match state {
            ElementState::Pressed => self.buttons_down.insert(button),
            ElementState::Released => self.buttons_down.remove(&button),
        };
    }

    fn is_button_down(&self, button: MouseButton) -> bool {
        self.buttons_down.contains(&button)
    }

    fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_state.get(&button)
        .is_some_and(|s| *s == ElementState::Pressed)
    }

    fn is_button_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_state.get(&button)
        .is_some_and(|s| *s == ElementState::Released)
    }

    fn clear(&mut self) {
        self.mouse_buttons_state.clear();
    }
}

pub struct Input {
    keyboard_state: KeyboardState,
    mouse_state: MouseState,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub wheel_delta: Vec2,
    pub last_char: Option<char>,

    scale_factor: f64,
}

impl Input {

    pub fn is_key_pressed(&self, key: impl Into<Key>) -> bool {
        self.keyboard_state.is_key_pressed(key.into())
    }

    pub fn is_key_released(&self, key: impl Into<Key>) -> bool {
        self.keyboard_state.is_key_released(key.into())
    }

    pub fn is_key_down(&self, key: impl Into<Key>) -> bool {
        self.keyboard_state.is_key_down(key.into())
    }

    pub fn is_mouse_button_pressed(&self, btn: MouseButton) -> bool {
        self.mouse_state.is_button_pressed(btn)
    }

    pub fn is_mouse_button_released(&self, btn: MouseButton) -> bool {
         self.mouse_state.is_button_released(btn)
    }

    pub fn is_mouse_button_down(&self, btn: MouseButton) -> bool {
        self.mouse_state.is_button_down(btn)
    }


    pub(crate) fn clear(&mut self) {
        self.keyboard_state.clear();
        self.mouse_state.clear();
        self.mouse_delta = Vec2::ZERO;
        self.wheel_delta = Vec2::ZERO;
        self.last_char = None;
    }

    pub(crate) fn new(scale_factor: f64) -> Self {
        Self {
            keyboard_state: KeyboardState::new(),
            mouse_state: MouseState::new(),
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            wheel_delta: Vec2::ZERO,
            last_char: None,

            scale_factor,
        }
    }

    pub(crate) fn process_events(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state,
                    text,
                    ..
                },
                ..
            } => {
                self.last_char = text.as_ref().and_then(|text| text.chars().last());
                let value = self.last_char;
                self.keyboard_state.set_key_state((*key_code).into(), value, (*state).into());
            }

            WindowEvent::MouseInput {
                button,
                state,
                ..
            } => {
                self.mouse_state.set_button_state((*button).into(), (*state).into());
            }

            WindowEvent::MouseWheel {
                delta,
                ..
            } => {
                self.wheel_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => vec2(*x, *y),
                    winit::event::MouseScrollDelta::PixelDelta(physical_position) => {
                        let logical_pos = physical_position.to_logical(self.scale_factor);
                        vec2(logical_pos.x, logical_pos.y)
                    }
                }
            }

            WindowEvent::CursorMoved {
                position,
                ..
            } => {
                let position = position.to_logical::<f32>(self.scale_factor);
                let new_pos = vec2(position.x, position.y);
                self.mouse_delta += new_pos - self.mouse_position;
                self.mouse_position = new_pos;
            }

            WindowEvent::ScaleFactorChanged {
                scale_factor,
                ..
            } => {
                self.scale_factor = *scale_factor;
            }

            WindowEvent::Focused(false) => {
                self.keyboard_state.down_keys.clear();
                self.mouse_state.buttons_down.clear();
            }

            _ => ()
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ElementState {
    Pressed,
    Released,
}

impl From<winit::event::ElementState> for ElementState {
    fn from(value: winit::event::ElementState) -> Self {
        match value {
            winit::event::ElementState::Pressed => Self::Pressed,
            winit::event::ElementState::Released => Self::Released,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<winit::event::MouseButton> for MouseButton {
    fn from(value: winit::event::MouseButton) -> Self {
        match value {
            winit::event::MouseButton::Left => Self::Left,
            winit::event::MouseButton::Right => Self::Right,
            winit::event::MouseButton::Middle => Self::Middle,
            winit::event::MouseButton::Back => Self::Back,
            winit::event::MouseButton::Forward => Self::Forward,
            winit::event::MouseButton::Other(v) => Self::Other(v),
        }
    }
}


#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum KeyCode {
    Unknown,
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    SuperLeft,
    SuperRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    Convert,
    KanaMode,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    Hiragana,
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
}

impl From<winit::keyboard::KeyCode> for KeyCode {
    fn from(value: winit::keyboard::KeyCode) -> Self {
        match value {
            winit::keyboard::KeyCode::Backquote => Self::Backquote,
            winit::keyboard::KeyCode::Backslash => Self::Backslash,
            winit::keyboard::KeyCode::BracketLeft => Self::BracketLeft,
            winit::keyboard::KeyCode::BracketRight => Self::BracketRight,
            winit::keyboard::KeyCode::Comma => Self::Comma,
            winit::keyboard::KeyCode::Digit0 => Self::Digit0,
            winit::keyboard::KeyCode::Digit1 => Self::Digit1,
            winit::keyboard::KeyCode::Digit2 => Self::Digit2,
            winit::keyboard::KeyCode::Digit3 => Self::Digit3,
            winit::keyboard::KeyCode::Digit4 => Self::Digit4,
            winit::keyboard::KeyCode::Digit5 => Self::Digit5,
            winit::keyboard::KeyCode::Digit6 => Self::Digit6,
            winit::keyboard::KeyCode::Digit7 => Self::Digit7,
            winit::keyboard::KeyCode::Digit8 => Self::Digit8,
            winit::keyboard::KeyCode::Digit9 => Self::Digit9,
            winit::keyboard::KeyCode::Equal => Self::Equal,
            winit::keyboard::KeyCode::IntlBackslash => Self::IntlBackslash,
            winit::keyboard::KeyCode::IntlRo => Self::IntlRo,
            winit::keyboard::KeyCode::IntlYen => Self::IntlYen,
            winit::keyboard::KeyCode::KeyA => Self::KeyA,
            winit::keyboard::KeyCode::KeyB => Self::KeyB,
            winit::keyboard::KeyCode::KeyC => Self::KeyC,
            winit::keyboard::KeyCode::KeyD => Self::KeyD,
            winit::keyboard::KeyCode::KeyE => Self::KeyE,
            winit::keyboard::KeyCode::KeyF => Self::KeyF,
            winit::keyboard::KeyCode::KeyG => Self::KeyG,
            winit::keyboard::KeyCode::KeyH => Self::KeyH,
            winit::keyboard::KeyCode::KeyI => Self::KeyI,
            winit::keyboard::KeyCode::KeyJ => Self::KeyJ,
            winit::keyboard::KeyCode::KeyK => Self::KeyK,
            winit::keyboard::KeyCode::KeyL => Self::KeyL,
            winit::keyboard::KeyCode::KeyM => Self::KeyM,
            winit::keyboard::KeyCode::KeyN => Self::KeyN,
            winit::keyboard::KeyCode::KeyO => Self::KeyO,
            winit::keyboard::KeyCode::KeyP => Self::KeyP,
            winit::keyboard::KeyCode::KeyQ => Self::KeyQ,
            winit::keyboard::KeyCode::KeyR => Self::KeyR,
            winit::keyboard::KeyCode::KeyS => Self::KeyS,
            winit::keyboard::KeyCode::KeyT => Self::KeyT,
            winit::keyboard::KeyCode::KeyU => Self::KeyU,
            winit::keyboard::KeyCode::KeyV => Self::KeyV,
            winit::keyboard::KeyCode::KeyW => Self::KeyW,
            winit::keyboard::KeyCode::KeyX => Self::KeyX,
            winit::keyboard::KeyCode::KeyY => Self::KeyY,
            winit::keyboard::KeyCode::KeyZ => Self::KeyZ,
            winit::keyboard::KeyCode::Minus => Self::Minus,
            winit::keyboard::KeyCode::Period => Self::Period,
            winit::keyboard::KeyCode::Quote => Self::Quote,
            winit::keyboard::KeyCode::Semicolon => Self::Semicolon,
            winit::keyboard::KeyCode::Slash => Self::Slash,
            winit::keyboard::KeyCode::AltLeft => Self::AltLeft,
            winit::keyboard::KeyCode::AltRight => Self::AltRight,
            winit::keyboard::KeyCode::Backspace => Self::Backspace,
            winit::keyboard::KeyCode::CapsLock => Self::CapsLock,
            winit::keyboard::KeyCode::ContextMenu => Self::ContextMenu,
            winit::keyboard::KeyCode::ControlLeft => Self::ControlLeft,
            winit::keyboard::KeyCode::ControlRight => Self::ControlRight,
            winit::keyboard::KeyCode::Enter => Self::Enter,
            winit::keyboard::KeyCode::SuperLeft => Self::SuperLeft,
            winit::keyboard::KeyCode::SuperRight => Self::SuperRight,
            winit::keyboard::KeyCode::ShiftLeft => Self::ShiftLeft,
            winit::keyboard::KeyCode::ShiftRight => Self::ShiftRight,
            winit::keyboard::KeyCode::Space => Self::Space,
            winit::keyboard::KeyCode::Tab => Self::Tab,
            winit::keyboard::KeyCode::Convert => Self::Convert,
            winit::keyboard::KeyCode::KanaMode => Self::KanaMode,
            winit::keyboard::KeyCode::Lang1 => Self::Lang1,
            winit::keyboard::KeyCode::Lang2 => Self::Lang2,
            winit::keyboard::KeyCode::Lang3 => Self::Lang3,
            winit::keyboard::KeyCode::Lang4 => Self::Lang4,
            winit::keyboard::KeyCode::Lang5 => Self::Lang5,
            winit::keyboard::KeyCode::NonConvert => Self::NonConvert,
            winit::keyboard::KeyCode::Delete => Self::Delete,
            winit::keyboard::KeyCode::End => Self::End,
            winit::keyboard::KeyCode::Help => Self::Help,
            winit::keyboard::KeyCode::Home => Self::Home,
            winit::keyboard::KeyCode::Insert => Self::Insert,
            winit::keyboard::KeyCode::PageDown => Self::PageDown,
            winit::keyboard::KeyCode::PageUp => Self::PageUp,
            winit::keyboard::KeyCode::ArrowDown => Self::ArrowDown,
            winit::keyboard::KeyCode::ArrowLeft => Self::ArrowLeft,
            winit::keyboard::KeyCode::ArrowRight => Self::ArrowRight,
            winit::keyboard::KeyCode::ArrowUp => Self::ArrowUp,
            winit::keyboard::KeyCode::NumLock => Self::NumLock,
            winit::keyboard::KeyCode::Numpad0 => Self::Numpad0,
            winit::keyboard::KeyCode::Numpad1 => Self::Numpad1,
            winit::keyboard::KeyCode::Numpad2 => Self::Numpad2,
            winit::keyboard::KeyCode::Numpad3 => Self::Numpad3,
            winit::keyboard::KeyCode::Numpad4 => Self::Numpad4,
            winit::keyboard::KeyCode::Numpad5 => Self::Numpad5,
            winit::keyboard::KeyCode::Numpad6 => Self::Numpad6,
            winit::keyboard::KeyCode::Numpad7 => Self::Numpad7,
            winit::keyboard::KeyCode::Numpad8 => Self::Numpad8,
            winit::keyboard::KeyCode::Numpad9 => Self::Numpad9,
            winit::keyboard::KeyCode::NumpadAdd => Self::NumpadAdd,
            winit::keyboard::KeyCode::NumpadBackspace => Self::NumpadBackspace,
            winit::keyboard::KeyCode::NumpadClear => Self::NumpadClear,
            winit::keyboard::KeyCode::NumpadClearEntry => Self::NumpadClearEntry,
            winit::keyboard::KeyCode::NumpadComma => Self::NumpadComma,
            winit::keyboard::KeyCode::NumpadDecimal => Self::NumpadDecimal,
            winit::keyboard::KeyCode::NumpadDivide => Self::NumpadDivide,
            winit::keyboard::KeyCode::NumpadEnter => Self::NumpadEnter,
            winit::keyboard::KeyCode::NumpadEqual => Self::NumpadEqual,
            winit::keyboard::KeyCode::NumpadHash => Self::NumpadHash,
            winit::keyboard::KeyCode::NumpadMemoryAdd => Self::NumpadMemoryAdd,
            winit::keyboard::KeyCode::NumpadMemoryClear => Self::NumpadMemoryClear,
            winit::keyboard::KeyCode::NumpadMemoryRecall => Self::NumpadMemoryRecall,
            winit::keyboard::KeyCode::NumpadMemoryStore => Self::NumpadMemoryStore,
            winit::keyboard::KeyCode::NumpadMemorySubtract => Self::NumpadMemorySubtract,
            winit::keyboard::KeyCode::NumpadMultiply => Self::NumpadMultiply,
            winit::keyboard::KeyCode::NumpadParenLeft => Self::NumpadParenLeft,
            winit::keyboard::KeyCode::NumpadParenRight => Self::NumpadParenRight,
            winit::keyboard::KeyCode::NumpadStar => Self::NumpadStar,
            winit::keyboard::KeyCode::NumpadSubtract => Self::NumpadSubtract,
            winit::keyboard::KeyCode::Escape => Self::Escape,
            winit::keyboard::KeyCode::Fn => Self::Fn,
            winit::keyboard::KeyCode::FnLock => Self::FnLock,
            winit::keyboard::KeyCode::PrintScreen => Self::PrintScreen,
            winit::keyboard::KeyCode::ScrollLock => Self::ScrollLock,
            winit::keyboard::KeyCode::Pause => Self::Pause,
            winit::keyboard::KeyCode::BrowserBack => Self::BrowserBack,
            winit::keyboard::KeyCode::BrowserFavorites => Self::BrowserFavorites,
            winit::keyboard::KeyCode::BrowserForward => Self::BrowserForward,
            winit::keyboard::KeyCode::BrowserHome => Self::BrowserHome,
            winit::keyboard::KeyCode::BrowserRefresh => Self::BrowserRefresh,
            winit::keyboard::KeyCode::BrowserSearch => Self::BrowserSearch,
            winit::keyboard::KeyCode::BrowserStop => Self::BrowserStop,
            winit::keyboard::KeyCode::Eject => Self::Eject,
            winit::keyboard::KeyCode::LaunchApp1 => Self::LaunchApp1,
            winit::keyboard::KeyCode::LaunchApp2 => Self::LaunchApp2,
            winit::keyboard::KeyCode::LaunchMail => Self::LaunchMail,
            winit::keyboard::KeyCode::MediaPlayPause => Self::MediaPlayPause,
            winit::keyboard::KeyCode::MediaSelect => Self::MediaSelect,
            winit::keyboard::KeyCode::MediaStop => Self::MediaStop,
            winit::keyboard::KeyCode::MediaTrackNext => Self::MediaTrackNext,
            winit::keyboard::KeyCode::MediaTrackPrevious => Self::MediaTrackPrevious,
            winit::keyboard::KeyCode::Power => Self::Power,
            winit::keyboard::KeyCode::Sleep => Self::Sleep,
            winit::keyboard::KeyCode::AudioVolumeDown => Self::AudioVolumeDown,
            winit::keyboard::KeyCode::AudioVolumeMute => Self::AudioVolumeMute,
            winit::keyboard::KeyCode::AudioVolumeUp => Self::AudioVolumeUp,
            winit::keyboard::KeyCode::WakeUp => Self::WakeUp,
            winit::keyboard::KeyCode::Meta => Self::Meta,
            winit::keyboard::KeyCode::Hyper => Self::Hyper,
            winit::keyboard::KeyCode::Turbo => Self::Turbo,
            winit::keyboard::KeyCode::Abort => Self::Abort,
            winit::keyboard::KeyCode::Resume => Self::Resume,
            winit::keyboard::KeyCode::Suspend => Self::Suspend,
            winit::keyboard::KeyCode::Again => Self::Again,
            winit::keyboard::KeyCode::Copy => Self::Copy,
            winit::keyboard::KeyCode::Cut => Self::Cut,
            winit::keyboard::KeyCode::Find => Self::Find,
            winit::keyboard::KeyCode::Open => Self::Open,
            winit::keyboard::KeyCode::Paste => Self::Paste,
            winit::keyboard::KeyCode::Props => Self::Props,
            winit::keyboard::KeyCode::Select => Self::Select,
            winit::keyboard::KeyCode::Undo => Self::Undo,
            winit::keyboard::KeyCode::Hiragana => Self::Hiragana,
            winit::keyboard::KeyCode::Katakana => Self::Katakana,
            winit::keyboard::KeyCode::F1 => Self::F1,
            winit::keyboard::KeyCode::F2 => Self::F2,
            winit::keyboard::KeyCode::F3 => Self::F3,
            winit::keyboard::KeyCode::F4 => Self::F4,
            winit::keyboard::KeyCode::F5 => Self::F5,
            winit::keyboard::KeyCode::F6 => Self::F6,
            winit::keyboard::KeyCode::F7 => Self::F7,
            winit::keyboard::KeyCode::F8 => Self::F8,
            winit::keyboard::KeyCode::F9 => Self::F9,
            winit::keyboard::KeyCode::F10 => Self::F10,
            winit::keyboard::KeyCode::F11 => Self::F11,
            winit::keyboard::KeyCode::F12 => Self::F12,
            winit::keyboard::KeyCode::F13 => Self::F13,
            winit::keyboard::KeyCode::F14 => Self::F14,
            winit::keyboard::KeyCode::F15 => Self::F15,
            winit::keyboard::KeyCode::F16 => Self::F16,
            winit::keyboard::KeyCode::F17 => Self::F17,
            winit::keyboard::KeyCode::F18 => Self::F18,
            winit::keyboard::KeyCode::F19 => Self::F19,
            winit::keyboard::KeyCode::F20 => Self::F20,
            winit::keyboard::KeyCode::F21 => Self::F21,
            winit::keyboard::KeyCode::F22 => Self::F22,
            winit::keyboard::KeyCode::F23 => Self::F23,
            winit::keyboard::KeyCode::F24 => Self::F24,
            winit::keyboard::KeyCode::F25 => Self::F25,
            winit::keyboard::KeyCode::F26 => Self::F26,
            winit::keyboard::KeyCode::F27 => Self::F27,
            winit::keyboard::KeyCode::F28 => Self::F28,
            winit::keyboard::KeyCode::F29 => Self::F29,
            winit::keyboard::KeyCode::F30 => Self::F30,
            winit::keyboard::KeyCode::F31 => Self::F31,
            winit::keyboard::KeyCode::F32 => Self::F32,
            winit::keyboard::KeyCode::F33 => Self::F33,
            winit::keyboard::KeyCode::F34 => Self::F34,
            winit::keyboard::KeyCode::F35 => Self::F35,
            _ => Self::Unknown,
        }
    }
}