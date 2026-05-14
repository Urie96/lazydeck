use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

#[derive(Debug, Default, Serialize, Deserialize)]
struct SecretStore {
    entries: HashMap<String, String>,
}

fn get_secrets_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/lazydeck/secrets")
    } else {
        PathBuf::from("/tmp/lazydeck_secrets")
    }
}

fn ensure_non_empty(label: &str, value: &str) -> mlua::Result<()> {
    if value.is_empty() {
        return Err(LuaError::RuntimeError(format!(
            "secrets {} must not be empty",
            label
        )));
    }
    Ok(())
}

fn namespace_to_filename(namespace: &str) -> mlua::Result<String> {
    ensure_non_empty("namespace", namespace)?;

    let mut encoded = String::with_capacity(namespace.len() * 2 + 5);
    for byte in namespace.as_bytes() {
        encoded.push_str(&format!("{:02x}", byte));
    }
    encoded.push_str(".json");
    Ok(encoded)
}

fn get_secret_path(namespace: &str) -> mlua::Result<PathBuf> {
    Ok(get_secrets_dir().join(namespace_to_filename(namespace)?))
}

fn ensure_secure_dir(dir: &Path) -> mlua::Result<()> {
    std::fs::create_dir_all(dir).into_lua_err().map_err(|err| {
        LuaError::RuntimeError(format!("Failed to create secrets directory: {}", err))
    })?;

    #[cfg(unix)]
    {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .into_lua_err()
            .map_err(|err| {
                LuaError::RuntimeError(format!(
                    "Failed to set secrets directory permissions: {}",
                    err
                ))
            })?;
    }

    Ok(())
}

fn load_store(namespace: &str) -> mlua::Result<SecretStore> {
    let path = get_secret_path(namespace)?;
    if !path.exists() {
        return Ok(SecretStore::default());
    }

    let content = std::fs::read_to_string(&path)
        .into_lua_err()
        .map_err(|err| LuaError::RuntimeError(format!("Failed to read secrets file: {}", err)))?;

    serde_json::from_str(&content)
        .into_lua_err()
        .map_err(|err| LuaError::RuntimeError(format!("Failed to parse secrets file: {}", err)))
}

fn write_secure_file(path: &Path, content: &str) -> mlua::Result<()> {
    let mut tmp_path = path.to_path_buf();
    tmp_path.set_extension("tmp");

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    options.mode(0o600);

    let mut file = options.open(&tmp_path).into_lua_err().map_err(|err| {
        LuaError::RuntimeError(format!("Failed to open temp secrets file: {}", err))
    })?;

    file.write_all(content.as_bytes())
        .into_lua_err()
        .map_err(|err| LuaError::RuntimeError(format!("Failed to write secrets file: {}", err)))?;
    file.flush()
        .into_lua_err()
        .map_err(|err| LuaError::RuntimeError(format!("Failed to flush secrets file: {}", err)))?;

    #[cfg(unix)]
    {
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))
            .into_lua_err()
            .map_err(|err| {
                LuaError::RuntimeError(format!("Failed to set secrets file permissions: {}", err))
            })?;
    }

    std::fs::rename(&tmp_path, path)
        .into_lua_err()
        .map_err(|err| {
            LuaError::RuntimeError(format!("Failed to replace secrets file: {}", err))
        })?;

    Ok(())
}

fn save_store(namespace: &str, store: &SecretStore) -> mlua::Result<()> {
    let path = get_secret_path(namespace)?;
    let dir = path
        .parent()
        .ok_or_else(|| LuaError::RuntimeError("Failed to resolve secrets directory".to_string()))?;
    ensure_secure_dir(dir)?;

    if store.entries.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).into_lua_err().map_err(|err| {
                LuaError::RuntimeError(format!("Failed to delete secrets file: {}", err))
            })?;
        }
        return Ok(());
    }

    let content = serde_json::to_string(store)
        .into_lua_err()
        .map_err(|err| LuaError::RuntimeError(format!("Failed to serialize secrets: {}", err)))?;

    write_secure_file(&path, &content)
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get = lua
        .create_function(
            |_, (namespace, key): (String, String)| -> mlua::Result<Option<String>> {
                ensure_non_empty("key", &key)?;
                let store = load_store(&namespace)?;
                Ok(store.entries.get(&key).cloned())
            },
        )?
        .into_lua(lua)?;

    let set = lua
        .create_function(
            |_, (namespace, key, value): (String, String, String)| -> mlua::Result<()> {
                ensure_non_empty("key", &key)?;
                let mut store = load_store(&namespace)?;
                store.entries.insert(key, value);
                save_store(&namespace, &store)
            },
        )?
        .into_lua(lua)?;

    let delete = lua
        .create_function(
            |_, (namespace, key): (String, String)| -> mlua::Result<()> {
                ensure_non_empty("key", &key)?;
                let mut store = load_store(&namespace)?;
                store.entries.remove(&key);
                save_store(&namespace, &store)
            },
        )?
        .into_lua(lua)?;

    lua.create_table_from([("get", get), ("set", set), ("delete", delete)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_filename_is_hex_encoded() -> mlua::Result<()> {
        let filename = namespace_to_filename("github.token")?;
        assert_eq!(filename, "6769746875622e746f6b656e.json");
        Ok(())
    }

    #[test]
    fn empty_namespace_is_rejected() {
        let err = namespace_to_filename("").unwrap_err();
        assert!(err.to_string().contains("namespace"));
    }
}
