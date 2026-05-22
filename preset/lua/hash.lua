---@class deck.hash
local hash = {}

---Return the lowercase hexadecimal MD5 digest of a string's bytes.
---@param data string The data to hash
---@return string digest 32-character lowercase hexadecimal MD5 digest
function hash.md5(data) return _deck.hash.md5(data) end

deck.hash = hash
