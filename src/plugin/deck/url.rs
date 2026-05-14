use mlua::prelude::*;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let encode = lua
        .create_function(|_lua, value: String| {
            Ok(utf8_percent_encode(&value, NON_ALPHANUMERIC).to_string())
        })?
        .into_lua(lua)?;

    let decode = lua
        .create_function(|_lua, value: String| {
            percent_decode_str(&value)
                .decode_utf8()
                .map(|s| s.into_owned())
                .map_err(|err| LuaError::RuntimeError(format!("Failed to decode URL: {}", err)))
        })?
        .into_lua(lua)?;

    lua.create_table_from([("encode", encode), ("decode", decode)])
}
