use ::base64::engine::general_purpose;
use ::base64::Engine;
use mlua::prelude::*;

/// Decode a base64 string to a Lua string (returns raw bytes as string)
fn decode(lua: &Lua, encoded: String) -> mlua::Result<LuaString> {
    let decoded = general_purpose::STANDARD
        .decode(&encoded)
        .map_err(|e| LuaError::RuntimeError(format!("Base64 decode error: {}", e)))?;

    // Convert bytes to Lua string
    lua.create_string(&decoded)
}

/// Encode a Lua string to base64
fn encode(_lua: &Lua, data: LuaString) -> mlua::Result<String> {
    let encoded = general_purpose::STANDARD.encode(data.as_bytes());
    Ok(encoded)
}

/// Create the deck.base64 table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let decode_fn = lua.create_function(decode)?.into_lua(lua)?;
    let encode_fn = lua.create_function(encode)?.into_lua(lua)?;

    lua.create_table_from([("decode", decode_fn), ("encode", encode_fn)])
}
