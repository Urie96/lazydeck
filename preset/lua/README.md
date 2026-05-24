# Preset Lua Scripts

本目录包含 lazydeck 的预设 Lua 脚本，这些脚本在应用启动时加载，为插件提供基础工具函数和 API 封装。

## 目录结构

```
preset/lua/
├── api.lua           # 页面管理 API 封装
├── base64.lua        # Base64 编解码
├── cache.lua         # 缓存 API 封装
├── clipboard.lua     # 系统剪贴板访问
├── component.lua     # UI 组件（对话框、通知）
├── copy_from_neovim.lua # 从 Neovim 复用的表工具函数
├── fs.lua            # 文件系统 API 封装
├── hash.lua          # Hash API 封装（MD5）
├── html.lua          # HTML 解析 API 封装
├── http.lua          # HTTP API 封装
├── http_server.lua   # 本地 HTTP 服务封装
├── init.lua          # 初始化脚本（默认配置和键盘映射）
├── inspect.lua       # 调试工具（表结构可视化）
├── interactive.lua   # 交互式命令封装
├── json.lua          # JSON 编解码
├── keymap.lua        # 键盘映射 API 封装
├── promise.lua       # 内置 Promise 实现与全局变量
├── secrets.lua       # secrets API 封装
├── socket.lua        # 长连接 socket 封装
├── string.lua        # 字符串扩展方法
├── style.lua         # 样式 API 封装
├── system.lua        # 系统命令 API 封装
├── time.lua          # 时间 API 封装
├── url.lua           # URL 编解码
├── util.lua          # 工具函数
├── yaml.lua          # YAML 编解码
└── global.d.lua      # 类型声明文件
```

## 概述

preset/lua 目录中的脚本是 Rust 后端 API 的 Lua 封装层。它们：

1. **提供更友好的 Lua API** - 封装底层 Rust 实现，添加类型注解和文档
2. **增加功能性** - 如 `json.lua`、`inspect.lua` 提供完整的 JSON 处理和调试能力
3. **统一接口** - 为不同调用方式提供统一的封装（如 `interactive.lua`、`system.lua`）

底层 Rust API 实现请参考 [src/plugin/README.md](../src/plugin/README.md)。

## 模块说明

### api.lua - 页面管理

封装页面和导航相关的 API。页面 entry 常用字段包括：

- `key: string` - 唯一标识
- `display?: string|Span|Line` - 列表区显示内容
- `bottom_line?: string|Span|Line` - 当前 entry 被 hover 时显示在底部左侧的一行
- `keymap?: table` - entry 局部快捷键
- `preview?: function` - entry 局部预览回调
- `selectable?: boolean` - 是否允许参与页面级选择

```lua
deck.api.set_entries(path, entries)    -- path=nil 为当前页面，entries=nil 清空 Rust 侧页面
deck.api.get_entries(path)             -- path=nil 为当前页面
deck.api.get_hovered()                 -- 获取当前悬停项
deck.api.set_hovered(path)             -- 按完整路径设置当前悬停项
deck.api.set_preview(path, widget)     -- path=nil 为当前悬停项，widget=nil 清空 Rust 侧预览缓存
deck.api.go_to(path)                   -- 导航到路径
deck.api.get_current_path()            -- 获取当前路径
deck.api.get_hovered_path()            -- 获取悬停项完整路径
deck.api.get_selected()                -- 获取当前页面选中的 entries；若没有选中则返回当前 hovered entry
deck.api.toggle_selected()             -- 切换当前 hovered entry 的选中状态，并自动下移一项
deck.api.clear_selected()              -- 清空当前页面选中状态
deck.api.argv()                        -- 获取命令行参数
deck.api.get_filter()                  -- 获取当前过滤字符串
deck.api.set_filter()                  -- 设置当前过滤字符串
deck.hook.pre_reload(cb)               -- 添加重载前钩子
```

### cache.lua - 缓存系统

持久化缓存的 Lua 封装：

```lua
deck.cache.get(namespace, key)           -- 获取缓存
deck.cache.set(namespace, key, value, opts)  -- 设置缓存（支持 TTL，支持 refresh_on_get）
deck.cache.delete(namespace, key)        -- 删除缓存
deck.cache.clear(namespace)              -- 清空指定 namespace 的缓存
```

### clipboard.lua - 系统剪贴板

系统剪贴板访问封装：

```lua
deck.clipboard.get()         -- 获取剪贴板内容
deck.clipboard.set(text)     -- 设置剪贴板内容
```

### base64.lua - Base64 编解码

Base64 相关封装：

```lua
deck.base64.encode("hello")                  -- 编码
deck.base64.decode("aGVsbG8=")               -- 解码为 Lua 字符串
local path = deck.fs.tempfile({ suffix = ".bin", content = deck.base64.decode("...") }) -- 解码后写入临时文件
```

### component.lua - UI 组件

对话框和通知组件：

```lua
deck.select(opts, on_selection)  -- 选择对话框
deck.confirm(opts)              -- 确认对话框
deck.notify(message)            -- 通知消息 (支持 string、Span、Line 或 Text 类型)
deck.input(opts)                -- 显示输入对话框
deck.input.show(opts)           -- 同上
deck.input.get()                -- 获取当前输入框文本，未打开时返回 nil
deck.input.set(value)           -- 设置当前输入框文本，并触发 on_change
deck.log(level, format, ...)   -- 写入日志
```

### fs.lua - 文件系统

文件系统操作封装：

```lua
deck.fs.read_dir_sync(path)     -- 读取目录（每项包含 name / is_dir / size）
deck.fs.read_file(path, callback)  -- 异步读取文件
deck.fs.read_file(path, { max_chars = 20000 }, callback)  -- 限制最多读取字符数
deck.fs.read_file_sync(path)    -- 读取文件
deck.fs.write_file_sync(path, content)  -- 写入文件（支持二进制内容）
deck.fs.stat(path)              -- 获取文件状态（包含 size 字段）
deck.fs.mkdir(path)             -- 创建目录
```

### secrets.lua - Secrets 存储

敏感字符串持久化封装：

```lua
deck.secrets.get(namespace, key)    -- 获取 secret
deck.secrets.set(namespace, key, value) -- 保存 secret
deck.secrets.delete(namespace, key) -- 删除 secret
```

### http.lua - HTTP 客户端

异步 HTTP 请求封装：

```lua
deck.http.get(url, callback)
deck.http.post(url, body, callback)
deck.http.put(url, body, callback)
deck.http.delete(url, callback)
deck.http.patch(url, body, callback)
deck.http.request(opts, callback)
```

### http_server.lua - 本地 HTTP 服务

本地 HTTP 服务封装，适合为外部进程或插件生成稳定 localhost URL：

```lua
deck.http_server.register_resolver('song', function(req, respond)
  respond {
    status = 307,
    headers = {
      Location = 'https://example.com/signed-url',
    },
  }
end)

local url = deck.http_server.url('song', { id = 123 })
local info = deck.http_server.info()
deck.http_server.unregister_resolver('song')
```

### hash.lua - Hash

Hash 相关封装：

```lua
deck.hash.md5('hello')  -- '5d41402abc4b2a76b9719d911017c592'
```

### html.lua - HTML 解析

HTML 文档/片段解析、CSS selector 查询，以及 HTML 转 Markdown 封装：

```lua
local doc = deck.html.parse(response.body)
local repos = doc:select("article.Box-row")
local first = repos[1]

if first then
  local link = first:first("h2 a")
  local href = link and link:attr("href")
  local text = link and link:text()
  local markdown = first:to_markdown()
end

local markdown = deck.html.to_markdown("<h1>Hello</h1><p>World</p>")
```

### path.lua - 路径操作

```lua
deck.path.split(path)          -- 分割路径为数组
deck.path.join(path_list)      -- 合并路径数组
deck.path.match(path, pattern) -- 判断 path 是否匹配 pattern，支持 * / **
```

### url.lua - URL 编解码

URL 百分号编码封装：

```lua
deck.url.encode("hello world")  -- "hello%20world"
deck.url.decode("hello%20world") -- "hello world"
```

### init.lua - 初始化脚本

应用启动时执行的默认初始化：

- 根据 `plugins` 配置把本地 `dir` 和远程安装目录加入 `package.path`
- 根路径 `/` 固定展示所有已配置插件，并从 `deck.cache` 读取插件 `icon` / `desc` 元信息用于展示
- 进入 `/plugin_name/...` 时懒加载该插件并执行其 `config/setup`
- 插件 spec 可设置 `lazy = false`，在 `deck.config` 调用时立即加载并执行 `config/setup`，适合通知历史等需要启动即初始化的插件
- 插件可选提供同步 `meta()` 函数返回 `{ icon = "󰏗", desc = "...", color = "cyan" }`；根路径展示插件时如果缓存不存在，会尝试加载插件并缓存 meta；插件不存在/`require` 失败时不写缓存，插件存在但未实现 `meta()` 时缓存空 meta 到 `lazydeck.plugin.meta` namespace
- 根据 `cfg.keymap` 注册默认主模式键盘映射
- 加载用户配置（默认 `require 'init'`，也支持通过 `LAZYDECK_CONFIG_FILE` 指定单个配置文件）
- 实现 `deck._list()` 和 `deck._preview()` 入口函数

默认配置中的 `keymap` 字段：

```lua
{
  up = '<up>',
  down = '<down>',
  top = 'gg',
  bottom = 'G',
  preview_up = '<pageup>',
  preview_down = '<pagedown>',
  reload = '<C-r>',
  history_back = '<C-o>',
  history_forward = '<C-i>',
  quit = 'q',
  command_prompt = ':',
  force_quit = '<C-q>',
  filter = '/',
  clear_filter = '<esc>',
  back = '<left>',
  open = '<right>',
  enter = '<enter>',
}
```

默认键盘映射：

| 按键 | 命令 |
|------|------|
| `↑` | 向上滚动 |
| `↓` | 向下滚动 |
| `gg` | 跳到开头 |
| `G` | 跳到结尾 |
| `<PageUp>` | 预览向上滚动 |
| `<PageDown>` | 预览向下滚动 |
| `Ctrl+r` | 刷新 |
| `Ctrl+o` | 跳回上一个访问页面 |
| `Ctrl+i` | 跳到下一个历史页面 |
| `:` | 打开命令输入框 |
| `q` | 退出 |
| `/` | 进入过滤模式 |
| `?` | 打开快捷键帮助 |
| `Esc` | 清除过滤 |
| `←` | 返回上级 |
| `→` / `Enter` | 进入目录 |

### inspect.lua - 调试工具

将任意 Lua 值转换为可读字符串（基于 `inspect.lua` 库）：

```lua
deck.inspect(value, options)
-- options: depth, newline, indent, process
```

### interactive.lua - 交互式命令

封装交互式命令执行，支持多种调用格式：

```lua
deck.interactive({"cmd", "arg1"})
deck.interactive({"cmd"}, callback)
deck.interactive({"cmd"}, {wait_confirm = true})
deck.interactive({"cmd"}, {wait_confirm = function(code) return code ~= 0 end})
deck.interactive({"cmd"}, {wait_confirm = true}, callback)
```

### json.lua - JSON 处理

完整的 JSON 编解码库（基于 rxi 的 json.lua）：

```lua
deck.json.encode(value)   -- Lua 值转 JSON 字符串
deck.json.decode(str)     -- JSON 字符串转 Lua 值
```

### promise.lua - Promise

启动时会直接加载内置 `Promise`，推荐直接使用全局变量：

```lua
Promise                -- 全局变量
```

兼容旧代码时，`require('promise')` 也会返回同一个 Promise 表；正常情况下不需要 `require`，也不需要再把 `promise` 作为插件加入 `deck.config.plugins`。

### keymap.lua - 键盘映射

设置键盘快捷键：

```lua
deck.keymap.set(mode, key, callback[, opt])
-- mode: "main" 或 "input"
-- key: 键序列（如 "j", "<C-d>", "<down>"）
-- callback: 命令字符串或回调函数
-- opt.desc: 可选描述，用于帮助面板
-- opt.path: 可选 page 路径 pattern，0 表示当前 page；数组里 "*" 匹配单个 segment，"**" 匹配零个或多个 segment
```

```lua
deck.keymap.set('main', '?', function() end, { desc = 'help' })
deck.keymap.set('main', 'p', function() end, { path = 0, desc = 'current page action' })
deck.keymap.set('main', 'd', function() end, { path = { 'docker', 'container' }, desc = 'delete container' })
deck.keymap.set('main', 'r', function() end, { path = '/mail/*', desc = 'mail page action' })
```

`deck.config` 支持通过 `keymap` 字段覆盖内置主模式快捷键：

```lua
deck.config {
  keymap = {
    enter = '<enter>',
    filter = '/',
    quit = 'q',
  },
}
```

输入框默认键位也通过 `deck.config.keymap` 配置，而不是写死在 Rust 中，例如：

```lua
deck.config {
  keymap = {
    input_submit = '<enter>',
    input_cancel = '<esc>',
    input_external_editor = '<C-g>',
  },
}
```

其中 `Backspace`、`Left`、`Right` 是 Rust 内置输入键位，不通过 `deck.config.keymap` 配置。默认 `input_external_editor` 为 `<C-g>`，由 `preset/lua/config.lua` 注册为 Lua keymap，通过 `deck.system.edit(...)` 调用外部编辑器编辑当前输入内容，优先使用 `$VISUAL`，其次 `$EDITOR`，否则回退到 `vi`。

支持的字段有：`up`、`down`、`top`、`bottom`、`preview_up`、`preview_down`、`reload`、`history_back`、`history_forward`、`quit`、`force_quit`、`command_prompt`、`filter`、`clear_filter`、`back`、`open`、`enter`、`input_submit`、`input_cancel`、`input_clear_before_cursor`、`input_cursor_to_start`、`input_cursor_to_end`、`input_external_editor`。每次调用 `deck.config` 都会根据当前 `keymap` 重新调用一遍 `deck.keymap.set`。

插件 spec 支持 `lazy = false`，用于在 `deck.config` 调用时立即加载并执行 `config/setup`；未设置时默认懒加载：

```lua
deck.config {
  plugins = {
    {
      'owner/notification-history.lazydeck',
      lazy = false,
    },
  },
}
```

插件 spec 也支持 `keys` 字段注册懒加载全局快捷键：

```lua
deck.config {
  plugins = {
    {
      'plugins/bookmarks.lazydeck',
      keys = {
        { 'ma', function() require('bookmarks').add() end, desc = 'add current page to bookmarks' },
      },
    },
  },
}
```

`keys` 是数组，元素格式为 `{ key, callback, desc = ... }`。按下对应按键时，lazydeck 会先加载该插件并执行其 `config/setup`，然后调用配置的函数。

通过 `:` 打开的命令输入框可以执行内部命令，例如：

```lua
cd /github/search
cd ../repo/lazygit
reload
history_back
```

其中 `cd` 支持绝对路径（以 `/` 开头）、相对路径，以及 `.` / `..`。

页面 entry 还可以定义 `keymap` 字段：

```lua
{
  key = "item",
  keymap = {
    ["x"] = { callback = function() print("entry local action") end, desc = "run action" },
  },
}
```

优先级顺序是：`entry.keymap` > page keymap > 普通全局 keymap。
page keymap 通过 `deck.keymap.set(..., { path = 0 })` 绑定到当前 page，或者用 `path = { ... }` 绑定到指定 page 路径 pattern；也可以直接传字符串如 `'/mail/*'`。`"*"` 匹配单个 segment，`"**"` 匹配零个或多个 segment。重复设置同一路径 pattern 会覆盖旧的 page keymap。
`entry.keymap` 的值也可以写成 `{ callback = fn, desc = "..." }`，供帮助面板展示描述。

可以通过 `deck.api.get_available_keymaps()` 获取当前上下文下可用的 entry/page/global 快捷键列表。

### plugin setup helper

可以主动触发某个已声明插件的 `setup/config`：

```lua
deck.plugin.load('mpv')
```

这会按 `deck.config.plugins` 里对应插件的配置执行一次 setup，适合一个插件依赖另一个插件的初始化结果时使用。

### hook helpers

```lua
deck.hook.pre_quit(function() ... end)
deck.hook.post_page_enter(function(ctx) print(vim.inspect(ctx.path)) end)
```

页面 entry 还可以定义 `preview` 字段：

```lua
{
  key = "item",
  preview = function(self, cb)
    cb(deck.style.text {
      deck.style.line { "Preview for " .. self.key },
    })
  end,
}
```

当光标停在该 entry 上且 `entry.preview` 存在时，会优先于插件级 `preview(entry, cb)`。如果回调执行时 hovered entry 已经变化，这次预览更新会被自动忽略。

`preview` 支持：
- `string` / `Span` / `Line` / `Text`
- `Image`
- 以上类型组成的数组，会在预览区按顺序渲染，适合图文混排

`Image` 可以传本地路径或 HTTP(S) URL。URL 会先在预览区显示占位文本，后台下载完成后自动刷新为真正的图片。

`Image` 会优先使用终端原生图片协议（当前支持 Kitty / iTerm Inline），不支持时退回 truecolor 块字符。

`entry.preview` 既可以异步调用 `cb(preview)`，也可以直接 `return preview` 返回同步结果：

```lua
{
  key = "item",
  preview = function(self)
    return deck.style.text {
      deck.style.line { "Immediate preview for " .. self.key },
    }
  end,
}
```

### string.lua - 字符串扩展

为字符串添加方法：

```lua
"text".fg("blue")       -- 设置前景色
"text":bold()           -- 加粗
"text":italic()         -- 斜体
"text":underline()      -- 下划线
"text":ansi()           -- 解析 ANSI 转义序列
"a,b,c":split(",")      -- 分割字符串
"  hello  ":trim()       -- 去除首尾空白
"你好世界":utf8_sub(1, 3)  -- UTF-8 字符截取
```

`utf8_sub()` 使用内置 `utf8` 库。

支持的颜色：`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`

### style.lua - 样式系统

创建 TUI 组件：

```lua
deck.style.span(s)              -- 创建 Span
deck.style.line({s1, s2, ...}) -- 创建 Line
deck.style.text({l1, l2, ...}) -- 创建 Text
deck.style.image(path, opts)   -- 创建 Image，默认读取 deck.config().image.max_width/max_height
deck.style.highlight(code, lang)  -- 语法高亮
deck.style.align_columns(lines)    -- 列对齐

deck.style.span("x"):bold()        -- Span 加粗
deck.style.line({"x"}):italic()    -- Line 斜体
deck.style.span("x"):underline()   -- Span 下划线

图片预览示例：

```lua
return {
  deck.style.line { "Cover" },
  deck.style.image("https://example.com/cover.png", { max_height = 20 }),
  "",
  deck.style.text { "Some description" },
}
```
```

### system.lua - 系统命令

执行外部命令：

```lua
deck.system.exec({cmd = {"ls", "-la"}, callback = function(out) end})
deck.system.exec({"ls", "-la"}, function(out) end)
local pid = deck.system.spawn({"mpv", "--idle=yes"})
deck.system.kill(pid) -- 默认发送 SIGTERM
deck.system.executable("rustc")  -- 检查命令是否存在
deck.system.open("file.txt")     -- 用默认应用打开文件
deck.system.edit({ path = "README.md" }, function(content, err)
  print(content, err)
end)
deck.system.edit({ content = "hello", ext = "lua" }, function(content, err)
  print(content, err)
end)
deck.system.edit({ content = "hello", ext = ".lua" })
```

### socket.lua - 长连接 Socket

复用 Unix socket 连接：

```lua
local temp_path = deck.fs.tempfile({ prefix = "test", suffix = ".sock" })
local sock = deck.socket.connect("unix:" .. temp_path)
sock:on_line(function(line) print(line) end)
sock:write("hello")
sock:close()
```

### time.lua - 时间处理

时间解析和格式化：

```lua
deck.time.now()                      -- 当前时间戳
deck.time.parse("2023-12-25T15:30:45Z")  -- 解析时间字符串
deck.time.format(1704067200)         -- 使用本地时区格式化时间戳
deck.time.format(1704067200, "compact")  -- 紧凑格式
deck.time.format(1704067200, "relative") -- 相对时间格式
```

### util.lua - 工具函数

通用工具函数：

```lua
deck.osc52_copy(text)        -- 复制到剪贴板
```

### copy_from_neovim.lua - 表工具函数

从 Neovim 代码中整理出的通用表操作函数：

```lua
deck.tbl_isempty(t)                     -- 判断表是否为空
deck.islist(t)                          -- 判断是否为连续数组
deck.tbl_extend('force', a, b, ...)     -- 浅合并多个表
deck.tbl_deep_extend('force', a, b, ...) -- 深合并多个表
deck.deep_equal(a, b)                   -- 深度比较
deck.tbl_map(func, t)                   -- 表值映射
deck.tbl_filter(func, t)                -- 过滤列表
deck.list_extend(dst, src)              -- 列表追加
```

### plugin_manager.lua - 插件管理核心

插件管理的核心逻辑，提供 GitHub 插件的安装、更新、锁文件管理等功能。挂载在 `deck._pm`：

```lua
-- 解析插件声明为标准化结构
-- 支持三种输入格式：字符串、表（单字符串）、表（完整配置）
local spec = deck._pm.parse_plugin_spec('owner/plugin.lazydeck')
-- spec.name = 'plugin'
-- spec.url = 'https://github.com/owner/plugin.lazydeck.git'
-- spec.install_path = '~/.local/share/lazydeck/plugins/plugin.lazydeck'
-- spec.config = auto-generated function() require('plugin').setup() end
-- spec.lazy = true

local local_spec = deck._pm.parse_plugin_spec({ dir = 'plugins/my-plugin.lazydeck' })
-- local_spec.name = 'my-plugin'
-- local_spec.dir = '<config-base>/plugins/my-plugin.lazydeck'
-- local_spec.is_remote = false

-- 展开插件列表（去重，保留首次出现顺序）
local flat = deck._pm.flatten_plugins(plugins)
-- flat[1].name = 'plugin-a'
-- flat[2].name = 'plugin-b'

-- 并行安装缺失的插件
deck._pm.install_missing(plugins, callback)

-- 并行更新所有插件（遵循约束）
deck._pm.update_all(plugins, callback)

-- 根据锁文件恢复插件
deck._pm.restore_all(plugins, callback)

-- 安装单个插件
deck._pm.install(spec, callback)

-- 更新单个插件
deck._pm.update(spec, callback)

-- 检查插件是否有更新
deck._pm.check_update(spec, callback)

-- 读取/写入锁文件
local lock = deck._pm.read_lock()
deck._pm.write_lock(lock)
```

### manager.lua - 插件管理器 UI

内置插件管理界面。启动 lazydeck 后自动加载。提供以下函数：

```lua
local manager = deck._manager
manager.setup(plugins)  -- 初始化并设置键盘映射
manager.list(path, cb)  -- 列出所有插件
manager.preview(entry, cb)  -- 显示插件详情和更新状态
```

### global.d.lua - 类型声明

Lua 语言服务器类型声明文件，为 IDE 提供类型提示。

## 加载顺序

预设文件按以下顺序加载（参见 `src/plugin/lua.rs`）：

1. `system.lua`
2. `copy_from_neovim.lua`
3. `socket.lua`
4. `component.lua`
5. `api.lua`
6. `style.lua`
7. `interactive.lua`
8. `string.lua`
9. `inspect.lua`
10. `json.lua`
11. `promise.lua`
12. `time.lua`
13. `keymap.lua`
14. `html.lua`
15. `http.lua`
16. `http_server.lua`
17. `cache.lua`
18. `fs.lua`
19. `hash.lua`
20. `util.lua`
21. `base64.lua`
22. `url.lua`
23. `clipboard.lua`
24. `secrets.lua`
25. `yaml.lua`
26. `plugin_manager.lua` ← 插件管理核心逻辑（提供 `deck._pm`）
27. `manager.lua` ← 插件管理器 UI（提供 `deck._manager`）
28. `config.lua` ← 最后加载，执行初始化逻辑

## 使用示例

```lua
-- 自定义插件示例

-- 使用 JSON 处理
local data = deck.json.encode({name = "test", value = 42})
local decoded = deck.json.decode(data)

-- 使用 HTTP 请求
deck.http.get("https://api.example.com/data", function(resp)
    if resp.success then
        local data = deck.json.decode(resp.body)
        -- 处理数据
    end
end)

-- 创建带样式的文本
local header = deck.style.line({
    deck.style.span("文件列表").fg("green"),
    deck.style.span(" (" .. count .. ")").fg("gray")
})
deck.api.set_preview(nil, deck.style.text({header, content}))

-- 语法高亮
local code = [[
function hello()
    print("Hello, World!")
end
]]
local highlighted = deck.style.highlight(code, "lua")
deck.api.set_preview(nil, highlighted)

-- 异步执行命令
deck.system.exec({"ls", "-la"}, function(out)
    print("Exit code:", out.code)
    print("Output:", out.stdout)
end)

-- 使用缓存
deck.cache.set("demo", "api_result", {data = "something"}, {ttl = 300})
deck.cache.set("demo", "api_result", {data = "something"}, {ttl = 300, refresh_on_get = true})
local cached = deck.cache.get("demo", "api_result")

-- 交互式确认
deck.confirm({
    title = "确认删除",
    prompt = "确定要删除这个文件吗？",
    on_confirm = function()
        -- 执行删除
    end,
    on_cancel = function()
        -- 取消操作
    end
})

-- 选择对话框
deck.select({
    prompt = "选择操作",
    options = {
        {value = "open", display = "📂 打开"},
        {value = "edit", display = "✏️ 编辑"},
        {value = "delete", display = "🗑️ 删除"}
    }
}, function(choice)
    if choice then
        -- 处理选择
    end
end)

-- 格式化时间
local timestamp = deck.time.now()
local formatted = deck.time.format(timestamp, "compact")  -- "14:30" 或 "03/15" 或 "2024/03"
local relative = deck.time.format(timestamp - 3600, "relative")  -- "1 hour ago"
```
