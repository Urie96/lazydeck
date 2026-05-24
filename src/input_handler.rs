use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::{plugin, State};

/// Handle keyboard input for the input dialog
/// Returns true if the key was handled, false if it should be passed to keymap
pub fn handle_input_dialog_key(
    lua: &mlua::Lua,
    state: &mut State,
    event_sender: &tokio::sync::mpsc::UnboundedSender<crate::events::Event>,
    key: KeyEvent,
) -> Result<bool> {
    // Ignore release events
    if key.kind == KeyEventKind::Release {
        return Ok(false);
    }

    if state.input_dialog.is_none() {
        return Ok(false);
    }

    if let Some(cb) = state.tap_key(key)? {
        plugin::scope(lua, state, event_sender, || cb.call::<()>(()))?;
        return Ok(true);
    }

    if !state.last_key_event_buffer.is_empty() {
        return Ok(true);
    }

    match key.code {
        KeyCode::Backspace => {
            let Some((text, on_change)) = state.input_dialog_backspace() else {
                return Ok(false);
            };

            plugin::scope(lua, state, event_sender, || on_change.call::<()>(text))?;
            Ok(true)
        }
        KeyCode::Left => Ok(state.input_dialog_cursor_left()),
        KeyCode::Right => Ok(state.input_dialog_cursor_right()),
        KeyCode::Char(c) => {
            let Some((text, on_change)) = state.input_dialog_insert_char(c) else {
                return Ok(false);
            };

            plugin::scope(lua, state, event_sender, || on_change.call::<()>(text))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::handle_input_dialog_key;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use mlua::Lua;

    use crate::{Keymap, Mode, State};

    fn make_noop(lua: &Lua) -> mlua::Function {
        lua.create_function(|_, ()| Ok(())).unwrap().to_owned()
    }

    #[test]
    fn input_keymap_can_override_builtin_shortcut() {
        let lua = Lua::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = State::new();

        state.show_input_dialog(
            "Search".to_string(),
            "keyword".to_string(),
            "abc".to_string(),
            make_noop(&lua),
            make_noop(&lua),
            make_noop(&lua),
        );

        let callback = lua
            .create_function(|lua, ()| lua.globals().set("hit", true))
            .unwrap()
            .to_owned();

        state.add_keymap(Keymap {
            mode: Mode::Input,
            raw_key: "<C-a>".to_string(),
            key_sequence: "<C-a>".into(),
            callback,
            desc: None,
            path: None,
        });

        let handled = handle_input_dialog_key(
            &lua,
            &mut state,
            &tx,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        )
        .unwrap();

        assert!(handled);
        assert_eq!(lua.globals().get::<bool>("hit").unwrap(), true);
        assert_eq!(state.input_dialog.as_ref().unwrap().cursor_position, 3);
    }

    #[test]
    fn enter_is_not_builtin_without_input_keymap() {
        let lua = Lua::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = State::new();

        state.show_input_dialog(
            "Search".to_string(),
            "keyword".to_string(),
            "abc".to_string(),
            make_noop(&lua),
            make_noop(&lua),
            make_noop(&lua),
        );

        let handled = handle_input_dialog_key(
            &lua,
            &mut state,
            &tx,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert!(!handled);
        assert!(state.input_dialog.is_some());
        assert_eq!(state.input_dialog.as_ref().unwrap().text, "abc");
    }
}
