use mlua::prelude::*;
use serde::Serialize;
use serde_json::Value;

/// Decode a JSON string to a Lua value
fn decode(lua: &Lua, json_str: String) -> mlua::Result<LuaValue> {
    let value: Value = serde_json::from_str(&json_str)
        .map_err(|e| LuaError::RuntimeError(format!("JSON parse error: {}", e)))?;
    json_to_lua(lua, value)
}

/// Convert serde_json::Value to LuaValue
fn json_to_lua(lua: &Lua, value: Value) -> mlua::Result<LuaValue> {
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
        Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.into_iter().enumerate() {
                let lua_v = json_to_lua(lua, v)?;
                table.set(i + 1, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj.into_iter() {
                let lua_k = LuaValue::String(lua.create_string(&k)?);
                let lua_v = json_to_lua(lua, v)?;
                table.set(lua_k, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Encode a Lua value to a JSON string
fn encode(lua: &Lua, (value, opts): (LuaValue, Option<LuaTable>)) -> mlua::Result<String> {
    let json_value = lua_to_json(lua, value)?;

    // Check for indent option
    let indent = opts.and_then(|opt| opt.get::<Option<u32>>("indent").ok().flatten());

    let json_string = if let Some(indent) = indent {
        // Use pretty printing with custom indent
        let indent_str = " ".repeat(indent as usize);
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        json_value
            .serialize(&mut ser)
            .map_err(|e| LuaError::RuntimeError(format!("JSON encode error: {}", e)))?;
        String::from_utf8(buf).map_err(|e| LuaError::RuntimeError(format!("UTF-8 error: {}", e)))?
    } else {
        serde_json::to_string(&json_value)
            .map_err(|e| LuaError::RuntimeError(format!("JSON encode error: {}", e)))?
    };

    Ok(json_string)
}

/// Convert LuaValue to serde_json::Value
fn lua_to_json(lua: &Lua, value: LuaValue) -> mlua::Result<Value> {
    match value {
        LuaValue::Nil => Ok(Value::Null),
        LuaValue::Boolean(b) => Ok(Value::Bool(b)),
        LuaValue::Integer(i) => Ok(Value::Number(serde_json::Number::from(i))),
        LuaValue::Number(n) => {
            // In Lua, all numbers are floats
            // Use as_f64 to get the value
            Ok(Value::Number(
                serde_json::Number::from_f64(n).unwrap_or(serde_json::Number::from(0)),
            ))
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
                let mut arr = Vec::new();
                for i in 1..=len {
                    let v: LuaValue = t.get(i)?;
                    arr.push(lua_to_json(lua, v)?);
                }
                Ok(Value::Array(arr))
            } else {
                // It's a mapping
                let mut obj = serde_json::Map::new();
                for pair in t.pairs::<LuaValue, LuaValue>() {
                    let (k, v) = pair?;
                    let key = match k {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        LuaValue::Integer(i) => i.to_string(),
                        LuaValue::Number(n) => n.to_string(),
                        LuaValue::Boolean(b) => b.to_string(),
                        _ => {
                            format!("{:?}", k)
                        }
                    };
                    let val = lua_to_json(lua, v)?;
                    obj.insert(key, val);
                }
                Ok(Value::Object(obj))
            }
        }
        LuaValue::Function(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua function to JSON".to_string(),
        )),
        LuaValue::Thread(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua thread to JSON".to_string(),
        )),
        LuaValue::UserData(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua userdata to JSON".to_string(),
        )),
        LuaValue::LightUserData(_) => Err(LuaError::RuntimeError(
            "Cannot encode Lua light userdata to JSON".to_string(),
        )),
        LuaValue::Error(_) | LuaValue::Other(_) => Err(LuaError::RuntimeError(
            "Cannot encode this Lua value to JSON".to_string(),
        )),
    }
}

/// Create the deck.json table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let decode_fn = lua.create_function(decode)?.into_lua(lua)?;
    let encode_fn = lua.create_function(encode)?.into_lua(lua)?;

    lua.create_table_from([("decode", decode_fn), ("encode", encode_fn)])
}
