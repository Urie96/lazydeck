---@alias Mode "main"|"input"

---@class deck.keymap
local keymap = {}

---@class KeymapOptions
---@field desc? string Human readable description for help panels
---@field path? 0|string|string[] Page path pattern for a page-scoped keymap; 0 means the current page path, string will be split by '/', "*" matches one segment, "**" matches zero or more segments

---@class PluginKeySpec
---@field [1] string Key sequence
---@field [2] fun() Callback to run after the plugin has been loaded
---@field desc? string Human readable description for help panels

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

local function normalize_keymap_path(path)
  if path == nil or path == 0 then return path end
  if type(path) == 'string' then
    local parts = {}
    for segment in path:gmatch('[^/]+') do
      parts[#parts + 1] = segment
    end
    return parts
  end
  if type(path) == 'table' then
    return path
  end
  error('keymap path must be 0, string, or table')
end

---Set a key mapping for a specific mode
---@param mode Mode The mode (e.g., "main", "input")
---@param key string The key sequence (e.g., "ctrl-d", "down", "j")
---@param callback string|fun() The command string or callback function
---@param opt? KeymapOptions Optional keymap metadata
function keymap.set(mode, key, callback, opt)
  if opt ~= nil then
    local next_opt = {}
    for k, v in pairs(opt) do next_opt[k] = v end
    next_opt.path = normalize_keymap_path(next_opt.path)
    opt = next_opt
  end
  return _deck.keymap.set(mode, key, callback, opt)
end

---@alias EntryKeymap table<string, fun()>

deck.keymap = keymap
