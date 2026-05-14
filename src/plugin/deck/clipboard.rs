use mlua::prelude::*;
use std::process::Command;

#[cfg(target_os = "android")]
fn termux_clipboard_get() -> mlua::Result<String> {
    let output = Command::new("termux-clipboard-get")
        .output()
        .map_err(|e| {
            LuaError::RuntimeError(format!(
                "Failed to run termux-clipboard-get: {}. Install Termux:API and run `pkg install termux-api`.",
                e
            ))
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(LuaError::RuntimeError(format!(
            "termux-clipboard-get failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(target_os = "android")]
fn termux_clipboard_set(text: &str) -> mlua::Result<()> {
    let status = Command::new("termux-clipboard-set")
        .arg(text)
        .status()
        .map_err(|e| {
            LuaError::RuntimeError(format!(
                "Failed to run termux-clipboard-set: {}. Install Termux:API and run `pkg install termux-api`.",
                e
            ))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(LuaError::RuntimeError(format!(
            "termux-clipboard-set failed with status: {}",
            status
        )))
    }
}

#[cfg(not(target_os = "android"))]
fn platform_clipboard_get() -> mlua::Result<String> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to access clipboard: {}", e)))?;

    clipboard
        .get_text()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to get clipboard content: {}", e)))
}

#[cfg(not(target_os = "android"))]
fn platform_clipboard_set(text: &str) -> mlua::Result<()> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to access clipboard: {}", e)))?;

    clipboard
        .set_text(text)
        .map_err(|e| LuaError::RuntimeError(format!("Failed to set clipboard content: {}", e)))
}

/// Get clipboard content
fn get(_lua: &Lua, _: ()) -> mlua::Result<String> {
    #[cfg(target_os = "android")]
    {
        termux_clipboard_get()
    }

    #[cfg(not(target_os = "android"))]
    {
        platform_clipboard_get()
    }
}

/// Set clipboard content
fn set(_lua: &Lua, text: String) -> mlua::Result<()> {
    #[cfg(target_os = "android")]
    {
        termux_clipboard_set(&text)
    }

    #[cfg(not(target_os = "android"))]
    {
        platform_clipboard_set(&text)
    }
}

/// Create the deck.clipboard table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get_fn = lua.create_function(get)?.into_lua(lua)?;
    let set_fn = lua.create_function(set)?.into_lua(lua)?;

    lua.create_table_from([("get", get_fn), ("set", set_fn)])
}
