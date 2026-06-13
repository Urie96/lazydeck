# lazydeck 源码指南

lazydeck 是一个基于 Rust 和 Lua 的终端 UI 应用，采用事件驱动架构。本文档介绍核心源代码结构。

## 目录结构

```
src/
├── main.rs           # 应用入口点
├── app.rs            # 主应用逻辑和 UI 渲染
├── state.rs          # 应用状态管理
├── events.rs         # 事件系统
├── keymap.rs         # 键盘映射解析
├── page.rs           # 页面和条目管理
├── mode.rs           # 模式定义
├── input_handler.rs  # 输入模式键盘处理
├── confirm_handler.rs # 确认对话框处理
├── select_handler.rs  # 选择对话框处理
├── term.rs           # 终端初始化和恢复
├── log.rs            # 日志系统
├── errors.rs         # 错误处理和 panic 钩子
├── plugin/           # Lua 插件系统
│   ├── mod.rs
│   ├── lua.rs        # Lua 初始化
│   ├── scope.rs      # 作用域管理
│   └── deck/           # Lua API 实现
└── widgets/          # UI 组件
    ├── mod.rs
    ├── renderable.rs # 可渲染 trait
    ├── text.rs       # 文本类型封装
    ├── list.rs       # 列表组件
    ├── header.rs     # 头部组件
    ├── input.rs      # 输入框组件
    ├── confirm.rs    # 确认对话框
    └── select.rs     # 选择对话框
```

## 核心架构

### 运行时

- **单线程异步运行时**：`tokio::main(flavor = "current_thread")`
- **任务集**：主要逻辑在 `LocalSet` 中运行

### 事件驱动系统

多流事件系统（`events.rs`）：

```
┌───────────────────────────────────────────────┐
│                 Events                        │
├───────────────────────────────────────────────┤
│  渲染流             │  Crossterm 流 (终端输入)│
│                     │  键盘、鼠标、调整大小   │
│                     ├─────────────────────────┤
│                     │  文本流 (MPSC 通道)     │
│                     │  内部应用事件           │
└───────────────────────────────────────────────┘
                     │
                     ▼
              StreamMap 处理
                     │
                     ▼
              Event::xxx 枚举
```

### 事件类型

| 事件                 | 说明           |
| -------------------- | -------------- |
| `Quit`               | 退出应用       |
| `Render`             | 触发重绘       |
| `Enter(path)`        | 进入目录       |
| `Command(cmd)`       | 执行内部命令   |
| `Crossterm(e)`       | 终端输入事件   |
| `AddKeymap(km)`      | 添加键盘映射   |
| `LuaCallback(f)`     | Lua 回调       |
| `InteractiveCommand` | 执行交互式命令 |
| `Notify(msg)`        | 显示通知       |
| `ExpireNotification(id)` | 移除到期通知 |
| `ShowConfirm`        | 显示确认对话框 |
| `ShowSelect`         | 显示选择对话框 |

## 应用流程

```
main.rs
    │
    ▼
Logs::start()           # 初始化日志
errors::install_hooks()  # 安装 panic 钩子
term::init()            # 初始化终端
parse_cli_options()     # 解析命令行参数（--help / --version / --config / --eval / 可选初始路径）
    │
    ▼
App::new(initial_path, eval_scripts)
    │
    ├─ 创建 State
    ├─ 初始化 Lua
    └─ 加载预设脚本
    │
    ▼
App::run(events)
    │
    ▼
事件循环
    │
    ├─ handle_event()             # 处理事件
    ├─ handle_command()           # 执行命令
    ├─ call_list()                # 调用 Lua _list()
    ├─ run_startup_eval_scripts() # 执行 --eval Lua 片段
    ├─ call_preview()             # 调用 Lua _preview()
    └─ draw()                     # 渲染 UI
```

路径处理约定：

- `State.current_path` 保存解码后的原始 path segment
- Lua 插件通过 `deck.api.go_to({ ... })` / `deck.api.get_current_path()` 读写的也都是原始 segment
- 当路径以字符串形式出现时（例如 header、命令行初始路径、`cd` 命令），Rust 侧会自动做 percent 编解码，避免插件手动处理

## 状态管理

`state.rs` 中的 `State` 结构体：

```rust
pub struct State {
    pub current_mode: Mode,              // 当前模式 (Main/Input)
    pub current_path: Vec<String>,        // 导航路径栈
    pub current_page: Option<Page>,       // 当前页面条目
    pub keymap_config: Vec<Keymap>,       // 键盘映射配置
    pub last_key_event_buffer: Vec<KeyEvent>,  // 按键序列缓冲区
    pub current_preview: Option<Box<dyn Renderable>>,  // 预览内容
    pub notifications: Vec<NotificationItem>,  // 通知消息队列
    pub filter_input: String,            // 过滤输入
    pub page_cache: HashMap<Vec<String>, Page>,  // 页面缓存
    pub confirm_dialog: Option<ConfirmDialog>,   // 确认对话框
    pub select_dialog: Option<SelectDialog>,     // 选择对话框
    // ... 更多字段
}
```

### 页面缓存

实现类似 vim 的目录缓存机制：

- 导航时保存当前页面到缓存
- 返回时从缓存恢复，保持选中位置和过滤状态

### 键盘映射匹配

使用前缀匹配缓冲区处理多键序列（如 `gg`, `dd`, `<C-x><C-c>`）：

1. 键盘事件累积在 `last_key_event_buffer`
2. 与注册的键盘映射匹配
3. 完全匹配时执行回调

## 模式系统

```rust
pub enum Mode {
    Main,   // 主模式 - 导航和操作
    Input,  // 输入模式 - 输入框 keymap / 编辑
}
```

## UI 渲染

`app.rs` 中的 `AppWidget` 实现了 `StatefulWidget`：

### 浮动组件

按渲染顺序（后者覆盖前者）：

1. Header
2. List + Preview
3. Input (过滤模式)
4. Notification
5. Confirm Dialog
6. Select Dialog

## 对话框

### 确认对话框

- 蓝色圆角边框
- Yes/No 按钮（选中时反色）
- 支持键盘导航：`←`/`→` 切换，`Enter` 确认，`Y`/`N` 快捷键

### 选择对话框

- 青色圆角边框
- 过滤输入框
- 选项列表（支持滚动和过滤）
- 支持 Unicode 字符正确显示

## Lua 集成

### 初始化流程

```rust
// app.rs
let lua = Lua::new();
plugin::scope(&lua, &mut state, &sender, || {
    plugin::init_lua(&lua)
})?;
```

### 作用域模式

```rust
plugin::scope(&lua, &mut state, &sender, || {
    // Lua 代码可访问：
    // - deck 全局表（API）
    // - state（通过注册表）
    // - sender（事件发送）
})?;
```

### Lua 回调

外部命令执行完成后，通过 `Event::LuaCallback` 回调：

```rust
Event::LuaCallback(Box::new(move |lua| {
    callback.call(response)
}))
```

## 内部命令

通过 `deck.cmd()` 或 `handle_command()` 执行：

| 命令                    | 说明          |
| ----------------------- | ------------- |
| `quit`                  | 退出应用      |
| `scroll_by [n]`         | 滚动列表 n 行 |
| `scroll_preview_by [n]` | 滚动预览 n 行 |
| `reload`                | 刷新当前列表  |
| `history_back`          | 跳回上一个访问页面 |
| `history_forward`       | 跳到下一个历史页面 |
| `tab_new [path]`        | 新建标签页，可选指定路径 |
| `tab_close`             | 关闭当前标签页 |
| `tab_next`              | 切换到下一个标签页 |
| `tab_prev`              | 切换到上一个标签页 |
| `tab_switch <index>`    | 切换到指定标签页（从 1 开始） |
| `command_prompt`        | 打开命令输入框并执行输入的内部命令 |
| `cd <path>`             | 跳转到指定页面路径，支持绝对/相对路径 |
| `enter_filter_mode`     | 进入过滤模式  |
| `exit_filter_mode`      | 退出过滤模式  |
| `accept_filter`         | 应用过滤      |
| `filter_clear`          | 清除过滤      |
| `filter_backspace`      | 删除过滤字符  |
| `enter`                 | 进入选中目录  |
| `back`                  | 返回上级目录  |

## 日志系统

- 通知不再依赖高频 Render 轮询过期，而是在创建时注册一次性延迟回调，到期后发送事件移除。
- **Rust 日志**：`~/.local/state/lazydeck/lazydeck.log`
- 使用 `tracing` 库
- 非阻塞写入

### 查看日志

```bash
tail -f ~/.local/state/lazydeck/lazydeck.log
```

## 错误处理

- 使用 `anyhow::Result<T>` 进行错误传播
- Panic 时恢复终端状态

## 相关文档

- [plugin/README.md](plugin/README.md) - Lua 插件系统
- [widgets/README.md](widgets/README.md) - UI 组件
- [preset/lua/README.md](../preset/lua/README.md) - 预设 Lua 脚本
