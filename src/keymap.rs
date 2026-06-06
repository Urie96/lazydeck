use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mlua::prelude::*;
use std::cmp::Reverse;

use crate::Mode;

pub struct Keymap {
    pub mode: Mode,
    pub raw_key: String,
    pub key_sequence: KeySequence,
    pub callback: LuaFunction,
    pub desc: Option<String>,
    pub path: Option<KeymapPathPattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeymapPathPattern {
    raw: Vec<String>,
    segments: Vec<PathSegmentPattern>,
    priority: KeymapPathPriority,
}

#[derive(Debug, Clone, PartialEq)]
enum PathSegmentPattern {
    Exact(String),
    Star,
    GlobStar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeymapPathPriority {
    globstar_count: usize,
    star_count: usize,
    exact_count: Reverse<usize>,
    len: Reverse<usize>,
}

impl KeymapPathPattern {
    pub fn new(raw: Vec<String>) -> Self {
        Self::from_segments(raw)
    }

    pub fn from_path_str(raw: &str) -> Self {
        let segments = raw
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect();
        Self::from_segments(segments)
    }

    fn from_segments(raw: Vec<String>) -> Self {
        let mut exact_count = 0;
        let mut star_count = 0;
        let mut globstar_count = 0;
        let segments = raw
            .iter()
            .map(|segment| match segment.as_str() {
                "*" => {
                    star_count += 1;
                    PathSegmentPattern::Star
                }
                "**" => {
                    globstar_count += 1;
                    PathSegmentPattern::GlobStar
                }
                _ => {
                    exact_count += 1;
                    PathSegmentPattern::Exact(segment.clone())
                }
            })
            .collect();

        Self {
            priority: KeymapPathPriority {
                globstar_count,
                star_count,
                exact_count: Reverse(exact_count),
                len: Reverse(raw.len()),
            },
            raw,
            segments,
        }
    }

    pub fn raw(&self) -> &[String] {
        &self.raw
    }

    pub fn priority(&self) -> KeymapPathPriority {
        self.priority
    }

    pub fn matches(&self, path: &[String]) -> bool {
        self.matches_from(0, 0, path)
    }

    fn matches_from(&self, pattern_idx: usize, path_idx: usize, path: &[String]) -> bool {
        if pattern_idx == self.segments.len() {
            return path_idx == path.len();
        }

        match &self.segments[pattern_idx] {
            PathSegmentPattern::Exact(segment) => {
                path.get(path_idx) == Some(segment)
                    && self.matches_from(pattern_idx + 1, path_idx + 1, path)
            }
            PathSegmentPattern::Star => {
                path_idx < path.len() && self.matches_from(pattern_idx + 1, path_idx + 1, path)
            }
            PathSegmentPattern::GlobStar => {
                (path_idx..=path.len()).any(|idx| self.matches_from(pattern_idx + 1, idx, path))
            }
        }
    }
}

impl From<Vec<String>> for KeymapPathPattern {
    fn from(raw: Vec<String>) -> Self {
        Self::new(raw)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeySequence(Vec<KeyEvent>);

impl KeySequence {
    pub fn prefix_match(&self, events: &[KeyEvent]) -> bool {
        self.0.len() >= events.len()
            && self
                .0
                .iter()
                .zip(events.iter())
                .all(|(expected, actual)| key_events_match(expected, actual))
    }

    pub fn all_match(&self, events: &[KeyEvent]) -> bool {
        self.0.len() == events.len()
            && self
                .0
                .iter()
                .zip(events.iter())
                .all(|(expected, actual)| key_events_match(expected, actual))
    }
}

fn is_ctrl_i(event: &KeyEvent) -> bool {
    event.code == KeyCode::Char('i')
        && event.modifiers.contains(KeyModifiers::CONTROL)
        && !event.modifiers.contains(KeyModifiers::ALT)
}

fn is_legacy_ctrl_i(event: &KeyEvent) -> bool {
    matches!(event.code, KeyCode::Tab | KeyCode::Char('\t'))
        && (event.modifiers.is_empty()
            || (event.modifiers.contains(KeyModifiers::CONTROL)
                && !event.modifiers.contains(KeyModifiers::ALT)))
}

fn key_events_match(expected: &KeyEvent, actual: &KeyEvent) -> bool {
    if expected.code == actual.code && expected.modifiers == actual.modifiers {
        return true;
    }

    // In legacy terminal input, Ctrl-I and Tab are encoded identically. Newer
    // terminals may disambiguate them, but without that support crossterm sees
    // Ctrl-I as Tab. Treat Tab as a fallback only when the configured keymap is
    // Ctrl-I so the default history-forward shortcut works across terminals.
    is_ctrl_i(expected) && is_legacy_ctrl_i(actual)
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
    fn test_ctrl_i_matches_legacy_tab_event() {
        let keyseq = KeySequence::from("<C-i>");
        assert!(keyseq.all_match(&[KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::empty()
        )]));
        assert!(keyseq.all_match(&[KeyEvent::new(
            KeyCode::Char('\t'),
            KeyModifiers::empty()
        )]));
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

    #[test]
    fn path_pattern_star_matches_one_segment() {
        let pattern = KeymapPathPattern::new(vec!["mail".into(), "*".into()]);

        assert!(pattern.matches(&["mail".into(), "inbox".into()]));
        assert!(!pattern.matches(&["mail".into()]));
        assert!(!pattern.matches(&["mail".into(), "inbox".into(), "thread".into()]));
    }

    #[test]
    fn path_pattern_globstar_matches_zero_or_more_segments() {
        let pattern = KeymapPathPattern::new(vec!["mail".into(), "**".into()]);

        assert!(pattern.matches(&["mail".into()]));
        assert!(pattern.matches(&["mail".into(), "inbox".into()]));
        assert!(pattern.matches(&["mail".into(), "inbox".into(), "thread".into()]));
        assert!(!pattern.matches(&["docker".into(), "inbox".into()]));
    }

    #[test]
    fn path_pattern_priority_prefers_more_specific_patterns() {
        let exact = KeymapPathPattern::new(vec!["mail".into(), "inbox".into()]);
        let star = KeymapPathPattern::new(vec!["mail".into(), "*".into()]);
        let globstar = KeymapPathPattern::new(vec!["mail".into(), "**".into()]);

        assert!(exact.priority() < star.priority());
        assert!(star.priority() < globstar.priority());
    }
}
