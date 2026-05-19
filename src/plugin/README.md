# Plugin System

lazydeck 的插件系统基于 Lua 运行时，允许用户通过 Lua 脚本扩展应用功能。

## 目录结构

```
src/plugin/
├── mod.rs      # 模块声明
├── lua.rs      # Lua 初始化和预设加载
├── scope.rs    # 作用域管理函数
└── deck/         # Lua API 子模块
    ├── api.rs          # 页面管理 API
    ├── cache.rs        # 缓存系统
    ├── fs.rs           # 文件系统操作
    ├── highlighter.rs  # 语法高亮
    ├── http.rs         # HTTP 客户端
    ├── http_server.rs  # 本地 HTTP 服务
    ├── keymap.rs       # 键盘映射
    ├── path.rs         # 路径操作
    ├── secrets.rs      # secrets 读写
    ├── style.rs        # UI 样式
    ├── system.rs       # 系统命令执行
    ├── time.rs         # 时间解析和格式化
    └── url.rs          # URL 编解码
```

## 核心组件

### lua.rs - Lua 初始化

负责初始化 Lua 环境和加载预设文件：

- `init_lua()` - 初始化 Lua 环境并设置配置基准目录
- 预设加载顺序包含 `system.lua`、`json.lua`、`promise.lua`、`config.lua` 等基础模块
- `package.path` 由 `preset/lua/config.lua` 根据 `plugins` 配置动态追加

其中 `promise.lua` 属于内置预设，启动后会直接注册全局 `Promise`；`require('promise')` 仅作为兼容旧代码的别名。

**插件路径搜索顺序**（`package.path`）：

| 来源 | 路径 | 说明 |
|------|------|------|
| 本地（显式 `dir`） | 配置中的相对/绝对路径 | 通过 `{ dir = '...' }` 注入到 `package.path` |
| **远程** | `~/.local/share/lazydeck/plugins/` | **从 GitHub 下载的插件** |
| 预设 | `preset/lua/` | 内置预设脚本（嵌入二进制） |

**远程插件目录结构**：
```
~/.local/share/lazydeck/plugins/
└── owner-plugin.lazydeck/
    └── owner-plugin/
        └── init.lua       # 插件入口
~/.config/lazydeck/plugins.lock        # 插件版本锁文件
```

加载预设文件（debug 模式从文件读取，release 模式从嵌入的二进制读取）：

### scope.rs - 作用域管理

提供 Lua 与 Rust 状态交互的桥梁：

```rust
scope(lua, state, sender, || {
    // Lua 代码可以访问 state 和 sender
})?
```

关键函数：
- `scope()` - 在作用域中执行 Lua 代码
- `borrow_scope_state()` - 不可变访问状态
- `mut_scope_state()` - 可变访问状态
- `send_render_event()` - 触发渲染更新
- `send_command()` - 发送内部命令

## LC API

全局表 `deck` 提供以下子系统：

### deck.api - 页面管理

页面 entry 支持的常用字段：

- `key: string` - 唯一标识
- `display?: string|Span|Line` - 列表区显示内容
- `bottom_line?: string|Span|Line` - 当 entry 被 hover 时，渲染在底部左侧的一行
- `keymap?: table` - entry 局部快捷键
- `preview?: function` - entry 局部预览回调

| 函数 | 说明 |
|------|------|
| `set_entries(path, entries)` | 设置指定页面的条目列表；`path=nil` 表示当前页面，`entries=nil` 会清空 Rust 侧页面 |
| `get_entries(path)` | 获取指定页面完整条目列表（过滤前）；`path=nil` 表示当前页面 |
| `get_hovered()` | 获取当前悬停条目 |
| `set_hovered(path)` | 按完整路径设置当前悬停条目 |
| `set_preview(path, preview)` | 设置指定悬停路径的预览；`path=nil` 表示当前悬停项，`preview=nil` 会清空 Rust 侧缓存/预览 |
| `go_to(path)` | 导航到指定路径 |
| `get_current_path()` | 获取当前路径 |
| `get_hovered_path()` | 获取悬停项路径 |
| `argv()` | 获取命令行参数 |
| `get_filter()` | 获取当前过滤条件 |
| `get_available_keymaps()` | 获取当前上下文可用快捷键 |
| `enter_filter_mode()` | 进入过滤模式 |
| `exit_filter_mode()` | 退出过滤模式 |
| `accept_filter()` | 应用过滤 |
| `deck.hook.pre_reload(cb)` | 添加重载前钩子 |
| `append_hook_pre_quit(cb)` | 添加退出前钩子（Lua 侧封装为 `deck.hook.pre_quit`） |

### deck.cache - 缓存系统

`deck.api` 里的路径数组都使用“原始 segment”：

- Lua 插件调用 `go_to({ ... })` 时不需要手动做 URL 编码
- `get_current_path()` / `get_hovered_path()` 返回的也是解码后的原始 segment
- 当路径需要显示为字符串（例如 header）或从字符串解析（例如命令行初始路径、`cd` 命令）时，Rust 侧会自动处理 percent 编解码

基于 JSON 文件的持久化缓存。缓存按 namespace 分文件存储，避免不同插件 key 冲突，也避免所有缓存共用单个大文件反复读写：

| 函数 | 说明 |
|------|------|
| `cache.get(namespace, key)` | 获取缓存值 |
| `cache.set(namespace, key, value, opts)` | 设置缓存值（支持 TTL） |
| `cache.delete(namespace, key)` | 删除缓存 |
| `cache.clear(namespace)` | 清空指定 namespace 的缓存 |

```lua
-- 使用示例
deck.cache.set("github.releases", "user_data", {name = "test"}, {ttl = 3600})  -- TTL 为秒
local data = deck.cache.get("github.releases", "user_data")
```

### deck.fs - 文件系统

文件系统操作：

| 函数 | 说明 |
|------|------|
| `fs.read_dir_sync(path)` | 读取目录（返回 name / is_dir / size 等字段） |
| `fs.read_file(path, [opts], callback)` | 异步读取文件内容，可限制最大字符数 |
| `fs.read_file_sync(path)` | 读取文件内容 |
| `fs.write_file_sync(path, content)` | 写入文件（支持任意 Lua 字符串字节） |
| `fs.stat(path)` | 获取文件状态（包含 exists / is_file / is_dir / size / readable / writable / executable） |
| `fs.mkdir(path)` | 创建目录 |

`fs.stat()` 返回的表包含：
- `exists` - 文件是否存在
- `is_file` - 是否为文件
- `is_dir` - 是否为目录
- `size` - 文件大小（字节，若可获取）
- `is_readable` - 是否可读
- `is_writable` - 是否可写
- `is_executable` - 是否可执行

### deck.secrets - Secrets 存储

用于保存敏感字符串，按 namespace 分文件存储到 `~/.config/lazydeck/secrets/`。和 `deck.cache` 不同，`deck.secrets` 只接受字符串值，不支持 TTL。

| 函数 | 说明 |
|------|------|
| `secrets.get(namespace, key)` | 获取 secret 值，不存在时返回 `nil` |
| `secrets.set(namespace, key, value)` | 保存 secret 字符串 |
| `secrets.delete(namespace, key)` | 删除 secret |

```lua
deck.secrets.set("github", "token", "ghp_xxx")
local token = deck.secrets.get("github", "token")
```

### deck.base64 - Base64 编解码

Base64 编解码：

| 函数 | 说明 |
|------|------|
| `base64.encode(data)` | Base64 编码 |
| `base64.decode(encoded)` | Base64 解码为 Lua 字符串 |

```lua
local encoded = deck.base64.encode("hello")
local decoded = deck.base64.decode(encoded)
deck.fs.tempfile({ suffix = ".bin", content = decoded })
```

### deck.http - HTTP 客户端

基于 reqwest 的异步 HTTP 客户端：

| 函数 | 说明 |
|------|------|
| `http.get(url, callback)` | GET 请求 |
| `http.post(url, body, callback)` | POST 请求 |
| `http.put(url, body, callback)` | PUT 请求 |
| `http.delete(url, callback)` | DELETE 请求 |
| `http.patch(url, body, callback)` | PATCH 请求 |
| `http.request(opts, callback)` | 通用请求 |

回调接收的响应对象：
```lua
function on_response(response)
    -- response.success  - 请求是否成功
    -- response.status   - HTTP 状态码
    -- response.body     - 响应体
    -- response.headers  - 响应头
    -- response.error    - 错误信息
end
```

### deck.http_server - 本地 HTTP 服务

为插件提供本地稳定 URL。Rust 负责监听 `127.0.0.1` 端口和路由，Lua 负责注册 resolver 并异步返回响应。

| 函数 | 说明 |
|------|------|
| `http_server.register_resolver(name, handler)` | 注册 resolver；`handler(request, respond)` 可稍后调用 `respond({...})` |
| `http_server.unregister_resolver(name)` | 注销 resolver |
| `http_server.url(name, params)` | 生成本地 URL，例如 `http://127.0.0.1:38173/r/song?id=123` |
| `http_server.info()` | 返回 `{ host, port, base_url }` |

请求对象：
```lua
{
  method = 'GET',
  path = '/r/song',
  query = { id = '123' },
  params = { id = '123' },
  headers = { host = '127.0.0.1:38173' },
}
```

响应对象：
```lua
{
  status = 307,
  headers = {
    Location = 'https://example.com/signed-url',
  },
  body = 'optional text body',
}
```

### deck.html - HTML 解析

基于 CSS selector 的 HTML 解析能力，适合从网页里提取结构化内容。

| 函数 | 说明 |
|------|------|
| `html.parse(source)` | 按完整 HTML 文档解析，返回 `HtmlDocument` userdata |
| `html.parse_fragment(source)` | 按 HTML 片段解析，返回 `HtmlDocument` userdata |

`HtmlDocument` 方法：

| 方法 | 说明 |
|------|------|
| `doc:select(selector)` | 返回 `HtmlNodeList` |
| `doc:first(selector)` | 返回首个 `HtmlNode`，不存在时为 `nil` |
| `doc:html()` | 返回原始 HTML |

`HtmlNode` 方法：

| 方法 | 说明 |
|------|------|
| `node:name()` | 标签名 |
| `node:html()` | 节点 outer HTML |
| `node:inner_html()` | 节点 inner HTML |
| `node:text()` | 节点及后代文本拼接结果 |
| `node:attr(name)` | 获取单个属性 |
| `node:attrs()` | 获取全部属性表 |
| `node:select(selector)` | 在当前节点片段内继续查询 |
| `node:first(selector)` | 在当前节点片段内查询首个匹配 |

`HtmlNodeList` 方法：

| 方法 | 说明 |
|------|------|
| `list:len()` | 返回节点数 |
| `list:get(index)` | 1-based 获取节点 |
| `list:to_table()` | 转成 Lua 数组 |

### deck.url - URL 编解码

用于 URL 百分号编码和解码：

| 函数 | 说明 |
|------|------|
| `url.encode(value)` | 对字符串做百分号编码 |
| `url.decode(value)` | 解码百分号编码字符串 |

```lua
local encoded = deck.url.encode("hello world")
local decoded = deck.url.decode(encoded)
```

### deck.keymap - 键盘映射

| 函数 | 说明 |
|------|------|
| `deck.keymap.set(mode, key, callback[, opt])` | 设置键盘映射 |

```lua
deck.keymap.set('main', 'q', function() deck.cmd('quit') end)
deck.keymap.set('main', 'j', 'scroll_by 1')
deck.keymap.set('input', '<C-k>', function() deck.notify('input keymap hit') end)
deck.keymap.set('input', '<enter>', 'input_submit')
deck.keymap.set('main', '<C-x>', function() ... end)
deck.keymap.set('main', '?', function() ... end, { desc = 'help' })
deck.keymap.set('main', 'p', function() paste() end, { once = true, desc = 'paste once' })
```

- `mode` 支持 `main` / `m` 和 `input` / `i`
- 输入框中 `Backspace`、`Left`、`Right` 为 Rust 内置键位
- 其余默认动作通过 `preset/lua/config.lua` 用 `deck.keymap.set('input', ...)` 注册到内部命令或 Lua 回调，例如 `input_submit`、`input_cancel`；默认 `<C-g>` 通过 `deck.system.edit(...)` 调用外部编辑器编辑当前输入内容

`deck.config` 还支持 `keymap` 字段来覆盖内置主模式键位，例如：

```lua
deck.config {
  keymap = {
    enter = '<enter>',
    filter = '/',
    quit = 'q',
  },
}
```

支持的键位名包括 `up`、`down`、`top`、`bottom`、`preview_up`、`preview_down`、`reload`、`history_back`、`history_forward`、`quit`、`force_quit`、`filter`、`clear_filter`、`back`、`open`、`enter`，以及 `input_submit`、`input_cancel`、`input_clear_before_cursor`、`input_cursor_to_start`、`input_cursor_to_end`、`input_external_editor`。每次调用 `deck.config` 都会按这些配置重新执行一遍 `deck.keymap.set`。

插件 spec 也可以定义全局 `keys`：

```lua
{
  'owner/myplugin.lazydeck',
  keys = {
    { 'x', function() run_my_action() end, desc = 'run action' },
  },
}
```

按下配置的按键时，会先懒加载对应插件并执行其 `config/setup`，然后再调用回调。

页面 entry 也可以定义局部 keymap：

```lua
{
  key = "container-1",
  keymap = {
    ["d"] = { callback = function() delete_container("container-1") end, desc = "delete" },
    ["gg"] = function() open_logs("container-1") end,
  },
}
```

- `entry.keymap` 会通过 Lua 表访问，支持由元表 `__index` 提供
- key 是按键序列字符串，value 可以是 Lua 函数，或 `{ callback = fn, desc = "..." }`
- 优先级为：`entry.keymap` > `opt.once = true` 的一次性 keymap > 普通全局 keymap
- `opt.once = true` 时，该全局 keymap 完整触发一次后会自动删除；如果存在相同按键的普通全局 keymap，删除后会恢复到普通全局 keymap
- `opt.desc` 可为全局 keymap 提供帮助面板中的描述文本

`deck.api` 额外提供当前上下文可用快捷键查询：

```lua
local items = deck.api.get_available_keymaps()
for _, item in ipairs(items) do
  print(item.key, item.desc, item.source)
end
```

页面 entry 也可以定义局部 preview：

```lua
{
  key = "song-1",
  preview = function(self, cb)
    cb(deck.style.text {
      deck.style.line { "Preview for ", self.key },
    })
  end,
}
```

- `entry.preview(cb)` 的优先级高于插件级 `plugin.preview(entry, cb)`
- `preview` 可以是 `string`、`Span`、`Line`、`Text`、`Image`，也可以是这些类型组成的数组，按顺序在预览区渲染
- `Image` 可以传本地路径或 HTTP(S) URL；URL 会先显示占位文本，后台下载完成后自动刷新
- `Image` 会优先走终端原生图片协议（当前支持 Kitty / iTerm Inline），不支持时退回 truecolor 块字符
- 当图片在预览区内被滚动裁切时，会临时退回块字符 fallback，避免 native 协议定位错误
- `entry.preview` 可以异步调用 `cb(preview)`，也可以直接返回一个 preview widget 作为同步结果
- `entry.preview` 同样通过 Lua 表访问，支持由元表 `__index` 提供
- 当异步回调返回时，如果当前 hovered entry 已经变化，这次 preview 更新会被丢弃

### deck.path - 路径操作

| 函数 | 说明 |
|------|------|
| `deck.path.split(path)` | 分割路径为数组 |
| `deck.path.join(path_list)` | 合并路径数组 |

### deck.style - UI 样式

创建 TUI 组件的函数：

| 函数 | 说明 |
|------|------|
| `deck.style.span(s)` | 创建单个 Span |
| `deck.style.line(args)` | 创建 Line（Span 数组） |
| `deck.style.text(args)` | 创建 Text（Line 数组） |
| `deck.style.image(path_or_url[, opts])` | 创建图片预览 widget，支持本地路径或 HTTP(S) URL；`opts` 支持 `max_width` / `max_height`（终端格），未显式指定时读取 `deck.config().image` 默认值 |
| `deck.style.highlight(code, lang)` | 语法高亮代码 |
| `deck.style.ansi(s)` | 解析 ANSI 转义序列 |
| `deck.style.align_columns(lines)` | 对齐列 |

`Span` / `Line` userdata 支持的方法：
- `:fg(color)` / `:bg(color)` - 设置颜色
- `:bold()` / `:italic()` / `:underline()` - 添加文本样式

### deck.system - 系统命令

| 函数 | 说明 |
|------|------|
| `deck.system.executable(cmd)` | 检查命令是否可执行 |
| `deck.system.spawn(cmd)` | 启动后台命令并返回 pid |
| `deck.system.kill(pid[, signal])` | 向进程发送信号，默认 `SIGTERM` |
| `deck.system.open(path)` | 用默认应用打开文件 |
| `deck.system.edit(opts[, callback])` | 用外部编辑器编辑文件内容；传 `path` 时直接原地编辑该文件；不传 `path` 时可用 `ext` 指定临时文件后缀以启用语法高亮；传 callback 时回调接收 `(content, error)`，不传时 Rust 不会读取编辑后内容 |
| `deck.system.exec(opts)` | 异步执行命令 |
| `deck.system.interactive(opts)` | 执行交互式命令 |

### deck.socket - 长连接 Socket

| 函数 | 说明 |
|------|------|
| `deck.socket.connect(addr)` | 连接 socket，返回可复用连接对象 |

连接对象方法：
- `sock:on_line(cb)` - 注册逐行回调
- `sock:write(message)` - 写入一条消息（自动补 `\n`）
- `sock:close()` - 关闭连接

### deck.time - 时间处理

| 函数 | 说明 |
|------|------|
| `deck.time.parse(str)` | 解析时间字符串为 Unix 时间戳 |
| `deck.time.now()` | 获取当前 Unix 时间戳 |
| `deck.time.format(ts, fmt)` | 格式化时间戳 |

支持的时间格式：
- ISO 8601: `2023-12-25T15:30:45Z`
- RFC 3339: `2023-12-25T15:30:45+08:00`
- RFC 2822: `Mon, 25 Dec 2023 15:30:45 +0800`
- 日期: `2023-12-25`
- 紧凑格式: `compact`（自动适配显示格式）
- 相对时间格式: `relative`（例如 `47 minutes ago`、`yesterday`、`last week`、`in 2 hours`）

### deck.cache - 缓存

| 函数 | 说明 |
|------|------|
| `deck.cache.get(namespace, key)` | 获取缓存 |
| `deck.cache.set(namespace, key, value, opts)` | 设置缓存 |
| `deck.cache.delete(namespace, key)` | 删除缓存 |
| `deck.cache.clear(namespace)` | 清空指定 namespace 的缓存 |

### 其他函数

| 函数 | 说明 |
|------|------|
| `deck.defer_fn(fn, ms)` | 延迟执行函数 |
| `deck.cmd(cmd)` | 发送内部命令 |
| `deck.split(s, sep)` | 分割字符串 |
| `deck.log(level, msg, ...)` | 写入日志 |
| `deck.osc52_copy(text)` | 通过 OSC 52 复制到剪贴板 |
| `deck.tbl_extend(behavior, ...)` | 浅合并多个表 |
| `deck.tbl_deep_extend(behavior, ...)` | 深合并多个表 |
| `deck.deep_equal(a, b)` | 深度比较两个值 |
| `deck.tbl_map(func, t)` | 映射表值 |
| `deck.tbl_filter(func, t)` | 过滤列表 |
| `deck.list_extend(dst, src)` | 追加列表内容 |
| `deck.notify(msg)` | 显示通知 (支持 string、Span、Line 或 Text 类型) |
| `deck.confirm(opts)` | 显示确认对话框 |
| `deck.select(opts, callback)` | 显示选择对话框 |
| `deck.input(opts)` / `deck.input.show(opts)` | 显示输入对话框 |
| `deck.input.get()` | 获取当前输入对话框文本；未打开时返回 `nil` |
| `deck.input.set(value)` | 设置当前输入对话框文本，并触发 `on_change`；未打开时抛错 |

## 内部命令

通过 `deck.cmd()` 发送：

| 命令 | 说明 |
|------|------|
| `quit` | 退出应用 |
| `scroll_by [n]` | 滚动列表 n 行 |
| `scroll_preview_by [n]` | 滚动预览 n 行 |
| `reload` | 刷新当前列表 |
| `enter_filter_mode` | 进入过滤模式 |
| `input_submit` | 提交输入框 |
| `input_cancel` | 取消输入框 |
| `input_clear_before_cursor` | 删除光标前所有文本 |
| `input_cursor_to_start` | 输入框光标移动到开头 |
| `input_cursor_to_end` | 输入框光标移动到结尾 |
| `exit_filter_mode` | 退出过滤模式 |
| `accept_filter` | 应用过滤 |
| `filter_clear` | 清除过滤 |

## 语法高亮

使用 syntect 库支持 180+ 种语言：

```lua
local code = [[
function hello() {
    print("Hello World");
}
]]
local highlighted = deck.style.highlight(code, "javascript")
deck.api.set_preview(nil, highlighted)
```

## 使用示例

```lua
-- 自定义插件示例
local M = {}

function M.setup()
    -- 设置键盘映射
    deck.keymap.set('main', 'r', function()
        deck.cmd('reload')
    end)
    
    -- 异步获取数据
    deck.http.get("https://api.example.com/data", function(resp)
        if resp.success then
            local data = deck.json.decode(resp.body)
            -- 处理数据
        end
    end)
end

function M.meta()
    return {
        icon = "󰏗",
        desc = "自定义插件描述",
        color = "cyan",
    }
end

function M.list(path, cb)
    -- 列出目录内容
    deck.fs.read_dir_sync(path, function(entries, err)
        if err then
            cb({})
            return
        end
        -- 转换为 PageEntry 格式
        local result = {}
        for _, e in ipairs(entries) do
            table.insert(result, {
                key = e.name,
                display = e.is_dir and e.name .. "/" or e.name
            })
        end
        cb(result)
    end)
end

return M
```
