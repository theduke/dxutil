use dioxus::html::input_data::keyboard_types::{Code, Key as DomKey, Modifiers as DomModifiers};
use dioxus::prelude::*;
use dxterm_core::input::{Key, KeyEvent, KeyEventKind, KeypadKey, Modifiers};

pub(crate) fn modifiers(m: DomModifiers) -> Modifiers {
    let alt_graph = m.contains(DomModifiers::ALT_GRAPH);
    Modifiers {
        shift: m.contains(DomModifiers::SHIFT),
        alt: m.contains(DomModifiers::ALT) && !alt_graph,
        control: m.contains(DomModifiers::CONTROL) && !alt_graph,
        super_key: m.contains(DomModifiers::META),
        caps_lock: m.contains(DomModifiers::CAPS_LOCK),
        num_lock: m.contains(DomModifiers::NUM_LOCK),
    }
}
pub(crate) fn key_event(data: &KeyboardData, release: bool) -> Option<KeyEvent> {
    // NumLock-off keypad navigation must retain its logical arrow/Home/etc.
    let code = if matches!(data.key(), DomKey::Character(_) | DomKey::Enter) {
        data.code()
    } else {
        Code::Unidentified
    };
    let key = match code {
        Code::Numpad0 => Key::Keypad(KeypadKey::Digit(0)),
        Code::Numpad1 => Key::Keypad(KeypadKey::Digit(1)),
        Code::Numpad2 => Key::Keypad(KeypadKey::Digit(2)),
        Code::Numpad3 => Key::Keypad(KeypadKey::Digit(3)),
        Code::Numpad4 => Key::Keypad(KeypadKey::Digit(4)),
        Code::Numpad5 => Key::Keypad(KeypadKey::Digit(5)),
        Code::Numpad6 => Key::Keypad(KeypadKey::Digit(6)),
        Code::Numpad7 => Key::Keypad(KeypadKey::Digit(7)),
        Code::Numpad8 => Key::Keypad(KeypadKey::Digit(8)),
        Code::Numpad9 => Key::Keypad(KeypadKey::Digit(9)),
        Code::NumpadAdd => Key::Keypad(KeypadKey::Add),
        Code::NumpadSubtract => Key::Keypad(KeypadKey::Subtract),
        Code::NumpadMultiply => Key::Keypad(KeypadKey::Multiply),
        Code::NumpadDivide => Key::Keypad(KeypadKey::Divide),
        Code::NumpadDecimal => Key::Keypad(KeypadKey::Decimal),
        Code::NumpadEnter => Key::Keypad(KeypadKey::Enter),
        Code::NumpadEqual => Key::Keypad(KeypadKey::Equal),
        _ => match data.key() {
            DomKey::Character(text) => Key::Character(text),
            DomKey::Enter => Key::Enter,
            DomKey::Tab => Key::Tab,
            DomKey::Backspace => Key::Backspace,
            DomKey::Escape => Key::Escape,
            DomKey::ArrowUp => Key::ArrowUp,
            DomKey::ArrowDown => Key::ArrowDown,
            DomKey::ArrowLeft => Key::ArrowLeft,
            DomKey::ArrowRight => Key::ArrowRight,
            DomKey::Home => Key::Home,
            DomKey::End => Key::End,
            DomKey::Insert => Key::Insert,
            DomKey::Delete => Key::Delete,
            DomKey::PageUp => Key::PageUp,
            DomKey::PageDown => Key::PageDown,
            other => {
                let name = other.to_string();
                Key::Function(name.strip_prefix('F')?.parse().ok()?)
            }
        },
    };
    let mut event = KeyEvent::new(key, modifiers(data.modifiers()));
    event.kind = if release {
        KeyEventKind::Release
    } else if data.is_auto_repeating() {
        KeyEventKind::Repeat
    } else {
        KeyEventKind::Press
    };
    if let Key::Character(text) = &event.key {
        event.base_key = text.chars().next().and_then(|c| c.to_lowercase().next());
        if event.modifiers.shift {
            event.shifted_key = text.chars().next();
        }
        event.base_layout_key =
            base_layout_key(data.code()).filter(|base| Some(*base) != event.base_key);
        if !event.modifiers.control && !event.modifiers.super_key && !event.modifiers.alt {
            event.text = Some(text.clone());
        }
    }
    Some(event)
}

fn base_layout_key(code: Code) -> Option<char> {
    let name = code.to_string();
    if let Some(letter) = name.strip_prefix("Key").filter(|value| value.len() == 1) {
        return letter.chars().next().map(|c| c.to_ascii_lowercase());
    }
    if let Some(digit) = name.strip_prefix("Digit").filter(|value| value.len() == 1) {
        return digit.chars().next();
    }
    Some(match code {
        Code::Backquote => '`',
        Code::Backslash => '\\',
        Code::BracketLeft => '[',
        Code::BracketRight => ']',
        Code::Comma => ',',
        Code::Equal => '=',
        Code::Minus => '-',
        Code::Period => '.',
        Code::Quote => '\'',
        Code::Semicolon => ';',
        Code::Slash => '/',
        _ => return None,
    })
}
