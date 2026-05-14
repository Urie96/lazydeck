--- Checks if a table is empty.
---
---@see https://github.com/premake/premake-core/blob/master/src/base/table.lua
---
---@param t table Table to check
---@return boolean `true` if `t` is empty
function deck.tbl_isempty(t)
  if type(t) ~= 'table' then return false end
  return next(t) == nil
end

--- Tests if `t` is a "list": a table indexed _only_ by contiguous integers starting from 1 (what
--- |lua-length| calls a "regular array").
---
--- Empty table `{}` is a list, unless it was created by |vim.empty_dict()| or returned as
--- a dict-like |API| or Vimscript result, for example from |rpcrequest()| or |vim.fn|.
---
---@see |vim.isarray()|
---
---@param t? table
---@return boolean `true` if list-like table, else `false`.
function deck.islist(t)
  if type(t) ~= 'table' then return false end

  if next(t) == nil then return true end

  local j = 1
  for _ in
    pairs(t--[[@as table<any,any>]])
  do
    if t[j] == nil then return false end
    j = j + 1
  end

  return true
end

--- We only merge empty tables or tables that are not list-like (indexed by consecutive integers
--- starting from 1)
local function can_merge(v) return type(v) == 'table' and (deck.tbl_isempty(v) or not deck.islist(v)) end

--- Recursive worker for tbl_extend
--- @param behavior 'error'|'keep'|'force'
--- @param deep_extend boolean
--- @param ... table<any,any>
local function tbl_extend_rec(behavior, deep_extend, ...)
  local ret = {} --- @type table<any,any>
  if deck._empty_dict_mt ~= nil and getmetatable(select(1, ...)) == deck._empty_dict_mt then ret = deck.empty_dict() end

  for i = 1, select('#', ...) do
    local tbl = select(i, ...) --[[@as table<any,any>]]
    if tbl then
      for k, v in pairs(tbl) do
        if deep_extend and can_merge(v) and can_merge(ret[k]) then
          ret[k] = tbl_extend_rec(behavior, true, ret[k], v)
        elseif behavior ~= 'force' and ret[k] ~= nil then
          if behavior == 'error' then error('key found in more than one map: ' .. k) end -- Else behavior is "keep".
        else
          ret[k] = v
        end
      end
    end
  end

  return ret
end

--- @param behavior 'error'|'keep'|'force'
--- @param deep_extend boolean
--- @param ... table<any,any>
local function tbl_extend(behavior, deep_extend, ...)
  if behavior ~= 'error' and behavior ~= 'keep' and behavior ~= 'force' then
    error('invalid "behavior": ' .. tostring(behavior))
  end

  local nargs = select('#', ...)

  if nargs < 2 then error(('wrong number of arguments (given %d, expected at least 3)'):format(1 + nargs)) end

  -- for i = 1, nargs do
  --   deck.validate('after the second argument', select(i, ...), 'table')
  -- end

  return tbl_extend_rec(behavior, deep_extend, ...)
end

--- Merges two or more tables.
---
---@see |extend()|
---
---@param behavior 'error'|'keep'|'force' Decides what to do if a key is found in more than one map:
---      - "error": raise an error
---      - "keep":  use value from the leftmost map
---      - "force": use value from the rightmost map
---@param ... table Two or more tables
---@return table : Merged table
function deck.tbl_extend(behavior, ...) return tbl_extend(behavior, false, ...) end

--- Merges recursively two or more tables.
---
--- Only values that are empty tables or tables that are not |lua-list|s (indexed by consecutive
--- integers starting from 1) are merged recursively. This is useful for merging nested tables
--- like default and user configurations where lists should be treated as literals (i.e., are
--- overwritten instead of merged).
---
---@see |deck.tbl_extend()|
---
---@generic T1: table
---@generic T2: table
---@param behavior 'error'|'keep'|'force' Decides what to do if a key is found in more than one map:
---      - "error": raise an error
---      - "keep":  use value from the leftmost map
---      - "force": use value from the rightmost map
---@param ... T2 Two or more tables
---@return T1|T2 (table) Merged table
function deck.tbl_deep_extend(behavior, ...) return tbl_extend(behavior, true, ...) end

--- Deep compare values for equality
---
--- Tables are compared recursively unless they both provide the `eq` metamethod.
--- All other types are compared using the equality `==` operator.
---@param a any First value
---@param b any Second value
---@return boolean `true` if values are equals, else `false`
function deck.deep_equal(a, b)
  if a == b then return true end
  if type(a) ~= type(b) then return false end
  if type(a) == 'table' then
    --- @cast a table<any,any>
    --- @cast b table<any,any>
    for k, v in pairs(a) do
      if not deck.deep_equal(v, b[k]) then return false end
    end
    for k in pairs(b) do
      if a[k] == nil then return false end
    end
    return true
  end
  return false
end

--- Apply a function to all values of a table.
---
---@generic T
---@param func fun(value: T): any Function
---@param t table<any, T> Table
---@return table : Table of transformed values
function deck.tbl_map(func, t)
  local rettab = {} --- @type table<any,any>
  for k, v in pairs(t) do
    rettab[k] = func(v)
  end
  return rettab
end

--- Filter a table using a predicate function
---
---@generic T
---@param func fun(value: T): boolean (function) Function
---@param t table<any, T> (table) Table
---@return T[] : Table of filtered values
function deck.tbl_filter(func, t)
  local rettab = {} --- @type table<any,any>
  for _, entry in pairs(t) do
    if func(entry) then rettab[#rettab + 1] = entry end
  end
  return rettab
end

---@generic T: table
---@param dst T List which will be modified and appended to
---@param src table List from which values will be inserted
---@param start integer? Start index on src. Defaults to 1
---@param finish integer? Final index on src. Defaults to `#src`
---@return T dst
function deck.list_extend(dst, src, start, finish)
  for i = start or 1, finish or #src do
    table.insert(dst, src[i])
  end
  return dst
end
