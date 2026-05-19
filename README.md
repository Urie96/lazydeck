# lazydeck

一个基于 Rust + Lua 的终端 UI (TUI) 文件管理器/命令面板，灵感来源于 [yazi](https://github.com/sxyazi/yazi)。

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## 特性

- 🚀 **高性能** - 基于 Rust 构建，异步事件驱动
- 🔌 **Lua 插件系统** - 使用 LuaJIT 脚本语言扩展功能
- 🎨 **语法高亮** - 内置 180+ 种编程语言语法高亮支持
- 🖥️ **现代化 UI** - 使用 ratatui 构建的美观终端界面
- ⌨️ **可配置键位导航** - 默认支持方向键、`gg`/`G`、`/` 等，并可通过 `deck.config.keymap` 覆盖
- 💾 **页面缓存** - 目录导航时保持状态和滚动位置

## 预览

```
/docker/container
╭────────────────────────────────┬───────────────────────────────╮
│ intelligent_benz redis:alpine  │🆔 ID:         15eb56799f61    │
│myalpine         alpine       │📊 State:      running         │
│ naughty_allen    alpine        │ℹ️ Status:     Up 23 hours     │
│ pedantic_jones   alpine        │⌨️ Command:    sh -c echo      │
│ silly_khayyam    alpine        │'容器启动了' && tail -f        │
│ dreamy_mcnulty   alpine        │/dev/null                      │
│                                │🚪 Entrypoint:                 │
│                                │📅 Created:    2026-03-07      │
│                                │15:45:18 +0800 CST             │
│                                │                               │
│                                │Logs:                          │
│                                │容器启动了                     │
│                                │容器启动了                     │
│                                │                               │
╰────────────────────────────────┴───────────────────────────────╯
```

## 安装

### 从源码编译

```bash
# 克隆仓库
git clone https://github.com/urie/lazydeck.git
cd lazydeck

# 构建
cargo build --release

# 运行
cargo run --release

# 查看帮助
cargo run --release -- --help

# 查看版本
cargo run --release -- --version

# 运行并直接进入指定页面
cargo run --release -- /docker/container
```

## 项目结构

```
lazydeck/
├── src/                    # Rust 源代码
│   ├── main.rs            # 入口点
│   ├── app.rs             # 主应用逻辑
│   ├── state.rs           # 状态管理
│   ├── events.rs          # 事件系统
│   ├── keymap.rs          # 键盘映射
│   ├── plugin/            # Lua 插件系统
│   │   ├── lua.rs        # Lua 初始化和路径配置
│   │   ├── scope.rs      # 作用域管理
│   │   └── deck/           # Lua API 实现
│   └── widgets/           # UI 组件
└── preset/                # 预设文件
    ├── lua/              # Lua 预设脚本
    │   ├── config.lua     # 初始化脚本
    │   ├── plugin_manager.lua  # 插件管理核心逻辑
    │   ├── manager.lua    # 插件管理器 UI
    │   ├── promise.lua    # 内置 Promise
    │   └── ...           # 其他工具模块
    ├── syntaxes/         # 语法定义文件
    └── themes/           # 颜色主题
```

## 核心概念

### 插件系统

lazydeck 的核心功能通过 Lua 插件实现。每个插件是一个包含 `init.lua` 的目录：

```
plugins/owner/myplugin.lazydeck/
└── myplugin/
    └── init.lua
```

插件需要导出以下函数：

```lua
-- init.lua
local M = {}

-- 初始化函数（可选）
function M.setup()
    -- 设置键盘映射等
end

-- 插件元信息（可选，同步返回；进入插件页后会缓存到 deck.cache 供根页面显示）
function M.meta()
    return {
        icon = "󰏗",       -- Nerd Font 图标
        desc = "插件描述",
        color = "cyan",   -- icon 前景色
    }
end

-- 列出条目（必需）
function M.list(path, cb)
    -- path 为绝对路径，例如 {'docker', 'container'}
    -- 获取条目列表
    -- cb(entries) 回调传递结果
end

-- 预览条目（可选）
function M.preview(entry, cb)
    -- 设置预览内容
    -- cb(preview_widget) 回调传递预览
end

return M
```

### PageEntry 格式

```lua
{
    key = "item_name",           -- 必需：唯一标识
    display = "显示文本",         -- 可选：显示文本，默认使用 key
    -- 自定义字段...
}
```

### LC API

全局表 `deck` 提供丰富的 API：

| 模块        | 功能              |
| ----------- | ----------------- |
| `deck.api`    | 页面管理、导航    |
| `deck.fs`     | 文件系统操作      |
| `deck.http`   | HTTP 请求         |
| `deck.html`   | HTML 解析与选择器 |
| `deck.system` | 执行外部命令      |
| `deck.cache`  | 持久化缓存        |
| `deck.time`   | 时间解析格式化    |
| `deck.style`  | UI 样式和语法高亮 |
| `deck.keymap` | 键盘映射          |
| `deck.json`   | JSON 编解码       |
| `deck.cmd`    | 发送内部命令      |

另外内置全局 `Promise`，正常使用时直接访问即可，不需要 `require`。

## 内置插件

lazydeck 自带多个示例插件：

| 插件       | 说明             |
| ---------- | ---------------- |
| `process`  | 进程管理器       |
| `memos`    | Memos 笔记客户端 |
| `himalaya` | 邮件客户端       |
| `systemd`  | systemd 服务管理 |
| `docker`   | Docker 容器管理  |
| `aria2`    | aria2 下载管理   |
| `rclone`   | rclone 远程管理  |

## 键盘快捷键

### 主模式

| 按键                  | 功能         |
| --------------------- | ------------ |
| `↑` / `↓` / `j` / `k` | 上下移动     |
| `gg`                  | 跳到开头     |
| `G`                   | 跳到结尾     |
| `/`                   | 进入过滤模式 |
| `:`                   | 打开命令输入框 |
| `Enter` / `→`         | 进入目录     |
| `←`                   | 返回上级     |
| `q`                   | 退出         |
| `Ctrl+r`              | 刷新         |
| `Ctrl+o`              | 跳回上一个访问页面 |

### 过滤模式

| 按键     | 功能         |
| -------- | ------------ |
| 任意字符 | 输入过滤文本 |
| `Enter`  | 应用过滤     |
| `Esc`    | 退出过滤模式 |
| `Ctrl+u` | 清空过滤     |

## 配置

在 `config/init.lua` 中配置（对应 `~/.config/lazydeck/init.lua`）：

```lua
deck.config {
  keymap = {
    enter = '<enter>',
    filter = '/',
    quit = 'q',
  },
  plugins = {
    -- 远程插件字符串语法
    'owner/process.lazydeck',
    'owner/memos.lazydeck',

    -- 完整表格式
    {
      'owner/myplugin.lazydeck',
      config = function()
        require('myplugin').setup { option = value }
      end,
      keys = {
        { 'x', function() require('myplugin').action() end, desc = 'run my action' },
      },
    },
    {
      'plugins/bookmarks.lazydeck',
      keys = {
        { 'ma', function() require('bookmarks').add() end, desc = 'add current page to bookmarks' },
      },
    },

    -- 本地目录插件：必须显式使用 dir
    {
      dir = 'plugins/myplugin.lazydeck',   -- 相对路径基于 ~/.config/lazydeck/
    },
    {
      'myplugin',
      dir = '/absolute/path/to/myplugin.lazydeck',
      config = function() require('myplugin').setup() end,
    },

    -- GitHub 远程插件
    {
      'owner/remote-plugin.lazydeck',
      config = function() require('remote-plugin').setup() end,
    },

    -- 带版本约束
    {
      'owner/versioned-plugin.lazydeck',
      tag = '1.0.0',                       -- 指定 tag
      config = function() end,
    },
    {
      'owner/dev-plugin.lazydeck',
      branch = 'develop',                  -- 指定分支
      config = function() end,
    },
    {
      'owner/pinned-plugin.lazydeck',
      commit = 'abc1234567890',            -- 锁定到具体 commit
      config = function() end,
    },

    -- 需要多个插件时，直接平铺列在 plugins 里
    'owner/dep1.lazydeck',
    'owner/dep2.lazydeck',
    {
      'owner/my-plugin.lazydeck',
      config = function() require('my-plugin').setup() end,
    },
  },
}
```

`keymap` 用于覆盖内置主模式快捷键。支持的字段有 `up`、`down`、`top`、`bottom`、`preview_up`、`preview_down`、`reload`、`quit`、`force_quit`、`filter`、`clear_filter`、`back`、`open`、`enter`。每次调用 `deck.config` 都会按当前 `keymap` 配置重新执行一遍 `deck.keymap.set(...)`。

插件 spec 可配置 `keys` 字段注册全局快捷键。`keys` 是数组，元素格式为 `{ key, callback, desc = ... }`；按下快捷键时会先执行对应插件的 `setup/config`（即懒加载插件），再调用 `callback`。

**语法说明**：

- 字符串形式：`'owner/plugin.lazydeck'`
- 表形式：`{ 'owner/plugin.lazydeck' }`
- 本地目录形式：`{ dir = 'plugins/myplugin.lazydeck' }` 或 `{ 'myplugin', dir = '/abs/path/myplugin.lazydeck' }`
- 字符串中包含 `/` 时，始终按 GitHub 仓库处理；本地文件路径不再从字符串推断
- `dir` 只能是相对路径或绝对路径；相对路径基于配置目录解析
- Lua 会根据 `plugins` 配置动态把本地 `dir` 和远程插件安装目录加入 `package.path`
- 无 `config` 字段时，自动生成 `config = function() require('plugin').setup() end`
- 不再支持 `dependencies` 字段；需要的插件请直接平铺写在 `plugins` 数组里

````

### 插件管理器

启动 lazydeck 后，会进入插件管理器界面：

```bash
lazydeck                 # 进入插件管理器
lazydeck --help          # 显示帮助
lazydeck --version       # 显示版本
lazydeck /docker/container   # 启动后直接进入指定页面
````

在插件管理器界面中：

| 按键      | 功能                               |
| --------- | ---------------------------------- |
| `U`       | 更新所有插件到最新版本（遵循约束） |
| `S`       | 根据锁文件恢复所有插件到锁定版本   |
| `u`       | 更新当前选中的插件                 |
| `i`       | 安装当前选中的缺失插件             |
| `↓` / `↑` | 上/下选择插件                      |
| `Enter`   | 查看插件详情和更新状态             |

**插件约束说明**：

- `tag`：更新时只追踪指定 tag 的最新代码，不会更新到其他 tag
- `branch`：更新时只追踪指定分支的最新代码
- `commit`：锁定到具体 commit，无法更新

**数据目录**：

- 插件安装目录：`~/.local/share/lazydeck/plugins/`
- 锁文件：`~/.config/lazydeck/plugins.lock`

**远程插件认证**：

- 远程插件安装/更新使用非交互式 `git` 调用，不会在 TUI 内请求输入 GitHub 用户名或密码
- 私有仓库或需要认证的 HTTPS 仓库，请先在系统里配置好 Git 凭据，或改用 SSH / 本地 `dir` 插件
- 若凭据缺失，插件管理器会直接报错而不是在预览区显示 `Username for 'https://github.com'`

锁文件记录了每个插件安装时的具体 commit，下次可以通过 `S` 恢复。

## 文档

- [源码指南](src/README.md) - Rust 核心代码说明
- [插件系统](src/plugin/README.md) - Lua API 详细文档
- [预设脚本](preset/lua/README.md) - Lua 预设模块说明
- [UI 组件](src/widgets/README.md) - widgets 模块说明

## 依赖

### Rust 核心依赖

- **mlua** - LuaJIT 绑定
- **tokio** - 异步运行时
- **crossterm** - 终端控制
- **ratatui** - TUI 组件库
- **syntect** - 语法高亮
- **reqwest** - HTTP 客户端
- **chrono** - 时间处理

## 开发

配置和插件通过软链接访问：

```bash
config/    -> ~/.config/lazydeck/      # 用户配置
plugins/   -> ~/.local/share/lazydeck/plugins/  # 插件安装目录
```

修改配置或开发插件时，直接编辑这些目录下的文件。

### 日志

```bash
# 查看 Rust 日志
tail -f ~/.local/state/lazydeck/lazydeck.log

# 查看 Lua 日志
tail -f ~/.local/state/lazydeck/lua.log
```

## 贡献

欢迎提交 Issue 和 Pull Request！
