---@class CacheOptions
---@field ttl number? Time-to-live in seconds (optional)

---@class deck.cache
local cache = {}

---Get a value from cache
---@param namespace string The cache namespace
---@param key string The cache key
---@return any value The cached value, or nil if not found or expired
function cache.get(namespace, key) return _deck.cache.get(namespace, key) end

---Set a value in cache
---@param namespace string The cache namespace
---@param key string The cache key
---@param value any The value to cache (nil, boolean, number, string, table, array)
---@param opts CacheOptions? Optional options (e.g., {ttl = 3600} for 1 hour TTL)
function cache.set(namespace, key, value, opts) return _deck.cache.set(namespace, key, value, opts) end

---Delete a value from cache
---@param namespace string The cache namespace
---@param key string The cache key to delete
function cache.delete(namespace, key) return _deck.cache.delete(namespace, key) end

---Clear all cached values in a namespace
---@param namespace string The cache namespace
function cache.clear(namespace) return _deck.cache.clear(namespace) end

deck.cache = cache
