# Widget System

lazydeck 使用 ratatui 库构建终端 UI。widgets 模块提供了可渲染到终端的各种 UI 组件。

## 目录结构

```
src/widgets/
├── mod.rs           # 模块导出
├── renderable.rs    # Renderable trait 和有状态段落
├── text.rs          # Lua 文本类型封装
├── list.rs          # 列表组件
├── header.rs        # 头部组件
├── input.rs         # 输入框组件
├── confirm.rs       # 确认对话框
└── select.rs        # 选择对话框
```

## 核心概念

### Renderable Trait

所有可渲染组件实现的基础 trait：

```rust
pub trait Renderable {
    fn render(&mut self, area: Rect, buf: &mut Buffer);
    fn scroll_by(&mut self, offset: i16);
}
```

Lua 中的值通过 `FromLua` trait 转换为 `Box<dyn Renderable>`：

- 字符串 → `StatefulParagraph`
- `LuaText` (UserData) → `StatefulParagraph`
- `LuaImage` (UserData) → 图片块渲染
- 由 `string` / `Text` / `Image` 等组成的 Lua 数组 → 组合预览，按顺序渲染

## 组件说明

### StatefulParagraph

有状态的段落组件，支持滚动：

- 自动计算内容高度
- 垂直滚动
- 右侧显示滚动条

```rust
// 从字符串或 Text 创建
let paragraph = StatefulParagraph::from("Hello World");
let paragraph = StatefulParagraph::from(text);
```

### LuaImage

图片预览组件，支持：

- 从本地文件读取图片
- 支持 HTTP(S) URL，先显示占位文本，下载完成后自动回填
- 按预览区宽度等比缩放，未指定尺寸时默认读取 `deck.config().image`
- 优先使用终端原生图片协议（当前支持 Kitty / iTerm Inline）
- 终端不支持原生协议时，使用 truecolor 半块字符（`▀` / `▄`）渲染
- 若终端支持原生协议，但图片被滚动裁切，或当前有对话框导致 native render 被禁用，则保留布局占位但不渲染图片内容
- 可和文本一起放进数组做图文混排
- 终端协议编码结果会缓存在 `~/.cache/lazydeck/prepared-images/`，避免重复 decode/resize/encode

### ListWidget

列表组件，支持：

- **scrolloff** - 保持光标远离列表边缘（类似 vim 的 scrolloff）
- 自定义选中样式（使用 `` 和 `` 标记）
- 支持过滤后的列表渲染

### HeaderWidget

头部组件，显示：

- 当前插件名称（绿色）
- 当前路径（青色）
- 过滤输入（黄色）

### FooterWidget

底部组件，显示在底部：

- 左下角：当前悬停 entry 的 `bottom_line`（若存在，支持 `string` / `Span` / `Line`）
- 右下角：计数器，格式为 ` current/total `
- `` 和 `` 符号使用蓝色前景色
- 中间的计数文本使用蓝色背景和白色前景色

包含多个 Span 的行：

```rust
pub struct LuaLine(pub Line<'static>);

// 方法
line.fg(color)    // 设置前景色
line.bg(color)   // 设置背景色
line.bold()      // 加粗
line.italic()    // 斜体
line.underline() // 下划线
// 支持连接操作: line .. "text" 或 line .. span
```

### LuaSpan

单个文本片段（Span）：

```rust
pub struct LuaSpan(pub Span<'static>);

// 方法
span.fg(color)   // 设置前景色
span.bg(color)  // 设置背景色
span.bold()     // 加粗
span.italic()   // 斜体
span.underline() // 下划线
// 支持连接操作: span .. "text" 或 span .. span
```

### 颜色支持

支持的颜色名称：

- 标准色：`black`, `white`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`
- 调色板色：`DarkGray`, `Gray`, `LightRed`, `LightGreen`, `LightYellow`, `LightBlue`, `LightMagenta`, `LightCyan`
- 256 色：`Color::Indexed(n)`
- RGB：`Color::Rgb(r, g, b)`

### 使用示例

```lua
-- 创建带样式的文本
local span = deck.style.span("hello"):fg("green")
local line = deck.style.line({span, " world"})
local text = deck.style.text({line})

local emph = "repo":bold()
local note = deck.style.span("important"):underline()

-- 设置预览
deck.api.set_preview(nil, text)

-- 给已有 Text 追加一行
text:append(deck.style.line { "tail line" })

-- 给已有 Text 追加另一个 Text
text:append(deck.style.highlight("# title", "markdown"))

-- 字符串颜色扩展
local colored = "hello".fg("blue")
deck.api.set_preview(nil, colored)

-- 字符串 ANSI 解析
local ansi_text = "\x1b[31mred\x1b[0m":ansi()
deck.api.set_preview(nil, ansi_text)
```

## 输入状态管理

### InputState

输入框的状态管理：

```rust
pub struct InputState {
    pub text: String,
    pub cursor_position: usize,
    pub cursor_x: u16,
    pub cursor_y: u16,
}
```

提供的方法：

- `insert_char(c)` - 插入字符
- `backspace()` - 删除前一个字符
- `delete()` - 删除当前字符
- `clear()` - 清空输入
- `cursor_left()` / `cursor_right()` - 光标移动
- `cursor_to_start()` / `cursor_to_end()` - 光标跳转

## 对话框状态

### ConfirmDialog

确认对话框状态：

```rust
pub struct ConfirmDialog {
    pub title: Option<String>,
    pub prompt: String,
    pub selected_button: ConfirmButton,
    pub on_confirm: LuaFunction,
    pub on_cancel: Option<LuaFunction>,
}

pub enum ConfirmButton {
    Yes,
    No,
}
```

### SelectDialog

选择对话框状态：

```rust
pub struct SelectDialog {
    pub prompt: Option<String>,
    pub options: Vec<SelectOption>,
    pub selected_index: Option<usize>,
    pub filter_input: String,
    pub list_state: ListState,
    pub on_selection: LuaFunction,
}

pub struct SelectOption {
    pub value: LuaValue,
    pub display: Line<'static>,
}
```

## 实现细节

### 滚动处理

列表和选择对话框使用类似的滚动逻辑：

1. 计算 scrolloff 边界
2. 调整 offset 保持选中项在可视区域内
3. 支持键盘导航（上下箭头）

### Unicode 支持

使用 `unicode-width` 库正确处理：

- 光标位置计算
- 字符宽度计算
- 东亚字符（CJK）的正确显示

### 渲染流程

```
App::draw()
  ├── HeaderWidget.render()
  ├── FooterWidget.render()
  ├── ListWidget.render()
  ├── InputWidget.render() (filter mode)
  ├── ConfirmWidget.render() (confirm dialog)
  ├── SelectWidget.render() (select dialog)
  └── StatefulParagraph.render() (preview)
```

## 相关文件

- `src/app.rs` - 应用主窗口布局
- `src/state.rs` - 应用状态管理
- `src/plugin/deck/style.rs` - Rust 样式 API
- `preset/lua/style.lua` - Lua 样式封装
- `preset/lua/string.lua` - 字符串方法扩展
