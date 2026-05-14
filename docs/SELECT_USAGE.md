# deck.select() 使用说明

`deck.select()` 是一个弹出式选择对话框组件，类似于 Neovim 的 `vim.ui.select()`。

## 功能特性

- 居中显示在屏幕上
- 支持上下箭头键（或 j/k）导航选项
- 支持实时过滤：输入文本即可筛选选项（不区分大小写）
- 回车确认选择
- ESC 取消选择（回调函数接收 nil）
- 每个选项占据单独一行
- 支持长列表滚动

## API

```lua
deck.select({
  prompt = "选择提示文本",  -- 可选，默认为 "Select"
  options = {...},          -- 必需，选项列表
}, function(choice)
  -- 处理选择结果
end)
```

## 选项格式

### 1. 简单字符串数组

```lua
deck.select({
  prompt = '请选择一个选项:',
  options = { '选项1', '选项2', '选项3' },
}, function(choice)
  if choice then
    print('选择了: ' .. choice)
  else
    print '取消了选择'
  end
end)
```

在这种格式中，`value` 和 `display` 都是同一个字符串。

### 2. 带有 value 和 display 的表数组

```lua
deck.select({
  prompt = '选择一种编程语言:',
  options = {
    { value = "py", display = "🐍 Python" },
    { value = "js", display = "📜 JavaScript" },
    { value = "lua", display = "🌙 Lua" },
    { value = "rs", display = "🦀 Rust" },
  },
}, function(choice)
  if choice then
    print('选择了: ' .. choice)
  else
    print '取消了选择'
  end
end)
```

在这种格式中：
- `value` 是返回给回调函数的实际值
- `display` 是在对话框中显示的文本

## 键盘快捷键

| 按键 | 功能 |
|------|------|
| ↑ / k | 向上移动 |
| ↓ / j | 向下移动 |
| Page Up | 向上翻页 |
| Page Down | 向下翻页 |
| Home | 跳到第一个选项 |
| End | 跳到最后一个选项 |
| Enter | 确认选择 |
| Esc | 取消选择 |
| Backspace | 删除过滤字符 |
| 任意字符 | 添加到过滤条件 |

## 实现细节

### Rust 结构

#### SelectOption
```rust
pub struct SelectOption {
    pub value: LuaValue,   // 返回给回调的值
    pub display: String,   // 显示的文本
}
```

#### SelectDialog
```rust
pub struct SelectDialog {
    pub prompt: Option<String>,              // 提示文本
    pub options: Vec<SelectOption>,           // 所有选项
    pub filtered_options: Vec<usize>,         // 过滤后的选项索引
    pub selected_index: Option<usize>,        // 当前选中索引
    pub filter_input: String,                 // 过滤输入文本
    pub list_state: ListState,                // 列表状态（用于滚动）
    pub on_selection: LuaFunction,           // 选择回调
}
```

### 事件流程

1. Lua 调用 `deck.select(opts)` 触发 `Event::ShowSelect`
2. Rust 接收事件，创建 `SelectDialog` 并存储在 `State::select_dialog`
3. `AppWidget::render()` 检测到 `select_dialog` 存在，渲染选择对话框
4. 键盘事件由 `select_handler::handle_select_dialog_key()` 处理
5. 用户确认或取消后，调用 `on_selection` 回调并清除 `select_dialog`

### 过滤逻辑

过滤是不区分大小写的子串匹配：

```rust
let filter_lower = self.filter_input.to_lowercase();
self.filtered_options = self.options
    .iter()
    .enumerate()
    .filter(|(_, opt)| opt.display.to_lowercase().contains(&filter_lower))
    .map(|(idx, _)| idx)
    .collect();
```

## 测试插件

项目包含一个测试插件 `select-test`，可以通过以下方式运行：

```bash
cargo run -- select-test
```

测试插件提供三个测试：
- 按 `s`: 测试简单字符串选项
- 按 `p`: 测试带图标和 value 的选项
- 按 `l`: 测试长列表（50项）的筛选功能

## 类型定义

完整的类型定义在 `preset/types.lua` 中：

```lua
---Show a selection dialog to the user
---@param opts table Configuration options
---@field prompt? string Optional prompt/title text (defaults to "Select")
---@field options (string|SelectOption)[] The list of options to display
---@param on_selection fun(choice: any) Callback function when user makes a selection
function deck.select(opts, on_selection) end
```
