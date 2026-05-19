use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mlua::prelude::*;

use crate::Mode;

pub struct Keymap {
    pub mode: Mode,
    pub raw_key: String,
    pub key_sequence: KeySequence,
    pub callback: LuaFunction,
    pub desc: Option<String>,
    pub once: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeySequence(Vec<KeyEvent>);

impl KeySequence {
    pub fn prefix_match(&self, events: &[KeyEvent]) -> bool {
        self.0.len() >= events.len() && &self.0[..events.len()] == events
    }

    pub fn all_match(&self, events: &[KeyEvent]) -> bool {
        self.0 == events
    }
}

impl From<&str> for KeySequence {
    fn from(raw: &str) -> Self {
        let mut events = Vec::new();
        let mut current = raw.trim();

        while !current.is_empty() {
            if current.starts_with('<') {
                // Find the closing '>'
                if let Some(end) = current.find('>') {
                    let bracket_content = &current[1..end];
                    let keyseq = parse_angle_bracket_notation(bracket_content);
                    events.extend(keyseq.0);
                    current = current[end + 1..].trim();
                } else {
                    break;
                }
            } else {
                // Parse single character (anything outside angle brackets is treated as individual chars)
                let (key_part, modifiers) = extract_modifiers(current);

                // Only parse as a single character
                let key_char = key_part.chars().next();
                if let Some(c) = key_char {
                    events.push(KeyEvent::new(KeyCode::Char(c), modifiers));
                    current = &current[1 + (current.len() - key_part.len())..].trim();
                } else {
                    break;
                }
            }
        }

        Self(events)
    }
}

fn parse_angle_bracket_notation(inner: &str) -> KeySequence {
    let (remaining, mut modifiers) = extract_modifiers(inner);
    let c = match remaining.to_lowercase().as_str() {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Char(' '),
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        // Treat remaining as single character
        c => {
            let char = c.chars().next().unwrap_or(' ');
            KeyCode::Char(char)
        }
    };
    KeySequence(vec![KeyEvent::new(c, modifiers)])
}

fn extract_modifiers(raw: &str) -> (&str, KeyModifiers) {
    let mut modifiers = KeyModifiers::empty();
    let mut current = raw;

    loop {
        match current {
            rest if rest.to_lowercase().starts_with("ctrl-") || rest.starts_with("C-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                current = if rest.starts_with("C-") {
                    &rest[2..]
                } else {
                    &rest[5..]
                };
            }
            rest if rest.to_lowercase().starts_with("alt-") || rest.starts_with("A-") => {
                modifiers.insert(KeyModifiers::ALT);
                current = if rest.starts_with("A-") {
                    &rest[2..]
                } else {
                    &rest[4..]
                };
            }
            rest if rest.to_lowercase().starts_with("shift-") || rest.starts_with("S-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                current = if rest.starts_with("S-") {
                    &rest[2..]
                } else {
                    &rest[6..]
                };
            }
            _ => break, // break out of the loop if no known prefix is detected
        };
    }

    (current, modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctrl_d() {
        let keyseq = KeySequence::from("ctrl-d");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('d'));
    }

    #[test]
    fn test_ctrl_x_angle_bracket() {
        let keyseq = KeySequence::from("<C-x>");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('x'));
    }

    #[test]
    fn test_ctrl_c() {
        let keyseq = KeySequence::from("ctrl-c");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('c'));
    }

    #[test]
    fn test_ctrl_i_angle_bracket() {
        let keyseq = KeySequence::from("<C-i>");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('i'));
    }

    #[test]
    fn test_alt_k_angle_bracket() {
        let keyseq = KeySequence::from("<A-k>");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::ALT));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('k'));
    }

    #[test]
    fn test_alt_a() {
        let keyseq = KeySequence::from("alt-a");
        assert_eq!(keyseq.0.len(), 1);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::ALT));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('a'));
    }

    #[test]
    fn test_simple_char() {
        let keyseq = KeySequence::from("x");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].modifiers, KeyModifiers::empty());
        assert_eq!(keyseq.0[0].code, KeyCode::Char('x'));
    }

    #[test]
    fn test_double_d() {
        let keyseq = KeySequence::from("dd");
        assert_eq!(keyseq.0.len(), 2);
        assert_eq!(keyseq.0[0].code, KeyCode::Char('d'));
        assert_eq!(keyseq.0[1].code, KeyCode::Char('d'));
        assert!(!keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert!(!keyseq.0[1].modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_down_as_four_chars() {
        let keyseq = KeySequence::from("down");
        assert_eq!(keyseq.0.len(), 4);
        assert_eq!(keyseq.0[0].code, KeyCode::Char('d'));
        assert_eq!(keyseq.0[1].code, KeyCode::Char('o'));
        assert_eq!(keyseq.0[2].code, KeyCode::Char('w'));
        assert_eq!(keyseq.0[3].code, KeyCode::Char('n'));
    }

    #[test]
    fn test_down_angle_bracket() {
        let keyseq = KeySequence::from("<down>");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].code, KeyCode::Down);
    }

    #[test]
    fn test_up_angle_bracket() {
        let keyseq = KeySequence::from("<up>");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].code, KeyCode::Up);
    }

    #[test]
    fn test_double_w_angle_bracket() {
        let keyseq = KeySequence::from("ww");
        assert_eq!(keyseq.0.len(), 2);
        assert_eq!(keyseq.0[0].code, KeyCode::Char('w'));
        assert_eq!(keyseq.0[1].code, KeyCode::Char('w'));
    }

    #[test]
    fn test_mixed_sequence() {
        let keyseq = KeySequence::from("<C-x><C-c>");
        assert_eq!(keyseq.0.len(), 2);
        assert!(keyseq.0[0].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[0].code, KeyCode::Char('x'));
        assert!(keyseq.0[1].modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(keyseq.0[1].code, KeyCode::Char('c'));
    }

    #[test]
    fn test_space() {
        let keyseq = KeySequence::from("<space>");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].code, KeyCode::Char(' '));
    }

    #[test]
    fn test_f_key() {
        let keyseq = KeySequence::from("<f5>");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].code, KeyCode::F(5));
    }

    #[test]
    fn test_enter() {
        let keyseq = KeySequence::from("<enter>");
        assert_eq!(keyseq.0.len(), 1);
        assert_eq!(keyseq.0[0].code, KeyCode::Enter);
    }
}
