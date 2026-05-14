--
-- yaml.lua
--
-- YAML encoding and decoding via deck.yaml.decode() and deck.yaml.encode()
-- Implemented in Rust (serde_yaml)
--

---@class deck.yaml
local yaml = {}

---Decode a YAML string to a Lua value
---@param str string The YAML string to decode
---@return any lua_value The decoded Lua value
function yaml.decode(str)
  return _deck.yaml.decode(str)
end

---Encode a Lua value to a YAML string
---@param val any The Lua value to encode (nil, boolean, number, string, table, array)
---@return string yaml_string The YAML encoded string
function yaml.encode(val)
  return _deck.yaml.encode(val)
end

deck.yaml = yaml
