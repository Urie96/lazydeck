use mlua::prelude::*;

/// Return the lowercase hexadecimal MD5 digest of a Lua string's raw bytes.
fn md5(_lua: &Lua, data: LuaString) -> mlua::Result<String> {
    Ok(format!("{:x}", md5::compute(data.as_bytes())))
}

/// Create the deck.hash table.
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let md5_fn = lua.create_function(md5)?.into_lua(lua)?;

    lua.create_table_from([("md5", md5_fn)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_hashes_lua_string_bytes() {
        let lua = Lua::new();
        let hash = new_table(&lua).expect("hash table");
        let md5: LuaFunction = hash.get("md5").expect("md5 function");

        let digest: String = md5.call("hello").expect("md5 digest");
        assert_eq!(digest, "5d41402abc4b2a76b9719d911017c592");
    }
}
