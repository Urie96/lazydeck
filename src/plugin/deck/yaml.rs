use mlua::prelude::*;
use serde_yaml::Value;

/// Decode a YAML string to a Lua value
fn decode(lua: &Lua, yaml_str: String) -> mlua::Result<LuaValue> {
    let value: Value = serde_yaml::from_str(&yaml_str)
        .map_err(|e| LuaError::RuntimeError(format!("YAML parse error: {}", e)))?;
    yaml_to_lua(lua, value)
}

/// Convert serde_yaml::Value to LuaValue
fn yaml_to_lua(lua: &Lua, value: Value) -> mlua::Result<LuaValue> {
    match value {
        Value::Null => Ok(LuaValue::Nil),
        Value::Bool(b) => Ok(LuaValue::Boolean(b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Err(LuaError::RuntimeError("Invalid number".to_string()))
            }
        }
        Value::String(s) => Ok(LuaValue::String(lua.create_string(&s)?)),
        Value::Sequence(seq) => {
            let table = lua.create_table()?;
            for (i, v) in seq.into_iter().enumerate() {
                let lua_v = yaml_to_lua(lua, v)?;
                table.set(i + 1, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Mapping(map) => {
            let table = lua.create_table()?;
            for (k, v) in map.into_iter() {
                let key = match k {
                    Value::String(s) => LuaValue::String(lua.create_string(&s)?),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            LuaValue::Integer(i)
                        } else if let Some(f) = n.as_f64() {
                            LuaValue::Number(f)
                        } else {
                            LuaValue::String(lua.create_string(&n.to_string())?)
                        }
                    }
                    Value::Bool(b) => LuaValue::Boolean(b),
                    Value::Null => LuaValue::Nil,
                    _ => LuaValue::String(lua.create_string(&k.as_str().unwrap_or_default())?),
                };
                let lua_v = yaml_to_lua(lua, v)?;
                table.set(key, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Tagged(tagged) => {
            // Just extract the inner value, ignoring the tag
            yaml_to_lua(lua, tagged.value)
        }
    }
}

/// Encode a Lua value to a YAML string
fn encode(lua: &Lua, value: LuaValue) -> mlua::Result<String> {
    let yaml_value = lua_to_yaml(lua, value)?;
    serde_yaml::to_string(&yaml_value)
        .map_err(|e| LuaError::RuntimeError(format!("YAML encode error: {}", e)))
}

/// Convert LuaValue to serde_yaml::Value
fn lua_to_yaml(lua: &Lua, value: LuaValue) -> mlua::Result<Value> {
    match value {
        LuaValue::Nil => Ok(Value::Null),
        LuaValue::Boolean(b) => Ok(Value::Bool(b)),
        LuaValue::Integer(i) => Ok(Value::Number(serde_yaml::Number::from(i))),
        LuaValue::Number(n) => {
            // In Lua, all numbers are floats
            Ok(Value::Number(serde_yaml::Number::from(n as f64)))
        }
        LuaValue::String(s) => Ok(Value::String(s.to_str()?.to_string())),
        LuaValue::Table(t) => {
            // Check if it's an array (sequential) or mapping
            let mut is_array = true;
            let len = t.len().unwrap_or(0);

            // Check if all keys are sequential integers starting from 1
            if len > 0 {
                for i in 1..=len {
                    if t.get::<LuaValue>(i).is_err() {
                        is_array = false;
                        break;
                    }
                }
            } else {
                is_array = false;
            }

            if is_array && len > 0 {
                // It's an array
                let mut seq = Vec::new();
                for i in 1..=len {
                    let v: LuaValue = t.get(i)?;
                    seq.push(lua_to_yaml(lua, v)?);
                }
                Ok(Value::Sequence(seq))
            } else {
                // It's a mapping
                let mut map = serde_yaml::Mapping::new();
                for pair in t.pairs::<LuaValue, LuaValue>() {
                    let (k, v) = pair?;
                    let key = match k {
                        LuaValue::String(s) => Value::String(s.to_str()?.to_string()),
                        LuaValue::Integer(i) => Value::String(i.to_string()),
                        LuaValue::Number(n) => Value::String(n.to_string()),
                        LuaValue::Boolean(b) => Value::String(b.to_string()),
                        _ => {
                            let key_str = lua_to_yaml(
                                lua,
                                LuaValue::String(lua.create_string(&format!("{:?}", k))?),
                            )?;
                            key_str
                        }
                    };
                    let val = lua_to_yaml(lua, v)?;
                    map.insert(key, val);
                }
                Ok(Value::Mapping(map))
            }
        }
        LuaValue::Function(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua function to YAML".to_string(),
        )),
        LuaValue::Thread(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua thread to YAML".to_string(),
        )),
        LuaValue::UserData(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua userdata to YAML".to_string(),
        )),
        LuaValue::LightUserData(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua light userdata to YAML".to_string(),
        )),
        LuaValue::Error(_) | LuaValue::Other(_) => Err(LuaError::RuntimeError(
            "Cannot encode this Lua value to YAML".to_string(),
        )),
    }
}

/// Create the deck.yaml table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let decode_fn = lua.create_function(decode)?.into_lua(lua)?;
    let encode_fn = lua.create_function(encode)?.into_lua(lua)?;

    lua.create_table_from([("decode", decode_fn), ("encode", encode_fn)])
}
