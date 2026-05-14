---@class HttpResponse
---@field success boolean Whether the request succeeded
---@field status number HTTP status code
---@field body string Response body
---@field headers table<string, string> Response headers
---@field error string|nil Error message if failed

---@class RequestOptions
---@field url string Request URL
---@field method string HTTP method (GET/POST/PUT/DELETE/PATCH)
---@field headers table<string, string>? Request headers
---@field body string? Request body
---@field timeout number? Timeout in milliseconds (default: 30000)

---@class deck.http
local http = {}

---Send a GET request
---@param url string The request URL
---@param callback fun(response: HttpResponse) Callback function
function http.get(url, callback) return _deck.http.get(url, callback) end

---Send a POST request
---@param url string The request URL
---@param body string Request body
---@param callback fun(response: HttpResponse) Callback function
function http.post(url, body, callback) return _deck.http.post(url, body, callback) end

---Send a PUT request
---@param url string The request URL
---@param body string Request body
---@param callback fun(response: HttpResponse) Callback function
function http.put(url, body, callback) return _deck.http.put(url, body, callback) end

---Send a DELETE request
---@param url string The request URL
---@param callback fun(response: HttpResponse) Callback function
function http.delete(url, callback) return _deck.http.delete(url, callback) end

---Send a PATCH request
---@param url string The request URL
---@param body string Request body
---@param callback fun(response: HttpResponse) Callback function
function http.patch(url, body, callback) return _deck.http.patch(url, body, callback) end

---Send a custom HTTP request with full options
---@param opts RequestOptions The request options
---@param callback fun(response: HttpResponse) Callback function
function http.request(opts, callback) return _deck.http.request(opts, callback) end

deck.http = http
