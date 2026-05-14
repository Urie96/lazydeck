table.unpack = table.unpack or unpack
unpack = unpack or table.unpack

---Copy text to system clipboard using OSC 52 escape sequence
---@param text string The text to copy
function deck.osc52_copy(text) return _deck.osc52_copy(text) end
