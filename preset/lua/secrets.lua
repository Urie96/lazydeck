---@class deck.secrets
local secrets = {}

---Get a secret value.
---@param namespace string Secret namespace, typically plugin scoped
---@param key string Secret key
---@return string? value Secret value, or nil if not found
function secrets.get(namespace, key)
  return _deck.secrets.get(namespace, key)
end

---Store a secret value.
---@param namespace string Secret namespace, typically plugin scoped
---@param key string Secret key
---@param value string Secret value
function secrets.set(namespace, key, value)
  _deck.secrets.set(namespace, key, value)
end

---Delete a secret value.
---@param namespace string Secret namespace, typically plugin scoped
---@param key string Secret key
function secrets.delete(namespace, key)
  _deck.secrets.delete(namespace, key)
end

deck.secrets = secrets
