use wasm_bindgen::JsCast;

use crate::viewport::ViewportPoint;

#[derive(Debug)]
pub enum MouseAction {
    Down,
    Enter,
    Up,
    Leave,
    Move,
    Wheel(f32),
}

#[derive(Clone, Copy, Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    Unknown,
}

impl MouseButton {
    fn from(button: i16) -> Self {
        // Reference:
        // https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            3 => MouseButton::Back,
            4 => MouseButton::Forward,
            _ => MouseButton::Unknown,
        }
    }
}

#[derive(Debug)]
pub enum KeyboardAction {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug)]
pub enum Key {
    Alt,
    Control,
    Delete,
    Equals,
    Down,
    Escape,
    Left,
    Minus,
    Plus,
    Right,
    Shift,
    Space,
    Tab,
    Underscore,
    Up,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Unknown,
}

impl Key {
    const LOG_UNKNOWN: bool = true;

    fn from(key: &str) -> Self {
        match key {
            "Alt" => Self::Alt,
            "Backspace" => Self::Delete,
            "Control" => Self::Control,
            "Delete" => Self::Delete,
            "Escape" => Self::Escape,
            "Shift" => Self::Shift,
            "Tab" => Self::Tab,
            "ArrowDown" => Self::Down,
            "ArrowLeft" => Self::Left,
            "ArrowRight" => Self::Right,
            "ArrowUp" => Self::Up,
            "-" => Self::Minus,
            "_" => Self::Underscore,
            "=" => Self::Equals,
            "+" => Self::Plus,
            " " => Self::Space,
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "f" => Self::F,
            "g" => Self::G,
            "h" => Self::H,
            "i" => Self::I,
            "j" => Self::J,
            "k" => Self::K,
            "l" => Self::L,
            "m" => Self::M,
            "n" => Self::N,
            "o" => Self::O,
            "p" => Self::P,
            "q" => Self::Q,
            "r" => Self::R,
            "s" => Self::S,
            "t" => Self::T,
            "u" => Self::U,
            "v" => Self::V,
            "w" => Self::W,
            "x" => Self::X,
            "y" => Self::Y,
            "z" => Self::Z,
            "A" => Self::A,
            "B" => Self::B,
            "C" => Self::C,
            "D" => Self::D,
            "E" => Self::E,
            "F" => Self::F,
            "G" => Self::G,
            "H" => Self::H,
            "I" => Self::I,
            "J" => Self::J,
            "K" => Self::K,
            "L" => Self::L,
            "M" => Self::M,
            "N" => Self::N,
            "O" => Self::O,
            "P" => Self::P,
            "Q" => Self::Q,
            "R" => Self::R,
            "S" => Self::S,
            "T" => Self::T,
            "U" => Self::U,
            "V" => Self::V,
            "X" => Self::X,
            "Y" => Self::Y,
            "Z" => Self::Z,
            _ => {
                if Self::LOG_UNKNOWN {
                    crate::bridge::flog!("Unknown key: {key}");
                }
                Self::Unknown
            }
        }
    }

    pub fn is_arrow(&self) -> bool {
        matches!(self, Key::Down | Key::Left | Key::Right | Key::Up)
    }
}

#[derive(Debug)]
pub enum Input {
    Mouse(ViewportPoint, MouseAction, MouseButton),
    Keyboard(KeyboardAction, Key),
}

#[derive(Debug)]
pub struct InputEvent {
    pub input: Input,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl InputEvent {
    pub fn from_web_sys(event: &web_sys::UiEvent) -> Option<InputEvent> {
        match event.type_().as_str() {
            "keydown" | "keyup" => {
                Self::from_keyboard(event.unchecked_ref::<web_sys::KeyboardEvent>())
            }
            "mousedown" | "mouseenter" | "mouseleave" | "mousemove" | "mouseup" | "wheel" => {
                Self::from_mouse(event.unchecked_ref::<web_sys::MouseEvent>())
            }
            _ => None,
        }
    }

    fn from_mouse(event: &web_sys::MouseEvent) -> Option<InputEvent> {
        let action = match event.type_().as_str() {
            "mousedown" => MouseAction::Down,
            "mouseenter" => MouseAction::Enter,
            "mouseleave" => MouseAction::Leave,
            "mousemove" => MouseAction::Move,
            "mouseup" => MouseAction::Up,
            "wheel" => {
                let event = event.unchecked_ref::<web_sys::WheelEvent>();

                // Because the app never has scroll bars, the delta is always
                // reported in the y
                MouseAction::Wheel(event.delta_y() as f32)
            }
            _ => return None,
        };

        Some(InputEvent {
            input: Input::Mouse(
                ViewportPoint::new(event.x(), event.y()),
                action,
                MouseButton::from(event.button()),
            ),
            shift: event.shift_key(),
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
        })
    }

    fn from_keyboard(event: &web_sys::KeyboardEvent) -> Option<InputEvent> {
        let action = match event.type_().as_str() {
            "keydown" => KeyboardAction::Down,
            "keyup" => KeyboardAction::Up,
            _ => return None,
        };

        Some(InputEvent {
            input: Input::Keyboard(action, Key::from(&event.key())),
            shift: event.shift_key(),
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
        })
    }
}
