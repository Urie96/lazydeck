---@class deck.url
local url = {}

---Percent-encode a string for safe inclusion in URL components.
---@param value string
---@return string encoded
function url.encode(value)
  return _deck.url.encode(value)
end

---Decode a percent-encoded string.
---@param value string
---@return string decoded
function url.decode(value)
  return _deck.url.decode(value)
end

deck.url = url
