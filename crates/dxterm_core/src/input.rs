//! Input encoders written from the xterm and kitty protocol specifications.
//! Kitty reference: <https://sw.kovidgoyal.net/kitty/keyboard-protocol/>.
use crate::CellPoint;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseTracking {
    #[default]
    None,
    X10,
    Normal,
    Button,
    Any,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub application_cursor: bool,
    pub application_keypad: bool,
    pub bracketed_paste: bool,
    pub focus_events: bool,
    pub mouse: MouseTracking,
    pub sgr_mouse: bool,
    pub utf8_mouse: bool,
    pub newline: bool,
    /// Kitty's five progressive enhancement flags, in protocol bit order.
    pub kitty_flags: u8,
    pub modify_other_keys: u8,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub super_key: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
}
impl Modifiers {
    fn parameter(self, kitty: bool) -> u16 {
        1 + u16::from(self.shift)
            + 2 * u16::from(self.alt)
            + 4 * u16::from(self.control)
            + 8 * u16::from(self.super_key)
            + if kitty {
                64 * u16::from(self.caps_lock) + 128 * u16::from(self.num_lock)
            } else {
                0
            }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Enter,
    Tab,
    Backspace,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    Function(u8),
    Keypad(KeypadKey),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeypadKey {
    Digit(u8),
    Decimal,
    Divide,
    Multiply,
    Subtract,
    Add,
    Enter,
    Equal,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyEventKind {
    #[default]
    Press,
    Repeat,
    Release,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
    pub kind: KeyEventKind,
    /// Unshifted logical key (for kitty shortcut matching).
    pub base_key: Option<char>,
    pub shifted_key: Option<char>,
    /// Physical key interpreted in the standard PC layout, for kitty's second
    /// alternate code. Omit when the platform cannot provide it.
    pub base_layout_key: Option<char>,
    pub text: Option<String>,
}
impl KeyEvent {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            modifiers,
            kind: KeyEventKind::Press,
            base_key: None,
            shifted_key: None,
            base_layout_key: None,
            text: None,
        }
    }
}

pub fn encode_key(modes: Modes, event: &KeyEvent) -> Vec<u8> {
    let flags = modes.kitty_flags;
    if event.kind == KeyEventKind::Release && flags & 2 == 0 {
        return Vec::new();
    }
    let m = event.modifiers;
    let text_key = matches!(event.key, Key::Character(_));
    let encode_kitty = flags != 0
        && (flags & 8 != 0
            || flags & 2 != 0 && event.kind != KeyEventKind::Press
            || !text_key && !matches!(event.key, Key::Enter | Key::Tab | Key::Backspace)
            || flags & 1 != 0 && (m.control || m.alt || m.super_key)
            || flags & 16 != 0);
    if encode_kitty {
        return kitty(event, flags);
    }
    if event.kind == KeyEventKind::Release {
        return Vec::new();
    }
    let modifier = m.parameter(false);
    let mut output = match &event.key {
        Key::Character(text) => {
            let ch = text.chars().next();
            if modes.modify_other_keys >= 2
                && modifier > 1
                && let Some(ch) = ch
            {
                return format!("\x1b[27;{modifier};{}~", ch as u32).into_bytes();
            }
            if m.control && text.len() == 1 {
                let c = text.as_bytes()[0];
                let control = match c {
                    b' ' | b'@' | b'2' => Some(0),
                    b'a'..=b'z' => Some(c - b'a' + 1),
                    b'A'..=b'Z' => Some(c - b'A' + 1),
                    b'[' | b'3' => Some(27),
                    b'\\' | b'4' => Some(28),
                    b']' | b'5' => Some(29),
                    b'^' | b'6' => Some(30),
                    b'_' | b'/' | b'7' => Some(31),
                    b'?' | b'8' => Some(127),
                    _ => None,
                };
                control
                    .map(|c| vec![c])
                    .unwrap_or_else(|| text.as_bytes().to_vec())
            } else {
                text.as_bytes().to_vec()
            }
        }
        Key::Enter => {
            if modes.newline {
                b"\r\n".to_vec()
            } else {
                vec![b'\r']
            }
        }
        Key::Tab => {
            if m.shift {
                b"\x1b[Z".to_vec()
            } else {
                vec![b'\t']
            }
        }
        Key::Backspace => vec![if m.control { 8 } else { 127 }],
        Key::Escape => vec![27],
        Key::Keypad(key) => {
            let (plain, application) = keypad(*key);
            if modes.application_keypad {
                if modifier > 1 {
                    format!("\x1b[1;{modifier}{application}").into_bytes()
                } else {
                    format!("\x1bO{application}").into_bytes()
                }
            } else {
                vec![plain as u8]
            }
        }
        key => {
            let Some((number, suffix)) = functional(key) else {
                return Vec::new();
            };
            if modifier > 1 {
                format!("\x1b[{number};{modifier}{suffix}").into_bytes()
            } else if suffix == '~' || suffix == 'u' {
                format!("\x1b[{number}{suffix}").into_bytes()
            } else if matches!(key, Key::Function(1..=4))
                || modes.application_cursor
                    && matches!(
                        key,
                        Key::ArrowUp
                            | Key::ArrowDown
                            | Key::ArrowLeft
                            | Key::ArrowRight
                            | Key::Home
                            | Key::End
                    )
            {
                format!("\x1bO{suffix}").into_bytes()
            } else {
                format!("\x1b[{suffix}").into_bytes()
            }
        }
    };
    // Modifiers in functional sequences have already encoded Alt.
    if m.alt
        && !(modes.application_keypad && matches!(event.key, Key::Keypad(_)))
        && matches!(
            event.key,
            Key::Character(_)
                | Key::Enter
                | Key::Tab
                | Key::Backspace
                | Key::Escape
                | Key::Keypad(_)
        )
    {
        output.insert(0, 27);
    }
    output
}

fn functional(key: &Key) -> Option<(u32, char)> {
    Some(match key {
        Key::Escape => (27, 'u'),
        Key::Enter => (13, 'u'),
        Key::Tab => (9, 'u'),
        Key::Backspace => (127, 'u'),
        Key::ArrowUp => (1, 'A'),
        Key::ArrowDown => (1, 'B'),
        Key::ArrowRight => (1, 'C'),
        Key::ArrowLeft => (1, 'D'),
        Key::Home => (1, 'H'),
        Key::End => (1, 'F'),
        Key::Insert => (2, '~'),
        Key::Delete => (3, '~'),
        Key::PageUp => (5, '~'),
        Key::PageDown => (6, '~'),
        Key::Function(n @ 1..=4) => (
            1,
            match n {
                1 => 'P',
                2 => 'Q',
                3 => 'R',
                _ => 'S',
            },
        ),
        Key::Function(n @ 5..=12) => ([15, 17, 18, 19, 20, 21, 23, 24][(*n - 5) as usize], '~'),
        Key::Function(n @ 13..=35) => (57376 + u32::from(*n - 13), 'u'),
        Key::Keypad(key) => (
            match key {
                KeypadKey::Digit(n) if *n <= 9 => 57399 + u32::from(*n),
                KeypadKey::Decimal => 57409,
                KeypadKey::Divide => 57410,
                KeypadKey::Multiply => 57411,
                KeypadKey::Subtract => 57412,
                KeypadKey::Add => 57413,
                KeypadKey::Enter => 57414,
                KeypadKey::Equal => 57415,
                _ => return None,
            },
            'u',
        ),
        _ => return None,
    })
}
fn keypad(key: KeypadKey) -> (char, char) {
    match key {
        KeypadKey::Digit(n @ 0..=9) => ((b'0' + n) as char, (b'p' + n) as char),
        KeypadKey::Decimal => ('.', 'n'),
        KeypadKey::Divide => ('/', 'o'),
        KeypadKey::Multiply => ('*', 'j'),
        KeypadKey::Subtract => ('-', 'm'),
        KeypadKey::Add => ('+', 'k'),
        KeypadKey::Enter => ('\r', 'M'),
        KeypadKey::Equal => ('=', 'X'),
        _ => (' ', ' '),
    }
}
fn kitty(event: &KeyEvent, flags: u8) -> Vec<u8> {
    let (number, suffix) = match &event.key {
        Key::Character(text) => {
            let Some(c) = event
                .base_key
                .or_else(|| text.chars().next().and_then(|c| c.to_lowercase().next()))
            else {
                return Vec::new();
            };
            (c as u32, 'u')
        }
        Key::Function(3) => (13, '~'),
        key => {
            let Some(value) = functional(key) else {
                return Vec::new();
            };
            value
        }
    };
    let mut key = number.to_string();
    if flags & 4 != 0 && suffix == 'u' {
        if event.shifted_key.is_some() || event.base_layout_key.is_some() {
            key.push(':');
        }
        if let Some(shifted) = event.shifted_key {
            key.push_str(&(shifted as u32).to_string());
        }
        if let Some(base) = event.base_layout_key {
            key.push_str(&format!(":{}", base as u32));
        }
    }
    let mut modifier = event.modifiers.parameter(true).to_string();
    if flags & 2 != 0 && event.kind != KeyEventKind::Press {
        modifier.push_str(match event.kind {
            KeyEventKind::Repeat => ":2",
            KeyEventKind::Release => ":3",
            _ => "",
        });
    }
    let text = if flags & 16 != 0 && event.kind != KeyEventKind::Release {
        event
            .text
            .as_deref()
            .or({
                if let Key::Character(text) = &event.key {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .filter(|_| !event.modifiers.control && !event.modifiers.super_key)
    } else {
        None
    };
    let mut output = format!("\x1b[{key}");
    if modifier != "1" || text.is_some() {
        output.push(';');
        output.push_str(&modifier);
    }
    if let Some(text) = text {
        output.push(';');
        output.push_str(
            &text
                .chars()
                .map(|c| (c as u32).to_string())
                .collect::<Vec<_>>()
                .join(":"),
        );
    }
    output.push(suffix);
    output.into_bytes()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    None,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub position: CellPoint,
    /// The pressed/held button, or None for motion without a button.
    pub button: MouseButton,
    pub kind: MouseEventKind,
    pub modifiers: Modifiers,
}
pub fn encode_mouse(modes: Modes, event: MouseEvent) -> Vec<u8> {
    if modes.mouse == MouseTracking::None
        || modes.mouse == MouseTracking::X10 && event.kind != MouseEventKind::Press
        || event.kind == MouseEventKind::Move
            && (matches!(modes.mouse, MouseTracking::Normal | MouseTracking::X10)
                || modes.mouse == MouseTracking::Button && event.button == MouseButton::None)
    {
        return Vec::new();
    }
    let mut button = match event.button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::None => 3,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
        MouseButton::WheelLeft => 66,
        MouseButton::WheelRight => 67,
    };
    if event.kind == MouseEventKind::Release && !modes.sgr_mouse {
        button = 3;
    }
    if event.kind == MouseEventKind::Move {
        button += 32;
    }
    if modes.mouse != MouseTracking::X10 {
        button += 4 * u16::from(event.modifiers.shift)
            + 8 * u16::from(event.modifiers.alt)
            + 16 * u16::from(event.modifiers.control);
    }
    let x = u32::from(event.position.col) + 1;
    let y = u32::from(event.position.row) + 1;
    if modes.sgr_mouse {
        return format!(
            "\x1b[<{button};{x};{y}{}",
            if event.kind == MouseEventKind::Release {
                'm'
            } else {
                'M'
            }
        )
        .into_bytes();
    }
    if !modes.utf8_mouse && (x > 223 || y > 223) {
        return Vec::new();
    }
    let mut bytes = b"\x1b[M".to_vec();
    for value in [u32::from(button) + 32, x + 32, y + 32] {
        if modes.utf8_mouse {
            let Some(c) = char::from_u32(value) else {
                return Vec::new();
            };
            bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
        } else {
            bytes.push(value as u8);
        }
    }
    bytes
}
pub fn encode_paste(modes: Modes, text: &str) -> Vec<u8> {
    let text = text.replace('\x1b', "");
    if modes.bracketed_paste {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r").into_bytes()
    }
}
pub fn encode_focus(modes: Modes, focused: bool) -> Vec<u8> {
    if modes.focus_events {
        if focused {
            b"\x1b[I".to_vec()
        } else {
            b"\x1b[O".to_vec()
        }
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(key: Key, modifiers: Modifiers, modes: Modes) -> Vec<u8> {
        encode_key(modes, &KeyEvent::new(key, modifiers))
    }
    #[test]
    fn xterm_and_application_keys() {
        let ctrl = Modifiers {
            control: true,
            ..Default::default()
        };
        assert_eq!(
            key(Key::Character("c".into()), ctrl, Modes::default()),
            vec![3]
        );
        assert_eq!(
            key(Key::ArrowUp, Modifiers::default(), Modes::default()),
            b"\x1b[A"
        );
        assert_eq!(
            key(
                Key::ArrowUp,
                Modifiers::default(),
                Modes {
                    application_cursor: true,
                    ..Default::default()
                }
            ),
            b"\x1bOA"
        );
        assert_eq!(key(Key::ArrowLeft, ctrl, Modes::default()), b"\x1b[1;5D");
        assert_eq!(
            key(
                Key::Function(5),
                Modifiers {
                    shift: true,
                    alt: true,
                    ..Default::default()
                },
                Modes::default()
            ),
            b"\x1b[15;4~"
        );
        assert_eq!(
            key(
                Key::Keypad(KeypadKey::Digit(2)),
                Modifiers::default(),
                Modes {
                    application_keypad: true,
                    ..Default::default()
                }
            ),
            b"\x1bOr"
        );
        assert_eq!(
            key(
                Key::Character("é".into()),
                Modifiers {
                    alt: true,
                    ..Default::default()
                },
                Modes::default()
            ),
            "\x1bé".as_bytes()
        );
        assert_eq!(
            key(
                Key::Keypad(KeypadKey::Digit(2)),
                Modifiers {
                    control: true,
                    alt: true,
                    ..Default::default()
                },
                Modes {
                    application_keypad: true,
                    ..Default::default()
                }
            ),
            b"\x1b[1;7r"
        );
    }
    #[test]
    fn kitty_progressive_flags() {
        let modes = Modes {
            kitty_flags: 1,
            ..Default::default()
        };
        assert_eq!(
            key(
                Key::Character("c".into()),
                Modifiers {
                    control: true,
                    ..Default::default()
                },
                modes
            ),
            b"\x1b[99;5u"
        );
        assert_eq!(key(Key::Escape, Modifiers::default(), modes), b"\x1b[27u");
        assert_eq!(
            key(
                Key::Character("A".into()),
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
                modes
            ),
            b"A"
        );
        let mut event = KeyEvent::new(Key::ArrowUp, Modifiers::default());
        event.kind = KeyEventKind::Release;
        assert_eq!(
            encode_key(
                Modes {
                    kitty_flags: 3,
                    ..Default::default()
                },
                &event
            ),
            b"\x1b[1;1:3A"
        );
        assert!(encode_key(modes, &event).is_empty());
        let mut event = KeyEvent::new(
            Key::Character("A".into()),
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        event.base_key = Some('a');
        event.shifted_key = Some('A');
        event.text = Some("A".into());
        assert_eq!(
            encode_key(
                Modes {
                    kitty_flags: 31,
                    ..Default::default()
                },
                &event
            ),
            b"\x1b[97:65;2;65u"
        );
        event.base_layout_key = Some('q');
        assert_eq!(
            encode_key(
                Modes {
                    kitty_flags: 31,
                    ..Default::default()
                },
                &event
            ),
            b"\x1b[97:65:113;2;65u"
        );
        assert_eq!(
            key(Key::Function(3), Modifiers::default(), modes),
            b"\x1b[13~"
        );
    }
    #[test]
    fn mouse_modes_and_coordinates() {
        let event = MouseEvent {
            position: CellPoint { row: 2, col: 4 },
            button: MouseButton::Left,
            kind: MouseEventKind::Press,
            modifiers: Modifiers::default(),
        };
        let modes = Modes {
            mouse: MouseTracking::Normal,
            sgr_mouse: true,
            ..Default::default()
        };
        assert_eq!(encode_mouse(modes, event), b"\x1b[<0;5;3M");
        assert_eq!(
            encode_mouse(
                modes,
                MouseEvent {
                    kind: MouseEventKind::Release,
                    ..event
                }
            ),
            b"\x1b[<0;5;3m"
        );
        assert!(
            encode_mouse(
                Modes {
                    mouse: MouseTracking::X10,
                    ..modes
                },
                MouseEvent {
                    kind: MouseEventKind::Release,
                    ..event
                }
            )
            .is_empty()
        );
        assert!(
            encode_mouse(
                Modes {
                    mouse: MouseTracking::Button,
                    ..modes
                },
                MouseEvent {
                    kind: MouseEventKind::Move,
                    button: MouseButton::None,
                    ..event
                }
            )
            .is_empty()
        );
        assert_eq!(
            encode_mouse(
                Modes {
                    mouse: MouseTracking::Any,
                    ..modes
                },
                MouseEvent {
                    kind: MouseEventKind::Move,
                    button: MouseButton::None,
                    ..event
                }
            ),
            b"\x1b[<35;5;3M"
        );
        assert_eq!(
            encode_mouse(
                Modes {
                    sgr_mouse: false,
                    ..modes
                },
                event
            ),
            b"\x1b[M %#"
        );
    }
    #[test]
    fn paste_and_focus() {
        assert_eq!(
            encode_paste(
                Modes {
                    bracketed_paste: true,
                    ..Default::default()
                },
                "a\x1bb\n"
            ),
            b"\x1b[200~ab\n\x1b[201~"
        );
        assert_eq!(encode_paste(Modes::default(), "a\r\nb\n"), b"a\rb\r");
        assert_eq!(
            encode_focus(
                Modes {
                    focus_events: true,
                    ..Default::default()
                },
                true
            ),
            b"\x1b[I"
        );
    }
}
