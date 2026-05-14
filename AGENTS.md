# lazydeck 开发指南

## 首次消息

如果用户没有在第一条消息中给出具体任务，先阅读以下 README 文件，然后询问用户要处理哪个模块：

- [README.md](README.md) - 项目整体介绍
- [src/README.md](src/README.md) - Rust 核心代码说明
- [src/plugin/README.md](src/plugin/README.md) - Lua 插件系统详解
- [src/widgets/README.md](src/widgets/README.md) - UI 组件说明
- [preset/lua/README.md](preset/lua/README.md) - Lua 预设脚本说明

根据用户回答，阅读对应的 README.md 文件来了解相关模块。

## 项目结构

```
lazydeck/
├── src/                    # Rust 源代码
│   ├── main.rs            # 入口点
│   ├── app.rs             # 主应用逻辑和 UI 渲染
│   ├── state.rs           # 应用状态管理
│   ├── events.rs          # 事件系统
│   ├── keymap.rs          # 键盘映射解析
│   ├── page.rs            # 页面和条目管理
│   ├── input_handler.rs   # 输入模式键盘处理
│   ├── confirm_handler.rs # 确认对话框处理
│   ├── select_handler.rs  # 选择对话框处理
│   ├── plugin/            # Lua 插件系统
│   │   ├── lua.rs        # Lua 初始化和预设加载
│   │   ├── scope.rs      # 作用域管理
│   │   └── deck/           # Lua API 实现
│   │       ├── api.rs    # 页面管理 API
│   │       ├── cache.rs  # 缓存系统
│   │       ├── fs.rs     # 文件系统操作
│   │       ├── http.rs   # HTTP 客户端
│   │       ├── style.rs  # UI 样式和语法高亮
│   │       ├── system.rs # 系统命令执行
│   │       └── time.rs   # 时间处理
│   └── widgets/           # UI 组件
│       ├── renderable.rs # Renderable trait
│       ├── text.rs       # 文本类型封装
│       ├── list.rs       # 列表组件
│       ├── header.rs     # 头部组件
│       ├── input.rs      # 输入框组件
│       ├── confirm.rs    # 确认对话框
│       └── select.rs     # 选择对话框
├── preset/                # 预设文件
│   ├── lua/              # Lua 预设脚本
│   ├── syntaxes/         # 语法高亮定义
│   └── themes/           # 颜色主题
├── config -> ~/.config/lazydeck/        # 用户配置（软链接）
└── plugins -> ~/.local/share/lazydeck/plugins/  # 插件目录（软链接）
```

> 项目中的 `./plugins` 和 `./config` 目录具有写权限，可以直接调用工具（write、edit）创建或修改文件，无需请求提权。

## 任务处理

首次对话建立基本认知后，根据用户任务类型采取不同处理方式：

### 添加新功能

1. 阅读相关模块的 README 文件（如添加 LC API 函数读 `src/plugin/README.md`，添加 UI 组件读 `src/widgets/README.md`）
2. 阅读相关源代码文件（见"项目结构"部分）
3. 阅读参考文件：
   - 添加 LC API 函数：阅读 `src/plugin/deck/mod.rs` 了解注册方式，阅读现有 API 实现
   - 添加内部命令：阅读 `src/app.rs::handle_command()` 了解命令处理方式
   - 创建插件：参考 `preset/lua/` 下的模块和 `plugins/` 下的现有插件
4. 修改代码
5. 同步更新 `preset/lua/` 中的相关文件
6. 更新相关 README 文档

### 修复 Bug

1. 根据问题描述，阅读相关模块的 README 和源代码
2. 定位问题位置
3. 修复代码
4. 运行相关测试：`cargo test`
5. 如有 TUI 相关修改，使用 tmux 测试（见"使用 tmux 测试 TUI"部分）

### 代码重构/优化

1. 阅读涉及模块的 README 和源代码
2. 理解现有设计
3. 进行重构
4. 运行测试确保功能正常

### 文档更新

1. 阅读要更新的 README 文件
2. 根据代码变更同步更新
3. 检查交叉引用是否准确

**重要原则**：

- 遇到不熟悉的模块，先读该模块的 README 再读源码
- 不要删除看起来是故意添加的代码或功能
- 修改后同步更新相关文档

## 常用命令

使用 `just` 或 `cargo` 进行开发：

```bash
cargo run                   # 运行 lazydeck
cargo build                 # Debug 构建
cargo run --release        # Release 构建
cargo test                 # 运行测试
cargo test <test_name>     # 运行单个测试
```

### 运行插件

```bash
cargo run                   # 启动 lazydeck，进入插件管理器

# 构建后的二进制用法：
lazydeck                     # 启动 lazydeck，进入插件管理器
```

## 代码质量

- 修改或添加新的 LC API 函数后，必须同步更新 `preset/lua/*.lua` 文件
- 不要删除看起来是故意添加的代码或功能

## 测试

### Rust 单元测试

各模块内置单元测试：

- `src/keymap.rs` - 键盘映射解析测试
- `src/plugin/deck/highlighter.rs` - 语法高亮测试
- `src/plugin/deck/style.rs` - 样式对齐测试
- `src/plugin/deck/time.rs` - 时间解析测试

## 日志

```bash
# 查看 Rust 日志
tail -f ~/.local/state/lazydeck/lazydeck.log

# 查看 Lua 日志
tail -f ~/.local/state/lazydeck/lua.log
```

## 使用 tmux 测试 TUI

在受控终端环境中测试 lazydeck 的 TUI：

```bash
# 创建指定尺寸的 tmux 会话
tmux new-session -d -s lazydeck-test -x 80 -y 24

# 从源码启动 lazydeck
tmux send-keys -t lazydeck-test "cargo run" Enter

# 等待启动，然后捕获输出
sleep 1 && tmux capture-pane -t lazydeck-test -p

# 发送输入
tmux send-keys -t lazydeck-test "your input" Enter

# 发送特殊键
tmux send-keys -t lazydeck-test Escape
tmux send-keys -t lazydeck-test C-c     # ctrl+c
tmux send-keys -t lazydeck-test C-r     # ctrl+r
tmux send-keys -t lazydeck-test "/"     # 进入过滤模式

# 发送方向键
tmux send-keys -t lazydeck-test Up
tmux send-keys -t lazydeck-test Down

# 清理
tmux kill-session -t lazydeck-test
```

## 添加新功能

### 添加新的 LC API 函数

1. 在适当的 `src/plugin/deck/*.rs` 文件中添加函数
2. 在 `src/plugin/deck/mod.rs` 的 `register()` 中注册
3. 如需状态访问，使用 `plugin::borrow_scope_state()` 或 `plugin::mut_scope_state()`
4. 如需触发更新，调用 `plugin::send_render_event()`
5. 在 `preset/lua/` 中对应的封装文件中添加 Lua 封装和类型注解

### 添加新的内部命令

1. 在 `src/app.rs::handle_command()` 的 match 分支中添加命令
2. 实现命令逻辑
3. 如改变 UI，设置 `self.dirty = true`

### 创建新插件

1. 创建 `plugins/myplugin.lazydeck/myplugin/init.lua`（或在 `config/` 对应目录）
2. 在 `config/init.lua` 中添加插件配置
3. 运行 `cargo run` 后进入 `/myplugin`

## 文档更新

更新代码后，确保同步更新相关 README：

- `src/README.md` - 核心架构变更
- `src/plugin/README.md` - API 变更
- `src/widgets/README.md` - 组件变更
- `preset/lua/README.md` - 预设脚本变更

## 关键文件

- `AGENTS.md` - 本文件
- `README.md` - 项目整体介绍
- `src/README.md` - Rust 核心代码
- `src/plugin/README.md` - Lua 插件系统
- `src/widgets/README.md` - UI 组件
- `preset/lua/README.md` - Lua 预设脚本
