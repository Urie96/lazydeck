use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Persist dirty cache namespaces in the background at a low frequency.
const FLUSH_INTERVAL: Duration = Duration::from_secs(10);

/// Get the cache directory path.
fn get_cache_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache/lazydeck/cache")
    } else {
        std::env::temp_dir().join("lazydeck_cache")
    }
}

fn namespace_to_filename(namespace: &str) -> mlua::Result<String> {
    if namespace.is_empty() {
        return Err(LuaError::RuntimeError(
            "cache namespace must not be empty".to_string(),
        ));
    }

    let mut encoded = String::with_capacity(namespace.len() * 2 + 5);
    for byte in namespace.as_bytes() {
        encoded.push_str(&format!("{:02x}", byte));
    }
    encoded.push_str(".json");
    Ok(encoded)
}

/// Get the cache file path for a namespace.
fn get_cache_path(namespace: &str) -> mlua::Result<PathBuf> {
    Ok(get_cache_dir().join(namespace_to_filename(namespace)?))
}

/// Cache entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    value: serde_json::Value,
    expires: Option<u64>, // Unix timestamp
    #[serde(default)]
    ttl: Option<u64>,
    #[serde(default)]
    refresh_on_get: bool,
}

#[derive(Debug, Default)]
struct NamespaceCache {
    entries: HashMap<String, CacheEntry>,
    dirty: bool,
}

static CACHE_STORE: OnceLock<Mutex<HashMap<String, NamespaceCache>>> = OnceLock::new();
static FLUSH_TASK_STARTED: OnceLock<()> = OnceLock::new();

fn cache_store() -> &'static Mutex<HashMap<String, NamespaceCache>> {
    CACHE_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_store() -> mlua::Result<std::sync::MutexGuard<'static, HashMap<String, NamespaceCache>>> {
    cache_store()
        .lock()
        .map_err(|_| LuaError::RuntimeError("cache store mutex poisoned".to_string()))
}

/// Load cache for a namespace from disk.
fn load_cache_from_disk(namespace: &str) -> mlua::Result<HashMap<String, CacheEntry>> {
    let cache_path = get_cache_path(namespace)?;

    if !cache_path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&cache_path)
        .into_lua_err()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to read cache file: {}", e)))?;

    serde_json::from_str(&content)
        .into_lua_err()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to parse cache file: {}", e)))
}

/// Save cache for a namespace to disk.
fn save_cache_to_disk(namespace: &str, cache: &HashMap<String, CacheEntry>) -> mlua::Result<()> {
    let cache_path = get_cache_path(namespace)?;

    // Ensure directory exists
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)
            .into_lua_err()
            .map_err(|e| {
                LuaError::RuntimeError(format!("Failed to create cache directory: {}", e))
            })?;
    }

    // Remove expired entries before saving
    let now = chrono::Utc::now().timestamp() as u64;
    let cleaned: HashMap<String, CacheEntry> = cache
        .iter()
        .filter(|(_, entry)| entry.expires.map(|exp| exp > now).unwrap_or(true))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if cleaned.is_empty() {
        if cache_path.exists() {
            std::fs::remove_file(&cache_path)
                .into_lua_err()
                .map_err(|e| {
                    LuaError::RuntimeError(format!("Failed to delete cache file: {}", e))
                })?;
        }
        return Ok(());
    }

    let json = serde_json::to_string(&cleaned)
        .into_lua_err()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to serialize cache: {}", e)))?;

    std::fs::write(&cache_path, json)
        .into_lua_err()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to write cache file: {}", e)))?;

    Ok(())
}

fn ensure_namespace_loaded<'a>(
    store: &'a mut HashMap<String, NamespaceCache>,
    namespace: &str,
) -> mlua::Result<&'a mut NamespaceCache> {
    if !store.contains_key(namespace) {
        let entries = load_cache_from_disk(namespace)?;
        store.insert(
            namespace.to_string(),
            NamespaceCache {
                entries,
                dirty: false,
            },
        );
    }

    Ok(store.get_mut(namespace).expect("namespace cache inserted"))
}

fn now_ts() -> u64 {
    chrono::Utc::now().timestamp() as u64
}

fn refresh_cache_entry(entry: &mut CacheEntry, now: u64) -> bool {
    if !entry.refresh_on_get {
        return false;
    }

    let Some(ttl) = entry.ttl else {
        return false;
    };

    entry.expires = Some(now.saturating_add(ttl));
    true
}

fn flush_dirty_locked(store: &mut HashMap<String, NamespaceCache>) -> mlua::Result<()> {
    for (namespace, cache) in store.iter_mut() {
        if !cache.dirty {
            continue;
        }

        cache
            .entries
            .retain(|_, entry| entry.expires.map(|exp| exp > now_ts()).unwrap_or(true));
        save_cache_to_disk(namespace, &cache.entries)?;
        cache.dirty = false;
    }

    Ok(())
}

pub(crate) fn flush_dirty_namespaces() -> mlua::Result<()> {
    let mut store = lock_store()?;
    flush_dirty_locked(&mut store)
}

pub(crate) fn start_background_flush_task() {
    FLUSH_TASK_STARTED.get_or_init(|| {
        tokio::task::spawn_local(async {
            let mut interval = tokio::time::interval(FLUSH_INTERVAL);
            loop {
                interval.tick().await;
                let _ = flush_dirty_namespaces();
            }
        });
    });
}

/// Convert Lua value to JSON value
fn lua_to_json(value: LuaValue) -> mlua::Result<serde_json::Value> {
    match value {
        LuaValue::Nil => Ok(serde_json::Value::Null),
        LuaValue::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        LuaValue::Integer(n) => Ok(serde_json::Value::Number(n.into())),
        LuaValue::Number(n) => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(n).unwrap_or(serde_json::Number::from(0)),
        )),
        LuaValue::String(s) => Ok(serde_json::Value::String(s.to_string_lossy().to_string())),
        LuaValue::Table(t) => {
            let len = t.len()?;
            let is_array = (1..=len).all(|i| t.contains_key(i).unwrap_or(false));

            if is_array && len > 0 {
                let mut arr = Vec::new();
                for i in 1..=len {
                    let val = t.get(i)?;
                    arr.push(lua_to_json(val)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut obj = serde_json::Map::new();
                for pair in t.pairs::<String, LuaValue>() {
                    let (k, v) = pair?;
                    obj.insert(k, lua_to_json(v)?);
                }
                Ok(serde_json::Value::Object(obj))
            }
        }
        _ => Err(LuaError::RuntimeError(format!(
            "Unsupported type for cache: {:?}",
            value.type_name()
        ))),
    }
}

/// Convert JSON value to Lua value
fn json_to_lua(lua: &Lua, value: serde_json::Value) -> mlua::Result<LuaValue> {
    match value {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Number(0.0))
            }
        }
        serde_json::Value::String(s) => Ok(lua.create_string(&s)?.into_lua(lua)?),
        serde_json::Value::Array(arr) => {
            let tbl = lua.create_table_with_capacity(arr.len(), 0)?;
            for (i, v) in arr.into_iter().enumerate() {
                tbl.raw_set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(tbl))
        }
        serde_json::Value::Object(obj) => {
            let tbl = lua.create_table_with_capacity(0, obj.len())?;
            for (k, v) in obj {
                tbl.raw_set(k, json_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(tbl))
        }
    }
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get = lua
        .create_function(
            |lua, (namespace, key): (String, String)| -> mlua::Result<LuaValue> {
                let mut store = lock_store()?;
                let cache = ensure_namespace_loaded(&mut store, &namespace)?;
                let now = now_ts();

                let expired = match cache.entries.get(&key) {
                    Some(entry) => entry.expires.map(|expires| expires <= now).unwrap_or(false),
                    None => return Ok(LuaValue::Nil),
                };

                if expired {
                    cache.entries.remove(&key);
                    cache.dirty = true;
                    return Ok(LuaValue::Nil);
                }

                if let Some(entry) = cache.entries.get_mut(&key) {
                    let value = entry.value.clone();
                    if refresh_cache_entry(entry, now) {
                        cache.dirty = true;
                    }
                    return json_to_lua(lua, value);
                }

                Ok(LuaValue::Nil)
            },
        )?
        .into_lua(lua)?;

    let set = lua
        .create_function(
            |_lua, (namespace, key, value, opts): (String, String, LuaValue, Option<LuaTable>)| {
                let mut store = lock_store()?;
                let cache = ensure_namespace_loaded(&mut store, &namespace)?;
                let json_value = lua_to_json(value)?;

                let (ttl, refresh_on_get) = if let Some(opts) = opts {
                    let ttl: Option<u64> = opts.get("ttl").ok();
                    let refresh_on_get: bool = opts.get("refresh_on_get").unwrap_or(false);
                    (ttl, refresh_on_get)
                } else {
                    (None, false)
                };

                let expires = ttl.map(|ttl| now_ts().saturating_add(ttl));

                cache.entries.insert(
                    key,
                    CacheEntry {
                        value: json_value,
                        expires,
                        ttl,
                        refresh_on_get: ttl.is_some() && refresh_on_get,
                    },
                );
                cache.dirty = true;
                Ok(())
            },
        )?
        .into_lua(lua)?;

    let delete = lua
        .create_function(
            |_, (namespace, key): (String, String)| -> mlua::Result<()> {
                let mut store = lock_store()?;
                let cache = ensure_namespace_loaded(&mut store, &namespace)?;
                if cache.entries.remove(&key).is_some() {
                    cache.dirty = true;
                }
                Ok(())
            },
        )?
        .into_lua(lua)?;

    let clear = lua
        .create_function(|_lua, namespace: String| -> mlua::Result<()> {
            let mut store = lock_store()?;
            let cache = ensure_namespace_loaded(&mut store, &namespace)?;
            if !cache.entries.is_empty() {
                cache.entries.clear();
                cache.dirty = true;
            } else if get_cache_path(&namespace)?.exists() {
                cache.dirty = true;
            }
            Ok(())
        })?
        .into_lua(lua)?;

    lua.create_table_from([
        ("get", get),
        ("set", set),
        ("delete", delete),
        ("clear", clear),
    ])
}

#[cfg(test)]
mod tests {
    use super::{namespace_to_filename, refresh_cache_entry, CacheEntry};

    #[test]
    fn namespace_filename_is_hex_encoded() {
        assert_eq!(
            namespace_to_filename("plugin/demo").unwrap(),
            "706c7567696e2f64656d6f.json"
        );
    }

    #[test]
    fn empty_namespace_is_rejected() {
        assert!(namespace_to_filename("").is_err());
    }

    #[test]
    fn refresh_cache_entry_updates_expiration_when_enabled() {
        let mut entry = CacheEntry {
            value: serde_json::Value::String("demo".to_string()),
            expires: Some(10),
            ttl: Some(30),
            refresh_on_get: true,
        };

        assert!(refresh_cache_entry(&mut entry, 100));
        assert_eq!(entry.expires, Some(130));
    }

    #[test]
    fn refresh_cache_entry_skips_disabled_entries() {
        let mut entry = CacheEntry {
            value: serde_json::Value::String("demo".to_string()),
            expires: Some(10),
            ttl: Some(30),
            refresh_on_get: false,
        };

        assert!(!refresh_cache_entry(&mut entry, 100));
        assert_eq!(entry.expires, Some(10));
    }
}
