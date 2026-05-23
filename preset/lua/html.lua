---@class HtmlDocument
local HtmlDocument = {}
local wrap_node
local wrap_list

---@param selector string
---@return HtmlNodeList
function HtmlDocument:select(selector) return wrap_list(self._raw:select(selector)) end

---@param selector string
---@return HtmlNode|nil
function HtmlDocument:first(selector) return wrap_node(self._raw:first(selector)) end

---@return string
function HtmlDocument:html() return self._raw:html() end

---@return string
function HtmlDocument:to_markdown() return self._raw:to_markdown() end

---@class HtmlNode
local HtmlNode = {}

---@return string
function HtmlNode:name() return self._raw:name() end

---@return string
function HtmlNode:html() return self._raw:html() end

---@return string
function HtmlNode:inner_html() return self._raw:inner_html() end

---@return string
function HtmlNode:text() return self._raw:text() end

---@return string
function HtmlNode:to_markdown() return self._raw:to_markdown() end

---@param name string
---@return string|nil
function HtmlNode:attr(name) return self._raw:attr(name) end

---@return table<string, string>
function HtmlNode:attrs() return self._raw:attrs() end

---@param selector string
---@return HtmlNodeList
function HtmlNode:select(selector) return wrap_list(self._raw:select(selector)) end

---@param selector string
---@return HtmlNode|nil
function HtmlNode:first(selector) return wrap_node(self._raw:first(selector)) end

---@class HtmlNodeList
local HtmlNodeList = {}

---@return integer
function HtmlNodeList:len() return #self end

---@param index integer
---@return HtmlNode|nil
function HtmlNodeList:get(index) return self[index] end

---@return HtmlNode[]
function HtmlNodeList:to_table()
  local items = {}
  for i, item in ipairs(self) do
    items[i] = item
  end
  return items
end

wrap_node = function(raw)
  if not raw then return nil end
  return setmetatable({ _raw = raw }, { __index = HtmlNode })
end

wrap_list = function(raw)
  local wrapped = { _raw = raw }
  local items = raw:to_table()
  for i, item in ipairs(items) do
    wrapped[i] = wrap_node(item)
  end
  return setmetatable(wrapped, { __index = HtmlNodeList })
end

local function wrap_document(raw)
  return setmetatable({ _raw = raw }, { __index = HtmlDocument })
end

---@class deck.html
local html = {}

---@param source string
---@return HtmlDocument
function html.parse(source)
  return wrap_document(_deck.html.parse(source))
end

---@param source string
---@return HtmlDocument
function html.parse_fragment(source)
  return wrap_document(_deck.html.parse_fragment(source))
end

---@param source string
---@return string
function html.to_markdown(source)
  return _deck.html.to_markdown(source)
end

deck.html = html
