---@class SelectOption
---@field value any The value to return when this option is selected
---@field display string|Span|Line|Text The text to display for this option (supports styled widgets)

---@class SelectOptions
---@field prompt? string Optional prompt/title text (defaults to "Select")
---@field options (string|SelectOption)[] The list of options to display

---Show a selection dialog to the user
---The dialog appears centered on screen with a list of options
---Users can navigate with arrow keys (or j/k), type to filter, Enter to select, Esc to cancel
---@param opts SelectOptions Configuration options
---  Can be simple strings: {"Option 1", "Option 2", "Option 3"}
---  Or tables with value/display: {{value = "py", display = "🐍 Python"}, {value = "js", display = "📜 JavaScript"}}
---@param on_selection fun(choice: any) Callback function when user makes a selection
---  - Called with the selected value (the value field from options, or the string itself)
---  - Called with nil if user cancels (Esc)
function deck.select(opts, on_selection) return _deck.select(opts, on_selection) end

---@class ConfirmOptions
---@field title? string Optional title text (defaults to "Confirm")
---@field prompt string The confirmation message to display
---@field on_confirm fun() Callback function when user confirms (Yes)
---@field on_cancel? fun() Callback function when user cancels (No)

---Show a confirmation dialog to the user
---The dialog appears centered on screen with Yes/No buttons
---Users can use Left/Right arrows to select buttons, Enter to confirm selection
---Or use Y/N keys to directly confirm or cancel
---@param opts ConfirmOptions Configuration options
function deck.confirm(opts) return _deck.confirm(opts) end

---Display a notification in bottom-right corner
---@param message string|Span|Line|Text The notification message (supports styled Text, Line, or Span)
function deck.notify(message) return _deck.notify(message) end

---@class InputOptions
---@field prompt? string Optional prompt text to display before input (defaults to "")
---@field placeholder? string Optional placeholder text shown when input is empty (defaults to "")
---@field value? string Optional initial input text (defaults to "")
---@field on_submit fun(input: string) Callback function when user presses Enter
---  - Called with the user's input text
---@field on_cancel? fun() Callback function when user presses Escape
---@field on_change? fun(input: string) Callback function when input text changes
---  - Called with the current input text on each keystroke

---Input dialog helpers.
---Can be called directly as `deck.input(opts)` to show a dialog.
---@class deck.input
deck.input = {}

---Show an input dialog to the user
---The dialog appears centered on screen with an input field
---Users can type text, use Backspace/Delete to edit, Left/Right to move cursor
---Press Enter to submit, Escape to cancel
---@param opts InputOptions Configuration options
function deck.input.show(opts) return _deck.input.show(opts) end

---Get current input dialog text.
---@return string|nil
function deck.input.get() return _deck.input.get() end

---Replace current input dialog text.
---@param value string
function deck.input.set(value) return _deck.input.set(value) end

setmetatable(deck.input, {
  __call = function(_, opts) return deck.input.show(opts) end,
})

---Write a log entry to the log file
---@param level string Log level (e.g., "info", "warn", "error", "debug")
---@param format string Format string with {} placeholders
---@vararg any Arguments to format into the message
function deck.log(level, format, ...) return _deck.log(level, format, ...) end
