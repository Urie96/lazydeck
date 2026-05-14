---@class deck.style
local style = {}

---@class Text
---A TUI Text widget
---@field append fun(self: Text, line: Text|Line|Span|string) Append content to the text (modifies in place)

---@class Image
---@field __deck_type "image"
---@field source string Local image path or remote URL
---@field max_width? integer Optional width cap in terminal cells. Defaults to deck.config().image.max_width.
---@field max_height? integer Optional height cap in terminal rows/cells. Defaults to deck.config().image.max_height.
---A preview image widget. URLs are resolved asynchronously before rendering.

---@class Span
---A TUI Span widget
---@field fg fun(self: Span, color: string): Span Set foreground color (modifies in place and returns self)
---@field bg fun(self: Span, color: string): Span Set background color (modifies in place and returns self)
---@field bold fun(self: Span): Span Apply bold style (modifies in place and returns self)
---@field italic fun(self: Span): Span Apply italic style (modifies in place and returns self)
---@field underline fun(self: Span): Span Apply underline style (modifies in place and returns self)

---@class Line
---A TUI Line widget containing multiple Spans
---@field fg fun(self: Line, color: string): Line Set foreground color (modifies in place and returns self)
---@field bg fun(self: Line, color: string): Line Set background color (modifies in place and returns self)
---@field bold fun(self: Line): Line Apply bold style (modifies in place and returns self)
---@field italic fun(self: Line): Line Apply italic style (modifies in place and returns self)
---@field underline fun(self: Line): Line Apply underline style (modifies in place and returns self)

---Create a Span from a string
---@param s string The string into a Line
---@return Span
function style.span(args) return _deck.style.span(args) end

---Create a Line from a table of Spans or Strings
---@param args (Span|string)[] The Spans or Strings to combine into a Line
---@return Line A Line widget containing the combined Spans
function style.line(args) return _deck.style.line(args) end

---Create a Text from a table of Texts, Lines, Spans, or Strings
---@param args (Text|Line|Span|string)[] The Texts, Lines, Spans, or Strings to combine into a Text
---@return Text A Text widget containing the combined content
function style.text(args) return _deck.style.text(args) end

---Create an Image preview widget from a local file path
---@param path string The local image path or remote URL
---@param opts? {max_width?: integer, max_height?: integer} Optional size caps in terminal cells
---@return Image
function style.image(path, opts)
  local image_cfg = (deck.config and deck.config.get and deck.config.get().image) or {}
  return {
    __deck_type = 'image',
    source = path,
    max_width = (opts and opts.max_width) or image_cfg.max_width,
    max_height = (opts and opts.max_height) or image_cfg.max_height,
  }
end

---Highlight code with syntax highlighting
---@param code string The code to highlight
---@param language string The programming language name (e.g., "javascript", "python", "rust", "lua")
---@return Text A Text widget with syntax-highlighted code
function style.highlight(code, language) return _deck.style.highlight(code, language) end

---Align columns in a 1D array of Lines, modifying them in place
---@param lines Line[] A 1D array of Lines, where each Line contains multiple Spans representing columns
function style.align_columns(lines) return _deck.style.align_columns(lines) end

deck.style = style
