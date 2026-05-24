use anyhow::Context;
use crossterm::event::KeyEvent;
use mlua::prelude::*;
use ratatui::{text::Line, text::Text, widgets::ListState};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    widgets::Renderable, KeySequence, Keymap, KeymapPathPattern, KeymapPathPriority, Mode, Page,
    PageEntry,
};

/// Represents which button is selected in the confirm dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmButton {
    Yes,
    No,
}

/// Option for select dialog
#[derive(Debug, Clone)]
pub struct SelectOption {
    /// The value to return when this option is selected
    pub value: LuaValue,
    /// The text to display for this option
    pub display: Line<'static>,
}

/// State for the select dialog
#[derive(Debug)]
pub struct SelectDialog {
    /// Optional prompt/title text
    pub prompt: Option<String>,
    /// All available options (unfiltered)
    pub options: Vec<SelectOption>,
    /// Filtered options (subset of options that match the filter)
    pub filtered_options: Vec<usize>,
    /// Index of currently selected option in filtered_options
    pub selected_index: Option<usize>,
    /// Current filter input text
    pub filter_input: String,
    /// Cursor position in the filter input
    pub cursor_position: usize,
    /// Cursor x position (for terminal cursor display)
    pub cursor_x: u16,
    /// Cursor y position (for terminal cursor display)
    pub cursor_y: u16,
    /// List state for rendering and scrolling
    pub list_state: ListState,
    /// Callback function when selection is made (or canceled with nil)
    pub on_selection: LuaFunction,
}

#[derive(Clone)]
pub struct AvailableKeymap {
    pub key: String,
    pub desc: Option<String>,
    pub callback: LuaFunction,
    pub source: &'static str,
    path_priority: Option<KeymapPathPriority>,
}

fn keymap_path_matches(keymap_path: Option<&KeymapPathPattern>, current_path: &[String]) -> bool {
    match keymap_path {
        Some(path) => path.matches(current_path),
        None => true,
    }
}

fn resolve_entry_keymap_value(
    hovered_key: &str,
    key: &str,
    value: LuaValue,
) -> anyhow::Result<(LuaFunction, Option<String>)> {
    match value {
        LuaValue::Function(f) => Ok((f, None)),
        LuaValue::Table(tbl) => {
            let callback = match tbl
                .get::<LuaValue>("callback")
                .map_err(anyhow::Error::from)?
            {
                LuaValue::Function(f) => f,
                other => {
                    return Err(anyhow::anyhow!(
                        "entry '{}' keymap callback for '{}' must be a function, got {}",
                        hovered_key,
                        key,
                        other.type_name()
                    ));
                }
            };
            let desc = tbl
                .get::<Option<String>>("desc")
                .map_err(anyhow::Error::from)?;
            Ok((callback, desc))
        }
        other => Err(anyhow::anyhow!(
            "entry '{}' keymap callback for '{}' must be a function or table, got {}",
            hovered_key,
            key,
            other.type_name()
        )),
    }
}

impl SelectDialog {
    pub fn new(
        prompt: Option<String>,
        options: Vec<SelectOption>,
        on_selection: LuaFunction,
    ) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let mut dialog = Self {
            prompt,
            options,
            filtered_options: Vec::new(),
            selected_index: Some(0),
            filter_input: String::new(),
            cursor_position: 0,
            cursor_x: 0,
            cursor_y: 0,
            list_state,
            on_selection,
        };

        // Initialize filtered options
        dialog.update_filtered_options();

        dialog
    }

    /// Get the current filtered options
    pub fn get_filtered_options(&self) -> Vec<SelectOption> {
        self.filtered_options
            .iter()
            .filter_map(|&idx| self.options.get(idx).cloned())
            .collect()
    }

    /// Update filtered options based on current filter input
    pub fn update_filtered_options(&mut self) {
        if self.filter_input.is_empty() {
            // No filter: show all options
            self.filtered_options = (0..self.options.len()).collect();
        } else {
            // Filter options by display text (case-insensitive)
            let filter_lower = self.filter_input.to_lowercase();
            self.filtered_options = self
                .options
                .iter()
                .enumerate()
                .filter(|(_, opt)| {
                    opt.display
                        .to_string()
                        .to_lowercase()
                        .contains(&filter_lower)
                })
                .map(|(idx, _)| idx)
                .collect();
        }

        // Adjust selected index
        if self.filtered_options.is_empty() {
            self.selected_index = None;
            self.list_state.select(None);
        } else {
            // Keep current selection if valid, otherwise select first
            let new_idx = self.selected_index.and_then(|idx| {
                if idx < self.filtered_options.len() {
                    Some(idx)
                } else {
                    None
                }
            });

            self.selected_index = new_idx.or(Some(0));
            self.list_state.select(self.selected_index);
        }
    }

    /// Move selection by delta
    pub fn move_selection(&mut self, delta: i32) {
        let filtered_count = self.filtered_options.len();
        if filtered_count == 0 {
            return;
        }

        let current = self.selected_index.unwrap_or(0);
        let new = if delta > 0 {
            // Moving down: wrap around if at end
            let target = current + delta as usize;
            if target >= filtered_count && current == filtered_count - 1 {
                0 // Wrap to top
            } else {
                target.min(filtered_count - 1)
            }
        } else {
            // Moving up: wrap around if at top
            let abs_delta = delta.unsigned_abs() as usize;
            if abs_delta > current && current == 0 {
                filtered_count - 1 // Wrap to bottom
            } else {
                current.saturating_sub(abs_delta)
            }
        };

        self.selected_index = Some(new);
        self.list_state.select(Some(new));
    }

    /// Select first option
    pub fn select_first(&mut self) {
        if !self.filtered_options.is_empty() {
            self.selected_index = Some(0);
            self.list_state.select(Some(0));
        }
    }

    /// Select last option
    pub fn select_last(&mut self) {
        let count = self.filtered_options.len();
        if count > 0 {
            self.selected_index = Some(count - 1);
            self.list_state.select(Some(count - 1));
        }
    }

    /// Move cursor to start of input
    pub fn cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to end of input
    pub fn cursor_to_end(&mut self) {
        self.cursor_position = self.filter_input.len();
    }

    /// Move cursor left by one character (not byte)
    pub fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous character boundary
            let prev_pos = self.filter_input[..self.cursor_position]
                .char_indices()
                .rev()
                .nth(0)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.cursor_position = prev_pos;
        }
    }

    /// Move cursor right by one character (not byte)
    pub fn cursor_right(&mut self) {
        if self.cursor_position < self.filter_input.len() {
            // Find the next character boundary (skip the character at current position)
            let next_pos = self.filter_input[self.cursor_position..]
                .char_indices()
                .nth(1)
                .map(|(idx, _)| self.cursor_position + idx)
                .unwrap_or(self.filter_input.len());
            self.cursor_position = next_pos;
        }
    }

    /// Insert character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.filter_input.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Delete character before cursor (backspace)
    pub fn delete_before_cursor(&mut self) {
        if self.cursor_position > 0 {
            // Find the start of the character before cursor
            let char_start = self.filter_input[..self.cursor_position]
                .char_indices()
                .rev()
                .nth(0)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.filter_input.remove(char_start);
            self.cursor_position = char_start;
        }
    }

    /// Delete character at cursor (delete)
    pub fn delete_at_cursor(&mut self) {
        if self.cursor_position < self.filter_input.len() {
            self.filter_input.remove(self.cursor_position);
        }
    }

    /// Delete all characters before cursor (ctrl-u)
    pub fn delete_before_cursor_all(&mut self) {
        if self.cursor_position > 0 {
            self.filter_input = self.filter_input[self.cursor_position..].to_string();
            self.cursor_position = 0;
            self.update_filtered_options();
        }
    }
}

impl ConfirmButton {
    pub fn toggle(&self) -> Self {
        match self {
            ConfirmButton::Yes => ConfirmButton::No,
            ConfirmButton::No => ConfirmButton::Yes,
        }
    }
}

/// State for the input dialog
#[derive(Debug)]
pub struct InputDialog {
    pub previous_mode: Mode,
    pub prompt: String,
    pub placeholder: String,
    pub text: String,
    pub cursor_position: usize,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub on_submit: LuaFunction,
    pub on_cancel: LuaFunction,
    pub on_change: LuaFunction,
}

impl InputDialog {
    fn prev_char_boundary(text: &str, cursor_position: usize) -> usize {
        text[..cursor_position]
            .char_indices()
            .next_back()
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn next_char_boundary(text: &str, cursor_position: usize) -> usize {
        text[cursor_position..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| cursor_position + idx)
            .unwrap_or(text.len())
    }

    pub fn new(
        previous_mode: Mode,
        prompt: String,
        placeholder: String,
        value: String,
        on_submit: LuaFunction,
        on_cancel: LuaFunction,
        on_change: LuaFunction,
    ) -> Self {
        let cursor_position = value.len();
        Self {
            previous_mode,
            prompt,
            placeholder,
            text: value,
            cursor_position,
            cursor_x: 0,
            cursor_y: 0,
            on_submit,
            on_cancel,
            on_change,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            let prev_pos = Self::prev_char_boundary(&self.text, self.cursor_position);
            self.text.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_position < self.text.len() {
            self.text.remove(self.cursor_position);
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            let prev_pos = Self::prev_char_boundary(&self.text, self.cursor_position);
            self.cursor_position = prev_pos;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_position < self.text.len() {
            let next_pos = Self::next_char_boundary(&self.text, self.cursor_position);
            self.cursor_position = next_pos;
        }
    }

    pub fn cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    pub fn cursor_to_end(&mut self) {
        self.cursor_position = self.text.len();
    }

    pub fn clear_before_cursor(&mut self) -> bool {
        let old_text = self.text.clone();
        if self.cursor_position > 0 {
            self.text = self.text[self.cursor_position..].to_string();
            self.cursor_position = 0;
        }
        self.text != old_text
    }
}

#[cfg(test)]
mod tests {
    use super::{InputDialog, State};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use mlua::Lua;

    use crate::{KeySequence, Keymap, PageEntry};

    fn make_dialog() -> InputDialog {
        let lua = Lua::new();
        let on_submit = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_cancel = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_change = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();

        InputDialog::new(
            crate::Mode::Main,
            "Search".to_string(),
            "keyword".to_string(),
            String::new(),
            on_submit,
            on_cancel,
            on_change,
        )
    }

    #[test]
    fn input_dialog_backspace_handles_utf8() {
        let mut dialog = make_dialog();
        dialog.insert_char('搜');
        dialog.insert_char('索');

        dialog.backspace();
        assert_eq!(dialog.text, "搜");
        assert_eq!(dialog.cursor_position, '搜'.len_utf8());

        dialog.backspace();
        assert_eq!(dialog.text, "");
        assert_eq!(dialog.cursor_position, 0);
    }

    #[test]
    fn input_dialog_initial_value_places_cursor_at_end() {
        let lua = Lua::new();
        let on_submit = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_cancel = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_change = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();

        let dialog = InputDialog::new(
            crate::Mode::Main,
            "Search".to_string(),
            "keyword".to_string(),
            "abc".to_string(),
            on_submit,
            on_cancel,
            on_change,
        );

        assert_eq!(dialog.text, "abc");
        assert_eq!(dialog.cursor_position, 3);
    }

    fn make_callback(lua: &Lua, marker: &'static str) -> mlua::Function {
        lua.create_function(move |lua, ()| lua.globals().set("hit", marker))
            .unwrap()
            .to_owned()
    }

    #[test]
    fn show_and_close_input_dialog_switches_mode() {
        let lua = Lua::new();
        let on_submit = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_cancel = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();
        let on_change = lua.create_function(|_, ()| Ok(())).unwrap().to_owned();

        let mut state = State::new();
        state.show_input_dialog(
            "Search".to_string(),
            "keyword".to_string(),
            String::new(),
            on_submit,
            on_cancel,
            on_change,
        );

        assert_eq!(state.current_mode, crate::Mode::Input);
        assert!(state.input_dialog.is_some());

        let dialog = state.close_input_dialog();
        assert!(dialog.is_some());
        assert_eq!(state.current_mode, crate::Mode::Main);
        assert!(state.input_dialog.is_none());
    }

    fn make_entry(lua: &Lua, keymap: &[(&str, mlua::Function)]) -> PageEntry {
        let entry = lua.create_table().unwrap();
        entry.set("key", "item").unwrap();

        let entry_keymap = lua.create_table().unwrap();
        for (key, callback) in keymap {
            entry_keymap.set(*key, callback.clone()).unwrap();
        }
        entry.set("keymap", entry_keymap).unwrap();

        PageEntry {
            key: "item".to_string(),
            tbl: entry,
        }
    }

    fn make_entry_with_key(lua: &Lua, key: &str) -> PageEntry {
        let entry = lua.create_table().unwrap();
        entry.set("key", key).unwrap();

        PageEntry {
            key: key.to_string(),
            tbl: entry,
        }
    }

    fn make_entry_with_desc(
        lua: &Lua,
        key: &str,
        callback: mlua::Function,
        desc: &str,
    ) -> PageEntry {
        let entry = lua.create_table().unwrap();
        entry.set("key", "item").unwrap();

        let entry_keymap = lua.create_table().unwrap();
        let keymap_item = lua.create_table().unwrap();
        keymap_item.set("callback", callback).unwrap();
        keymap_item.set("desc", desc).unwrap();
        entry_keymap.set(key, keymap_item).unwrap();
        entry.set("keymap", entry_keymap).unwrap();

        PageEntry {
            key: "item".to_string(),
            tbl: entry,
        }
    }

    #[test]
    fn entry_keymap_overrides_global_keymap() {
        let lua = Lua::new();
        let mut state = State::new();
        state.set_current_page_entries(vec![make_entry(
            &lua,
            &[("x", make_callback(&lua, "entry"))],
        )]);
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "global"),
            desc: None,
            path: None,
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "entry");
    }

    #[test]
    fn global_keymap_is_used_when_entry_keymap_has_no_match() {
        let lua = Lua::new();
        let mut state = State::new();
        state.set_current_page_entries(vec![make_entry(
            &lua,
            &[("y", make_callback(&lua, "entry"))],
        )]);
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "global"),
            desc: None,
            path: None,
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "global");
    }

    #[test]
    fn entry_prefix_match_blocks_global_shortcut() {
        let lua = Lua::new();
        let mut state = State::new();
        state.set_current_page_entries(vec![make_entry(
            &lua,
            &[("gg", make_callback(&lua, "entry"))],
        )]);
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "g".to_string(),
            key_sequence: KeySequence::from("g"),
            callback: make_callback(&lua, "global"),
            desc: None,
            path: None,
        });

        let first = state
            .tap_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()))
            .unwrap();
        assert!(first.is_none());
        assert_eq!(state.last_key_event_buffer.len(), 1);

        let second = state
            .tap_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();
        second.call::<()>(()).unwrap();

        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "entry");
        assert!(state.last_key_event_buffer.is_empty());
    }

    #[test]
    fn scrolling_clears_pending_key_sequence() {
        let lua = Lua::new();
        let mut state = State::new();
        state.set_current_page_entries(vec![make_entry(
            &lua,
            &[("gg", make_callback(&lua, "entry"))],
        )]);

        let first = state
            .tap_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()))
            .unwrap();
        assert!(first.is_none());
        assert_eq!(state.last_key_event_buffer.len(), 1);

        state.scroll_by(1);
        assert!(state.last_key_event_buffer.is_empty());
    }

    #[test]
    fn available_keymaps_include_entry_page_and_global_desc() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["docker".to_string(), "container".to_string()];
        state.set_current_page_entries(vec![make_entry_with_desc(
            &lua,
            "x",
            make_callback(&lua, "entry"),
            "play song",
        )]);
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "p".to_string(),
            key_sequence: KeySequence::from("p"),
            callback: make_callback(&lua, "page"),
            desc: Some("page action".to_string()),
            path: Some(vec!["docker".to_string(), "container".to_string()].into()),
        });
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "q".to_string(),
            key_sequence: KeySequence::from("q"),
            callback: make_callback(&lua, "global"),
            desc: Some("quit".to_string()),
            path: None,
        });

        let keymaps = state.available_keymaps().unwrap();

        assert_eq!(keymaps.len(), 3);
        assert_eq!(keymaps[0].source, "entry");
        assert_eq!(keymaps[0].key, "x");
        assert_eq!(keymaps[0].desc.as_deref(), Some("play song"));
        assert_eq!(keymaps[1].source, "page");
        assert_eq!(keymaps[1].key, "p");
        assert_eq!(keymaps[1].desc.as_deref(), Some("page action"));
        assert_eq!(keymaps[2].source, "global");
        assert_eq!(keymaps[2].key, "q");
        assert_eq!(keymaps[2].desc.as_deref(), Some("quit"));
    }

    #[test]
    fn page_keymap_overrides_global_keymap() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["docker".to_string(), "container".to_string()];
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "global"),
            desc: None,
            path: None,
        });
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "page"),
            desc: Some("page".to_string()),
            path: Some(vec!["docker".to_string(), "container".to_string()].into()),
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "page");
    }

    #[test]
    fn exact_page_keymap_overrides_wildcard_page_keymap() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["mail".to_string(), "inbox".to_string()];
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "wildcard"),
            desc: None,
            path: Some(vec!["mail".to_string(), "*".to_string()].into()),
        });
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "exact"),
            desc: None,
            path: Some(vec!["mail".to_string(), "inbox".to_string()].into()),
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "exact");
    }

    #[test]
    fn wildcard_page_keymap_is_used_when_exact_has_no_matching_key() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["mail".to_string(), "inbox".to_string()];
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "y".to_string(),
            key_sequence: KeySequence::from("y"),
            callback: make_callback(&lua, "exact"),
            desc: None,
            path: Some(vec!["mail".to_string(), "inbox".to_string()].into()),
        });
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "wildcard"),
            desc: None,
            path: Some(vec!["mail".to_string(), "*".to_string()].into()),
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "wildcard");
    }

    #[test]
    fn page_keymap_is_overwritten_for_same_path() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["docker".to_string(), "container".to_string()];
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "old"),
            desc: None,
            path: Some(vec!["docker".to_string(), "container".to_string()].into()),
        });
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "new"),
            desc: Some("page".to_string()),
            path: Some(vec!["docker".to_string(), "container".to_string()].into()),
        });

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "new");
    }

    #[test]
    fn entry_keymap_still_overrides_page_keymap() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["docker".to_string(), "container".to_string()];
        state.set_current_page_entries(vec![make_entry(
            &lua,
            &[("x", make_callback(&lua, "entry"))],
        )]);
        state.add_keymap(Keymap {
            mode: crate::Mode::Main,
            raw_key: "x".to_string(),
            key_sequence: KeySequence::from("x"),
            callback: make_callback(&lua, "page"),
            desc: None,
            path: Some(vec!["docker".to_string(), "container".to_string()].into()),
        });

        let keymaps = state.available_keymaps().unwrap();

        assert_eq!(keymaps.len(), 2);
        assert_eq!(keymaps[0].source, "entry");
        assert_eq!(keymaps[0].key, "x");
        assert_eq!(keymaps[1].source, "page");

        let cb = state
            .tap_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()))
            .unwrap()
            .unwrap();

        cb.call::<()>(()).unwrap();
        assert_eq!(lua.globals().get::<String>("hit").unwrap(), "entry");
    }

    #[test]
    fn set_hover_by_path_selects_matching_entry_on_current_page() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["file".to_string(), "tmp".to_string()];
        state.set_current_page_entries(vec![
            make_entry_with_key(&lua, "a"),
            make_entry_with_key(&lua, "b"),
            make_entry_with_key(&lua, "c"),
        ]);

        assert!(state.set_hover_by_path(&["file".into(), "tmp".into(), "c".into()]));
        assert_eq!(state.hovered().map(|entry| entry.key.as_str()), Some("c"));
    }

    #[test]
    fn set_hover_by_path_ignores_other_pages() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["file".to_string(), "tmp".to_string()];
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "a")]);

        assert!(!state.set_hover_by_path(&["file".into(), "other".into(), "a".into()]));
        assert_eq!(state.hovered().map(|entry| entry.key.as_str()), Some("a"));
    }

    #[test]
    fn go_to_records_history_and_pop_history_returns_previous_path() {
        let lua = Lua::new();
        let mut state = State::new();

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "github")]);
        assert!(!state.go_to(vec!["github".to_string()], true));

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "search")]);
        assert!(!state.go_to(vec!["github".to_string(), "search".to_string()], true));

        let Some(path) = state.pop_history_path() else {
            panic!("expected history entry");
        };
        assert_eq!(path, vec!["github".to_string()]);
        assert_eq!(
            state.current_path,
            vec!["github".to_string(), "search".to_string()]
        );
    }

    #[test]
    fn pop_history_path_skips_current_page_duplicates() {
        let lua = Lua::new();
        let mut state = State::new();

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "github")]);
        assert!(!state.go_to(vec!["github".to_string()], true));
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "search")]);

        assert!(state.go_to(vec!["github".to_string()], true));
        let Some(path) = state.pop_history_path() else {
            panic!("expected history entry");
        };
        assert_eq!(path, Vec::<String>::new());
        assert_eq!(state.current_path, vec!["github".to_string()]);
    }

    #[test]
    fn go_to_same_path_does_not_push_history() {
        let lua = Lua::new();
        let mut state = State::new();

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "github")]);
        assert!(!state.go_to(vec!["github".to_string()], true));
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "search")]);

        assert!(state.go_to(vec!["github".to_string()], true));
        assert!(state.pop_history_path().is_some());
        assert!(state.pop_history_path().is_none());
    }

    #[test]
    fn history_forward_returns_to_page_after_history_back() {
        let lua = Lua::new();
        let mut state = State::new();

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "github")]);
        assert!(!state.go_to(vec!["github".to_string()], true));
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "search")]);
        assert!(!state.go_to(vec!["github".to_string(), "search".to_string()], true));

        let back_path = state.pop_history_path().expect("expected back history");
        assert_eq!(back_path, vec!["github".to_string()]);
        assert!(state.go_to(back_path, false));

        let forward_path = state
            .pop_forward_history_path()
            .expect("expected forward history");
        assert_eq!(
            forward_path,
            vec!["github".to_string(), "search".to_string()]
        );
    }

    #[test]
    fn normal_navigation_clears_forward_history() {
        let lua = Lua::new();
        let mut state = State::new();

        state.set_current_page_entries(vec![make_entry_with_key(&lua, "github")]);
        assert!(!state.go_to(vec!["github".to_string()], true));
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "search")]);
        assert!(!state.go_to(vec!["github".to_string(), "search".to_string()], true));

        let back_path = state.pop_history_path().expect("expected back history");
        assert!(state.go_to(back_path, false));
        state.set_current_page_entries(vec![make_entry_with_key(&lua, "issues")]);
        assert!(!state.go_to(vec!["github".to_string(), "issues".to_string()], true));

        assert!(state.pop_forward_history_path().is_none());
    }

    #[test]
    fn cached_preview_is_restored_when_hover_returns_to_entry() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["file".to_string()];
        state.set_current_page_entries(vec![
            make_entry_with_key(&lua, "a"),
            make_entry_with_key(&lua, "b"),
        ]);

        let path_a = vec!["file".to_string(), "a".to_string()];
        let path_b = vec!["file".to_string(), "b".to_string()];

        state.set_preview_for_path(
            &path_a,
            Some(Box::new(crate::widgets::StatefulParagraph::from(
                "preview-a",
            ))),
        );
        assert!(state.current_preview.is_some());

        state.scroll_by(1);
        assert!(!state.restore_preview_for_hovered());
        assert!(state.current_preview.is_none());

        state.set_preview_for_path(
            &path_b,
            Some(Box::new(crate::widgets::StatefulParagraph::from(
                "preview-b",
            ))),
        );
        assert!(state.current_preview.is_some());

        state.scroll_by(-1);
        assert!(state.restore_preview_for_hovered());
        assert!(state.current_preview.is_some());
    }

    #[test]
    fn stale_async_preview_result_is_cached_for_non_hovered_entry() {
        let lua = Lua::new();
        let mut state = State::new();
        state.current_path = vec!["file".to_string()];
        state.set_current_page_entries(vec![
            make_entry_with_key(&lua, "a"),
            make_entry_with_key(&lua, "b"),
        ]);

        let path_a = vec!["file".to_string(), "a".to_string()];
        state.scroll_by(1);
        state.set_preview_for_path(
            &path_a,
            Some(Box::new(crate::widgets::StatefulParagraph::from(
                "late-preview-a",
            ))),
        );

        assert_eq!(state.hovered().map(|entry| entry.key.as_str()), Some("b"));
        assert!(state.current_preview.is_none());

        state.scroll_by(-1);
        assert!(state.restore_preview_for_hovered());
        assert!(state.current_preview.is_some());
    }
}

/// State for the confirm dialog
#[derive(Debug)]
pub struct ConfirmDialog {
    pub title: Option<String>,
    pub prompt: String,
    pub on_confirm: LuaFunction,
    pub on_cancel: Option<LuaFunction>,
    pub selected_button: ConfirmButton,
}

impl ConfirmDialog {
    pub fn new(
        title: Option<String>,
        prompt: String,
        on_confirm: LuaFunction,
        on_cancel: Option<LuaFunction>,
    ) -> Self {
        Self {
            title,
            prompt,
            on_confirm,
            on_cancel,
            selected_button: ConfirmButton::Yes, // Default to Yes
        }
    }
}

pub struct NotificationItem {
    pub id: u64,
    pub message: Text<'static>,
    pub expiry: Instant,
}

#[derive(Default)]
pub struct TabSnapshot {
    pub id: u64,
    pub title: Option<String>,
    pub current_path: Vec<String>,
    pub current_page: Option<Page>,
    pub current_preview: Option<Box<dyn Renderable>>,
    current_preview_path: Option<Vec<String>>,
    /// Cache for pages to preserve cursor position, entries and filter when navigating back
    page_cache: HashMap<Vec<String>, Page>,
    preview_cache: HashMap<Vec<String>, Box<dyn Renderable>>,
    /// Navigation history for jumping back to the previously visited page
    navigation_history: Vec<Vec<String>>,
    /// Navigation history for jumping forward after jumping back
    navigation_forward_history: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct State {
    pub current_mode: Mode,
    tabs: Vec<TabSnapshot>,
    active_tab: usize,
    next_tab_id: u64,
    pub current_path: Vec<String>,
    pub current_page: Option<Page>,
    pub keymap_config: Vec<Keymap>,
    pub last_key_event_buffer: Vec<KeyEvent>,
    pub current_preview: Option<Box<dyn Renderable>>,
    current_preview_path: Option<Vec<String>>,
    pub notifications: Vec<NotificationItem>,
    next_notification_id: u64,

    /// Cache for pages to preserve cursor position, entries and filter when navigating back
    page_cache: HashMap<Vec<String>, Page>,
    preview_cache: HashMap<Vec<String>, Box<dyn Renderable>>,
    /// Navigation history for jumping back to the previously visited page
    navigation_history: Vec<Vec<String>>,
    /// Navigation history for jumping forward after jumping back
    navigation_forward_history: Vec<Vec<String>>,
    /// Hooks to call before reload command
    pub pre_reload_hooks: Vec<LuaFunction>,
    /// Hooks to call before quit command
    pub pre_quit_hooks: Vec<LuaFunction>,
    /// Hooks to call after entering a page
    pub post_page_enter_hooks: Vec<LuaFunction>,
    /// Confirm dialog state (shown on top of all UI)
    pub confirm_dialog: Option<ConfirmDialog>,
    /// Select dialog state (shown on top of all UI)
    pub select_dialog: Option<SelectDialog>,
    /// Input dialog state (shown on top of all UI)
    pub input_dialog: Option<InputDialog>,
    /// Minimum lines to keep between cursor and edge (like vim's scrolloff)
    pub scrolloff: usize,
}

impl State {
    /// Create a new State with default values
    pub fn new() -> Self {
        let mut state = Self {
            scrolloff: 5, // Keep 5 lines between cursor and edge
            next_tab_id: 1,
            ..Default::default()
        };
        state.tabs.push(TabSnapshot {
            id: 0,
            ..Default::default()
        });
        state
    }
}

impl State {
    fn clear_key_buffer(&mut self) {
        self.last_key_event_buffer.clear();
    }

    fn save_current_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.stash_current_preview();
        let tab = &mut self.tabs[self.active_tab];
        tab.title = self
            .current_path
            .first()
            .cloned()
            .or_else(|| Some("/".to_string()));
        tab.current_path = self.current_path.clone();
        tab.current_page = self.current_page.take();
        tab.current_preview = self.current_preview.take();
        tab.current_preview_path = self.current_preview_path.take();
        tab.page_cache = std::mem::take(&mut self.page_cache);
        tab.preview_cache = std::mem::take(&mut self.preview_cache);
        tab.navigation_history = std::mem::take(&mut self.navigation_history);
        tab.navigation_forward_history = std::mem::take(&mut self.navigation_forward_history);
    }

    fn load_current_tab(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        self.current_path = tab.current_path.clone();
        self.current_page = tab.current_page.take();
        self.current_preview = tab.current_preview.take();
        self.current_preview_path = tab.current_preview_path.take();
        self.page_cache = std::mem::take(&mut tab.page_cache);
        self.preview_cache = std::mem::take(&mut tab.preview_cache);
        self.navigation_history = std::mem::take(&mut tab.navigation_history);
        self.navigation_forward_history = std::mem::take(&mut tab.navigation_forward_history);
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    pub fn tab_infos(&self) -> Vec<(u64, Option<String>, Vec<String>)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(idx, tab)| {
                if idx == self.active_tab {
                    (
                        tab.id,
                        self.current_path
                            .first()
                            .cloned()
                            .or_else(|| Some("/".to_string())),
                        self.current_path.clone(),
                    )
                } else {
                    (tab.id, tab.title.clone(), tab.current_path.clone())
                }
            })
            .collect()
    }

    pub fn new_tab(&mut self, path: Vec<String>) {
        self.clear_key_buffer();
        self.save_current_tab();
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        self.tabs.push(TabSnapshot {
            id,
            title: path.first().cloned().or_else(|| Some("/".to_string())),
            current_path: path,
            ..Default::default()
        });
        self.active_tab = self.tabs.len() - 1;
        self.load_current_tab();
    }

    pub fn switch_tab(&mut self, index: usize) -> bool {
        self.clear_key_buffer();
        if index >= self.tabs.len() || index == self.active_tab {
            return false;
        }
        self.save_current_tab();
        self.active_tab = index;
        self.load_current_tab();
        true
    }

    pub fn next_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        let next = (self.active_tab + 1) % self.tabs.len();
        self.switch_tab(next)
    }

    pub fn prev_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        let prev = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        self.switch_tab(prev)
    }

    pub fn close_current_tab(&mut self) -> bool {
        self.clear_key_buffer();
        if self.tabs.len() <= 1 {
            return false;
        }
        self.current_page = None;
        self.current_preview = None;
        self.current_preview_path = None;
        self.page_cache.clear();
        self.preview_cache.clear();
        self.navigation_history.clear();
        self.navigation_forward_history.clear();
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.load_current_tab();
        true
    }

    fn set_page_entries(page: &mut Page, entries: Vec<PageEntry>) {
        // Save current selected index before updating entries
        let old_selected = page.list_state.selected();

        page.list = entries;
        // Apply current filter to new entries
        page.apply_filter();

        // Restore selection if possible
        if let Some(old_idx) = old_selected {
            // Only restore if there was a previous selection
            if page.filtered_list.is_empty() {
                page.list_state.select(None);
            } else {
                // Keep the old selection if it's still valid
                if old_idx < page.filtered_list.len() {
                    page.list_state.select(Some(old_idx));
                } else {
                    // Old index is out of range, select the last item
                    page.list_state.select(Some(page.filtered_list.len() - 1));
                }
            }
        }
    }

    pub fn set_current_page_entries(&mut self, entries: Vec<PageEntry>) {
        self.set_entries_for_path(&self.current_path.clone(), Some(entries));
    }

    pub fn set_entries_for_path(&mut self, path: &[String], entries: Option<Vec<PageEntry>>) {
        self.clear_key_buffer();
        let is_current_path = self.current_path == path;

        if is_current_path {
            self.stash_current_preview();
            self.current_preview_path = None;
            match entries {
                Some(entries) => {
                    let page = self.current_page.get_or_insert_default();
                    Self::set_page_entries(page, entries);
                }
                None => {
                    self.current_page = None;
                }
            }
            return;
        }

        match entries {
            Some(entries) => {
                let page = self.page_cache.entry(path.to_vec()).or_default();
                Self::set_page_entries(page, entries);
            }
            None => {
                self.page_cache.remove(path);
            }
        }
    }

    pub fn entries_for_path(&self, path: &[String]) -> Option<&[PageEntry]> {
        if self.current_path == path {
            self.current_page.as_ref().map(|page| page.list.as_slice())
        } else {
            self.page_cache.get(path).map(|page| page.list.as_slice())
        }
    }

    pub fn set_hover_by_path(&mut self, path: &[String]) -> bool {
        self.clear_key_buffer();
        let Some((key, parent_path)) = path.split_last() else {
            return false;
        };
        if self.current_path != parent_path {
            return false;
        }

        let Some(page) = &mut self.current_page else {
            return false;
        };

        let Some(idx) = page
            .filtered_list
            .iter()
            .position(|entry| entry.key == *key)
        else {
            return false;
        };

        page.list_state.select(Some(idx));
        true
    }
    pub fn add_keymap(&mut self, keymap: Keymap) {
        self.keymap_config.retain(|v| {
            !(v.mode == keymap.mode
                && v.key_sequence == keymap.key_sequence
                && v.path == keymap.path)
        });
        self.keymap_config.push(keymap);
    }

    pub fn tap_key(&mut self, event: KeyEvent) -> anyhow::Result<Option<LuaFunction>> {
        self.last_key_event_buffer.push(event);
        let entry_cands = self.entry_keymap_candidates()?;
        if !entry_cands.is_empty() {
            return Ok(self.resolve_keymap_candidates(entry_cands));
        }

        let page_cands = self.page_keymap_candidates();
        if !page_cands.is_empty() {
            return Ok(self.resolve_keymap_candidates(page_cands));
        }

        let global_cands = self.global_keymap_candidates();
        Ok(self.resolve_keymap_candidates(global_cands))
    }

    pub fn hovered(&self) -> Option<&PageEntry> {
        self.current_page
            .as_ref()
            .and_then(|p| p.list_state.selected().and_then(|s| p.filtered_list.get(s)))
    }

    pub fn hovered_path(&self) -> Option<Vec<String>> {
        self.hovered().map(|hovered| {
            self.current_path
                .iter()
                .cloned()
                .chain([hovered.key.clone()])
                .collect()
        })
    }

    fn entry_keymap_candidates(&self) -> anyhow::Result<Vec<ResolvedKeymap>> {
        if self.current_mode != Mode::Main {
            return Ok(Vec::new());
        }
        let Some(hovered) = self.hovered() else {
            return Ok(Vec::new());
        };
        let keymap_table = hovered
            .keymap_table()
            .map_err(anyhow::Error::from)
            .with_context(|| format!("Failed to read keymap for entry '{}'", hovered.key))?;
        let Some(keymap_table): Option<LuaTable> = keymap_table else {
            return Ok(Vec::new());
        };

        let mut cands = Vec::new();
        for pair in keymap_table.pairs::<String, LuaValue>() {
            let (key, value) = pair.map_err(anyhow::Error::from).with_context(|| {
                format!("Invalid keymap entry on hovered entry '{}'", hovered.key)
            })?;
            let (callback, _) = resolve_entry_keymap_value(&hovered.key, &key, value)?;

            let key_sequence = KeySequence::from(key.as_str());
            if key_sequence.prefix_match(&self.last_key_event_buffer) {
                cands.push(ResolvedKeymap {
                    key_sequence,
                    callback,
                });
            }
        }

        Ok(cands)
    }

    pub fn available_keymaps(&self) -> anyhow::Result<Vec<AvailableKeymap>> {
        let mut keymaps = Vec::new();

        if self.current_mode == Mode::Main {
            if let Some(hovered) = self.hovered() {
                if let Some(keymap_table) = hovered
                    .keymap_table()
                    .map_err(anyhow::Error::from)
                    .with_context(|| format!("Failed to read keymap for entry '{}'", hovered.key))?
                {
                    for pair in keymap_table.pairs::<String, LuaValue>() {
                        let (key, value) =
                            pair.map_err(anyhow::Error::from).with_context(|| {
                                format!("Invalid keymap entry on hovered entry '{}'", hovered.key)
                            })?;
                        let (callback, desc) =
                            resolve_entry_keymap_value(&hovered.key, &key, value)?;

                        keymaps.push(AvailableKeymap {
                            key,
                            desc,
                            callback,
                            source: "entry",
                            path_priority: None,
                        });
                    }
                }
            }
        }

        keymaps.extend(
            self.keymap_config
                .iter()
                .filter(|keymap| {
                    keymap.mode == self.current_mode
                        && keymap_path_matches(keymap.path.as_ref(), &self.current_path)
                })
                .map(|keymap| AvailableKeymap {
                    key: keymap.raw_key.clone(),
                    desc: keymap.desc.clone(),
                    callback: keymap.callback.clone(),
                    source: if keymap.path.is_some() { "page" } else { "global" },
                    path_priority: keymap.path.as_ref().map(|path| path.priority()),
                }),
        );

        keymaps.sort_by(|a, b| {
            let source_order = |source: &str| match source {
                "entry" => 0,
                "page" => 1,
                _ => 2,
            };

            source_order(a.source)
                .cmp(&source_order(b.source))
                .then_with(|| a.path_priority.cmp(&b.path_priority))
                .then_with(|| a.key.cmp(&b.key))
                .then_with(|| a.desc.cmp(&b.desc))
        });

        Ok(keymaps)
    }

    fn global_keymap_candidates(&self) -> Vec<ResolvedKeymap> {
        self.keymap_config
            .iter()
            .filter(|keymap| {
                keymap.mode == self.current_mode
                    && keymap.path.is_none()
                    && keymap
                        .key_sequence
                        .prefix_match(&self.last_key_event_buffer)
            })
            .map(|keymap| ResolvedKeymap {
                key_sequence: keymap.key_sequence.clone(),
                callback: keymap.callback.clone(),
            })
            .collect()
    }

    fn page_keymap_candidates(&self) -> Vec<ResolvedKeymap> {
        let best_priority = self
            .keymap_config
            .iter()
            .filter_map(|keymap| {
                let path = keymap.path.as_ref()?;
                (keymap.mode == self.current_mode
                    && path.matches(&self.current_path)
                    && keymap
                        .key_sequence
                        .prefix_match(&self.last_key_event_buffer))
                .then(|| path.priority())
            })
            .min();

        let Some(best_priority) = best_priority else {
            return Vec::new();
        };

        let mut cands = Vec::new();
        for keymap in self.keymap_config.iter().rev().filter(|keymap| {
            keymap.mode == self.current_mode
                && keymap
                    .path
                    .as_ref()
                    .is_some_and(|path| path.matches(&self.current_path) && path.priority() == best_priority)
                && keymap
                    .key_sequence
                    .prefix_match(&self.last_key_event_buffer)
        }) {
            if cands
                .iter()
                .any(|cand: &ResolvedKeymap| cand.key_sequence == keymap.key_sequence)
            {
                continue;
            }
            cands.push(ResolvedKeymap {
                key_sequence: keymap.key_sequence.clone(),
                callback: keymap.callback.clone(),
            });
        }
        cands.reverse();
        cands
    }

    fn resolve_keymap_candidates(&mut self, cands: Vec<ResolvedKeymap>) -> Option<LuaFunction> {
        match cands.len() {
            0 => {
                self.clear_key_buffer();
                None
            }
            1 => {
                let cand = cands.first().unwrap();
                if cand.key_sequence.all_match(&self.last_key_event_buffer) {
                    let cb = cand.callback.clone();
                    self.clear_key_buffer();
                    Some(cb)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn go_to(&mut self, path: Vec<String>, record_history: bool) -> bool {
        self.clear_key_buffer();
        if path == self.current_path {
            return self.current_page.is_some();
        }

        if record_history && self.current_page.is_some() {
            self.navigation_history.push(self.current_path.clone());
            self.navigation_forward_history.clear();
        }

        // Cache current page before navigating away
        self.stash_current_preview();
        if let Some(page) = self.current_page.take() {
            self.page_cache.insert(self.current_path.clone(), page);
        }

        self.current_path = path.clone();
        self.current_preview_path = None;

        // Try to restore page from cache
        if let Some(page) = self.page_cache.remove(&path) {
            self.current_page = Some(page);
            true // Restored from cache
        } else {
            false // Not in cache, needs to be loaded
        }
    }

    pub fn pop_history_path(&mut self) -> Option<Vec<String>> {
        self.clear_key_buffer();

        while let Some(path) = self.navigation_history.pop() {
            if path != self.current_path {
                self.navigation_forward_history
                    .push(self.current_path.clone());
                return Some(path);
            }
        }

        None
    }

    pub fn pop_forward_history_path(&mut self) -> Option<Vec<String>> {
        self.clear_key_buffer();

        while let Some(path) = self.navigation_forward_history.pop() {
            if path != self.current_path {
                self.navigation_history.push(self.current_path.clone());
                return Some(path);
            }
        }

        None
    }

    /// Clear cache for current path (used by reload command)
    pub fn clear_current_cache(&mut self) {
        self.page_cache.remove(&self.current_path);
    }

    /// Clear cache for a specific path.
    pub fn clear_cache_for_path(&mut self, path: &[String]) {
        self.page_cache.remove(path);
    }

    fn stash_current_preview(&mut self) {
        if let (Some(path), Some(preview)) = (
            self.current_preview_path.take(),
            self.current_preview.take(),
        ) {
            self.preview_cache.insert(path, preview);
        }
    }

    pub fn restore_preview_for_hovered(&mut self) -> bool {
        let Some(hovered_path) = self.hovered_path() else {
            self.stash_current_preview();
            self.current_preview_path = None;
            return false;
        };

        if self.current_preview.is_some()
            && self.current_preview_path.as_ref() == Some(&hovered_path)
        {
            return true;
        }

        self.stash_current_preview();
        if let Some(preview) = self.preview_cache.remove(&hovered_path) {
            self.current_preview = Some(preview);
            self.current_preview_path = Some(hovered_path);
            true
        } else {
            self.current_preview = None;
            self.current_preview_path = None;
            false
        }
    }

    pub fn set_preview_for_path(&mut self, path: &[String], preview: Option<Box<dyn Renderable>>) {
        let is_current = self
            .current_preview_path
            .as_ref()
            .is_some_and(|p| p == path)
            || self.hovered_path().as_deref() == Some(path);

        if is_current {
            if self.current_preview_path.as_deref() != Some(path) {
                self.stash_current_preview();
            }
            self.preview_cache.remove(path);
            self.current_preview = preview;
            self.current_preview_path = self.current_preview.as_ref().map(|_| path.to_vec());
            return;
        }

        match preview {
            Some(preview) => {
                self.preview_cache.insert(path.to_vec(), preview);
            }
            None => {
                self.preview_cache.remove(path);
            }
        }
    }

    pub fn clear_preview_for_path(&mut self, path: &[String]) {
        if self
            .current_preview_path
            .as_ref()
            .is_some_and(|p| p == path)
        {
            self.current_preview.take();
            self.current_preview_path = None;
        }
        self.preview_cache.remove(path);
    }

    pub fn scroll_by(&mut self, amount: i16) {
        self.clear_key_buffer();
        if let Some(page) = &mut self.current_page {
            if page.filtered_list.is_empty() {
                return;
            }

            let len = page.filtered_list.len();
            let current = page.list_state.selected().unwrap_or(0);

            // Calculate new selected index
            let new = if amount > 0 {
                let target = current.saturating_add(amount as usize);
                // Only wrap if single-step scroll and at the last entry
                if amount == 1 && current == len - 1 {
                    0
                } else {
                    target.min(len - 1)
                }
            } else {
                let target = current.saturating_sub(amount.unsigned_abs() as usize);
                // Only wrap if single-step scroll and at the first entry
                if amount == -1 && current == 0 {
                    len - 1
                } else {
                    target
                }
            };

            // Set the new selection
            // Offset will be adjusted by ListWidget::render based on scrolloff
            page.list_state.select(Some(new));
        }
    }

    pub fn scroll_preview_by(&mut self, amount: i16) {
        if let Some(p) = &mut self.current_preview {
            p.scroll_by(amount);
        }
    }

    pub fn push_notification(&mut self, message: Text<'static>) -> u64 {
        let id = self.next_notification_id;
        self.next_notification_id = self.next_notification_id.saturating_add(1);
        self.notifications.push(NotificationItem {
            id,
            message,
            expiry: Instant::now() + Duration::from_secs(3),
        });
        id
    }

    pub fn expire_notification(&mut self, id: u64) -> bool {
        let before = self.notifications.len();
        self.notifications.retain(|item| item.id != id);
        before != self.notifications.len()
    }

    pub fn prune_expired_notifications(&mut self) -> bool {
        let now = Instant::now();
        let before = self.notifications.len();
        self.notifications.retain(|item| item.expiry > now);
        before != self.notifications.len()
    }

    /// Show the confirm dialog
    pub fn show_confirm_dialog(
        &mut self,
        title: Option<String>,
        prompt: String,
        on_confirm: LuaFunction,
        on_cancel: Option<LuaFunction>,
    ) {
        self.confirm_dialog = Some(ConfirmDialog::new(title, prompt, on_confirm, on_cancel));
    }

    /// Show the input dialog
    pub fn show_input_dialog(
        &mut self,
        prompt: String,
        placeholder: String,
        value: String,
        on_submit: LuaFunction,
        on_cancel: LuaFunction,
        on_change: LuaFunction,
    ) {
        self.clear_key_buffer();
        let previous_mode = self.current_mode;
        self.current_mode = Mode::Input;
        self.input_dialog = Some(InputDialog::new(
            previous_mode,
            prompt,
            placeholder,
            value,
            on_submit,
            on_cancel,
            on_change,
        ));
    }

    pub fn close_input_dialog(&mut self) -> Option<InputDialog> {
        let dialog = self.input_dialog.take();
        if let Some(dialog) = &dialog {
            self.current_mode = dialog.previous_mode;
        }
        self.clear_key_buffer();
        dialog
    }

    pub fn input_dialog_submit(&mut self) -> Option<(String, LuaFunction)> {
        self.close_input_dialog()
            .map(|dialog| (dialog.text, dialog.on_submit))
    }

    pub fn input_dialog_cancel(&mut self) -> Option<LuaFunction> {
        self.close_input_dialog().map(|dialog| dialog.on_cancel)
    }

    pub fn input_dialog_insert_char(&mut self, c: char) -> Option<(String, LuaFunction)> {
        let dialog = self.input_dialog.as_mut()?;
        dialog.insert_char(c);
        Some((dialog.text.clone(), dialog.on_change.clone()))
    }

    pub fn input_dialog_get_text(&self) -> Option<String> {
        self.input_dialog.as_ref().map(|dialog| dialog.text.clone())
    }

    pub fn input_dialog_replace_text(
        &mut self,
        text: String,
    ) -> Option<(String, LuaFunction, bool)> {
        let dialog = self.input_dialog.as_mut()?;
        let changed = dialog.text != text;
        dialog.text = text;
        dialog.cursor_position = dialog.text.len();
        Some((dialog.text.clone(), dialog.on_change.clone(), changed))
    }

    pub fn input_dialog_backspace(&mut self) -> Option<(String, LuaFunction)> {
        let dialog = self.input_dialog.as_mut()?;
        dialog.backspace();
        Some((dialog.text.clone(), dialog.on_change.clone()))
    }

    pub fn input_dialog_clear_before_cursor(&mut self) -> Option<(String, LuaFunction)> {
        let dialog = self.input_dialog.as_mut()?;
        if dialog.clear_before_cursor() {
            Some((dialog.text.clone(), dialog.on_change.clone()))
        } else {
            None
        }
    }

    pub fn input_dialog_cursor_left(&mut self) -> bool {
        let Some(dialog) = self.input_dialog.as_mut() else {
            return false;
        };
        dialog.cursor_left();
        true
    }

    pub fn input_dialog_cursor_right(&mut self) -> bool {
        let Some(dialog) = self.input_dialog.as_mut() else {
            return false;
        };
        dialog.cursor_right();
        true
    }

    pub fn input_dialog_cursor_to_start(&mut self) -> bool {
        let Some(dialog) = self.input_dialog.as_mut() else {
            return false;
        };
        dialog.cursor_to_start();
        true
    }

    pub fn input_dialog_cursor_to_end(&mut self) -> bool {
        let Some(dialog) = self.input_dialog.as_mut() else {
            return false;
        };
        dialog.cursor_to_end();
        true
    }

    /// Toggle selected button in confirm dialog
    pub fn toggle_confirm_button(&mut self) {
        if let Some(dialog) = &mut self.confirm_dialog {
            dialog.selected_button = dialog.selected_button.toggle();
        }
    }

    /// Get the current selected button
    pub fn get_selected_button(&self) -> Option<ConfirmButton> {
        self.confirm_dialog.as_ref().map(|d| d.selected_button)
    }

    /// Get the current scrolloff value
    pub fn scrolloff(&self) -> usize {
        self.scrolloff
    }
}

struct ResolvedKeymap {
    key_sequence: KeySequence,
    callback: LuaFunction,
}
