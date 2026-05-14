---@class HttpServerRequest
---@field method string
---@field path string
---@field query table<string, string>
---@field params table<string, string>
---@field headers table<string, string>

---@class HttpServerResponse
---@field status number?
---@field headers table<string, string>?
---@field body string?

---@class HttpServerInfo
---@field host string
---@field port number
---@field base_url string

---@class deck.http_server
local http_server = {}

---Register a local HTTP resolver.
---@param name string
---@param handler fun(request: HttpServerRequest, respond: fun(response: HttpServerResponse))
function http_server.register_resolver(name, handler) return _deck.http_server.register_resolver(name, handler) end

---Unregister a local HTTP resolver.
---@param name string
function http_server.unregister_resolver(name) return _deck.http_server.unregister_resolver(name) end

---Build a localhost URL for a registered resolver.
---@param name string
---@param params table<string, string|number|boolean>|nil
---@return string
function http_server.url(name, params) return _deck.http_server.url(name, params) end

---Get local HTTP server info.
---@return HttpServerInfo
function http_server.info() return _deck.http_server.info() end

deck.http_server = http_server
