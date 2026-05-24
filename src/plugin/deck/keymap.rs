use crate::{plugin, Keymap, KeymapPathPattern, Mode};
use mlua::prelude::*;

fn resolve_path(lua: &Lua, opt: Option<&LuaTable>) -> mlua::Result<Option<KeymapPathPattern>> {
    let Some(opt) = opt else {
        return Ok(None);
    };

    let path = match opt.get::<Option<LuaValue>>("path")? {
        None => None,
        Some(LuaValue::Integer(0)) | Some(LuaValue::Number(0.0)) => {
            Some(plugin::borrow_scope_state(lua, |state| {
                Ok(state.current_path.clone())
            })?)
        }
        Some(LuaValue::Table(tbl)) => Some(
            tbl.sequence_values::<String>()
                .collect::<mlua::Result<Vec<_>>>()?,
        ),
        Some(other) => {
            return Err(LuaError::RuntimeError(format!(
                "keymap path must be 0 or a string array, got {}",
                other.type_name()
            )))
        }
    };

    Ok(path.map(KeymapPathPattern::new))
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let set = lua
        .create_function(
            |lua, (mode, key, cb, opt): (Mode, String, LuaValue, Option<LuaTable>)| {
                let path = resolve_path(lua, opt.as_ref())?;

                // Convert the callback to a LuaFunction
                let callback = match cb {
                    LuaValue::String(s) => {
                        // If it's a string, wrap it as: function() deck.cmd(s) end
                        let cmd_str = s.to_str()?.to_string();
                        let deck = lua.globals().get::<LuaTable>("deck")?;
                        let cmd_fn = deck.get::<LuaFunction>("cmd")?;
                        lua.create_function(move |_lua, ()| cmd_fn.call::<()>(cmd_str.clone()))?
                    }
                    LuaValue::Function(f) => f,
                    other => {
                        return Err(LuaError::RuntimeError(format!(
                            "keymap callback must be a string or function, got {}",
                            other.type_name()
                        )))
                    }
                };
                let desc = match opt.as_ref() {
                    Some(opt) => opt.get::<Option<String>>("desc")?,
                    None => None,
                };

                plugin::mut_scope_state(lua, |state| {
                    state.add_keymap(Keymap {
                        mode,
                        raw_key: key.clone(),
                        key_sequence: key.as_str().into(),
                        callback,
                        desc,
                        path,
                    });
                    Ok(())
                })
            },
        )?
        .into_lua(lua)?;
    lua.create_table_from([("set", set)])
}
