use mlua::prelude::*;

/// `deck.stdpath(kind)`：按名称返回 lazydeck 目录。
///
/// 每次调用都重新解析 XDG 环境变量（见 `crate::paths`），不做缓存。
pub(super) fn new_function(lua: &Lua) -> mlua::Result<LuaFunction> {
    lua.create_function(|_, kind: String| {
        crate::paths::stdpath(&kind)
            .map(|path| path.to_string_lossy().to_string())
            .ok_or_else(|| {
                LuaError::RuntimeError(format!(
                    "unknown stdpath kind '{kind}' (expected 'config', 'data', 'state' or 'cache')"
                ))
            })
    })
}
