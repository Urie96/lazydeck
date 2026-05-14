use crate::{plugin, Keymap, Mode};
use mlua::prelude::*;

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let set = lua
        .create_function(
            |lua, (mode, key, cb, opt): (Mode, String, LuaValue, Option<LuaTable>)| {
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
                let (desc, once) = opt
                    .map(|opt| {
                        Ok::<_, LuaError>((
                            opt.get::<Option<String>>("desc")?,
                            opt.get::<Option<bool>>("once")?.unwrap_or(false),
                        ))
                    })
                    .transpose()?
                    .unwrap_or((None, false));

                plugin::mut_scope_state(lua, |state| {
                    state.add_keymap(Keymap {
                        mode,
                        raw_key: key.clone(),
                        key_sequence: key.as_str().into(),
                        callback,
                        desc,
                        once,
                    });
                    Ok(())
                })
            },
        )?
        .into_lua(lua)?;
    lua.create_table_from([("set", set)])
}
