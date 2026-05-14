---@class LuaSocket
local LuaSocket = {}

---@param callback fun(line: string)
function LuaSocket:on_line(callback) return self:_on_line(callback) end

---@param message string
function LuaSocket:write(message) return self:_write(message) end

function LuaSocket:close() return self:_close() end

---@class deck.socket
local socket = {}

---Connect to a socket endpoint.
---@param addr string Endpoint like `unix:$TMPDIR/test.sock`
---@return LuaSocket
function socket.connect(addr)
  local sock = _deck.socket.connect(addr)

  return setmetatable({
    _raw = sock,
  }, {
    __index = function(self, key)
      if key == '_on_line' then
        return function(_, callback) return self._raw:on_line(callback) end
      end
      if key == '_write' then
        return function(_, message) return self._raw:write(message) end
      end
      if key == '_close' then
        return function() return self._raw:close() end
      end
      return LuaSocket[key]
    end,
  })
end

deck.socket = socket
