--
-- clipboard.lua
--
-- System clipboard access via deck.clipboard.get() and deck.clipboard.set()
-- Implemented in Rust (arboard crate)
--

---@class deck.clipboard
local clipboard = {}

---Get the current clipboard content
---@return string content The clipboard text content
function clipboard.get()
  return _deck.clipboard.get()
end

---Set the clipboard content
---@param text string The text to copy to the clipboard
function clipboard.set(text)
  _deck.clipboard.set(text)
end

deck.clipboard = clipboard
