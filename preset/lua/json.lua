--
-- json.lua
--
-- JSON encoding and decoding via deck.json.decode() and deck.json.encode()
-- Implemented in Rust (serde_json)
--

---@class deck.json
local json = {}

---@class JsonEncodeOptions
---@field indent number? Number of spaces for indentation (e.g., 2)

---Decode a JSON string to a Lua value
---@param str string The JSON string to decode
---@return any lua_value The decoded Lua value
function json.decode(str)
  return _deck.json.decode(str)
end

---Encode a Lua value to a JSON string
---@param val any The Lua value to encode (nil, boolean, number, string, table, array)
---@param opt JsonEncodeOptions? Optional settings
---@param opt.indent number? Number of spaces for indentation (e.g., 2)
---@return string json_string The JSON encoded string
function json.encode(val, opt)
  return _deck.json.encode(val, opt)
end

deck.json = json
