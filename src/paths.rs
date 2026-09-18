//! lazydeck 目录解析。
//!
//! 所有属于 lazydeck 的目录都通过 XDG Base Directory 环境变量解析，因此用户可以按
//! XDG 规范分别迁移配置、数据、状态和缓存目录。按照规范，只有值为**绝对路径**的
//! 环境变量才会被采用（空值同样被忽略），否则回退到 `$HOME` 下的标准默认位置；
//! `$HOME` 也不存在时回退到系统临时目录。
//!
//! 解析结果通过 `_deck.stdpath` 暴露给 Lua（见 `preset/lua/stdpath.lua`）。
//!
//! 这里刻意不做缓存：每次调用都重新读取环境变量，因此 Lua 侧每次调用
//! `deck.stdpath(...)` 拿到的都是当前环境下的路径。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const APP_NAME: &str = "lazydeck";

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 解析 XDG 基础目录：`env_value` 为绝对路径时直接采用，否则用 `$HOME/<fallback>`。
fn resolve_base_dir(
    env_value: Option<&OsStr>,
    home: Option<&Path>,
    fallback: &str,
) -> Option<PathBuf> {
    if let Some(value) = env_value.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Some(path);
        }
    }

    home.map(|home| home.join(fallback))
}

fn app_dir(env_var: &str, fallback: &str, temp_fallback: &str) -> PathBuf {
    let env_value = std::env::var_os(env_var);
    resolve_base_dir(env_value.as_deref(), home_dir().as_deref(), fallback)
        .unwrap_or_else(|| std::env::temp_dir().join(temp_fallback))
        .join(APP_NAME)
}

/// `$XDG_CONFIG_HOME/lazydeck`，默认 `~/.config/lazydeck`。
pub(crate) fn config_dir() -> PathBuf {
    app_dir("XDG_CONFIG_HOME", ".config", "lazydeck-config")
}

/// `$XDG_DATA_HOME/lazydeck`，默认 `~/.local/share/lazydeck`。
pub(crate) fn data_dir() -> PathBuf {
    app_dir("XDG_DATA_HOME", ".local/share", "lazydeck-data")
}

/// `$XDG_STATE_HOME/lazydeck`，默认 `~/.local/state/lazydeck`。
pub(crate) fn state_dir() -> PathBuf {
    app_dir("XDG_STATE_HOME", ".local/state", "lazydeck-state")
}

/// `$XDG_CACHE_HOME/lazydeck`，默认 `~/.cache/lazydeck`。
pub(crate) fn cache_dir() -> PathBuf {
    app_dir("XDG_CACHE_HOME", ".cache", "lazydeck-cache")
}

/// 按名称解析 lazydeck 目录，供 Lua 的 `deck.stdpath(kind)` 使用。
pub(crate) fn stdpath(kind: &str) -> Option<PathBuf> {
    Some(match kind {
        "config" => config_dir(),
        "data" => data_dir(),
        "state" => state_dir(),
        "cache" => cache_dir(),
        _ => return None,
    })
}

/// 默认配置文件：`<config_dir>/init.lua`。
pub(crate) fn config_file() -> PathBuf {
    config_dir().join("init.lua")
}

/// Rust 日志文件：`<state_dir>/lazydeck.log`。
pub(crate) fn rust_log_file() -> PathBuf {
    state_dir().join("lazydeck.log")
}

/// Lua 日志文件：`<state_dir>/lua.log`。
pub(crate) fn lua_log_file() -> PathBuf {
    state_dir().join("lua.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(env: Option<&str>, home: Option<&str>, fallback: &str) -> Option<PathBuf> {
        resolve_base_dir(env.map(OsStr::new), home.map(Path::new), fallback)
    }

    #[test]
    fn absolute_env_var_wins() {
        assert_eq!(
            base(Some("/custom/config"), Some("/home/user"), ".config"),
            Some(PathBuf::from("/custom/config"))
        );
    }

    #[test]
    fn empty_and_relative_env_vars_are_ignored() {
        assert_eq!(
            base(Some(""), Some("/home/user"), ".config"),
            Some(PathBuf::from("/home/user/.config"))
        );
        assert_eq!(
            base(Some("relative/config"), Some("/home/user"), ".config"),
            Some(PathBuf::from("/home/user/.config"))
        );
    }

    #[test]
    fn missing_home_falls_back_to_none() {
        assert_eq!(base(None, None, ".config"), None);
    }

    #[test]
    fn home_relative_fallbacks_are_appended() {
        assert_eq!(
            base(None, Some("/home/user"), ".local/share"),
            Some(PathBuf::from("/home/user/.local/share"))
        );
    }
}
