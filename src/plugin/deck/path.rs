use mlua::prelude::*;
use std::{ffi::OsString, path::PathBuf};

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let split = lua
        .create_function(|lua, path: PathBuf| path.iter().collect::<Vec<_>>().into_lua(lua))?
        .into_lua(lua)?;

    let join = lua
        .create_function(|_, path_list: Vec<OsString>| {
            // Path::new(s)
            Ok(path_list.iter().collect::<PathBuf>())
        })?
        .into_lua(lua)?;

    lua.create_table_from([("split", split), ("join", join)])
}
