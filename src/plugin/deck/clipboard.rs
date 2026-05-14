use arboard::Clipboard;
use mlua::prelude::*;

/// Get clipboard content
fn get(_lua: &Lua, _: ()) -> mlua::Result<String> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to access clipboard: {}", e)))?;

    clipboard
        .get_text()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to get clipboard content: {}", e)))
}

/// Set clipboard content
fn set(_lua: &Lua, text: String) -> mlua::Result<()> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to access clipboard: {}", e)))?;

    clipboard
        .set_text(&text)
        .map_err(|e| LuaError::RuntimeError(format!("Failed to set clipboard content: {}", e)))
}

/// Create the deck.clipboard table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get_fn = lua.create_function(get)?.into_lua(lua)?;
    let set_fn = lua.create_function(set)?.into_lua(lua)?;

    lua.create_table_from([("get", get_fn), ("set", set_fn)])
}
