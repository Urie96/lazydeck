---@alias Mode "main"|"input"

---@class deck.keymap
local keymap = {}

---@class KeymapOptions
---@field desc? string Human readable description for help panels
---@field once? boolean Remove this global keymap after it is triggered once

---@class deck.ConfigKeymap
---@field up? string
---@field down? string
---@field top? string
---@field bottom? string
---@field preview_up? string
---@field preview_down? string
---@field reload? string
---@field quit? string
---@field force_quit? string
---@field filter? string
---@field clear_filter? string
---@field back? string
---@field open? string
---@field enter? string
---@field input_submit? string
---@field input_cancel? string
---@field input_clear_before_cursor? string
---@field input_cursor_to_start? string
---@field input_cursor_to_end? string

---Set a key mapping for a specific mode
---@param mode Mode The mode (e.g., "main", "input")
---@param key string The key sequence (e.g., "ctrl-d", "down", "j")
---@param callback string|fun() The command string or callback function
---@param opt? KeymapOptions Optional keymap metadata
function keymap.set(mode, key, callback, opt) return _deck.keymap.set(mode, key, callback, opt) end

---@alias EntryKeymap table<string, fun()>

deck.keymap = keymap
