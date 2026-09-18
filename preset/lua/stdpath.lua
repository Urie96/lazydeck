---@alias deck.stdpath.kind
---| '"config"' # 配置目录（$XDG_CONFIG_HOME/lazydeck，默认 ~/.config/lazydeck）
---| '"data"' # 数据目录（$XDG_DATA_HOME/lazydeck，默认 ~/.local/share/lazydeck）
---| '"state"' # 状态目录（$XDG_STATE_HOME/lazydeck，默认 ~/.local/state/lazydeck）
---| '"cache"' # 缓存目录（$XDG_CACHE_HOME/lazydeck，默认 ~/.cache/lazydeck）

---获取 lazydeck 目录的绝对路径。
---
---按 XDG Base Directory 规范解析：环境变量只在值为绝对路径时生效，否则回退到
---标准默认位置。每次调用都会重新从 Rust 侧解析，不缓存，因此运行时改动的环境
---变量会立即生效。
---@param kind deck.stdpath.kind
---@return string path
function deck.stdpath(kind) return _deck.stdpath(kind) end
