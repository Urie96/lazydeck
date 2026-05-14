---Set text color for display
---@param s string content
---@param color string Color name (e.g., "blue", "red", "green")
---@return Span A colored span widget
function string.fg(s, color) return _deck.style.span(s):fg(color) end

---Apply bold style for display
---@param s string content
---@return Span A bold span widget
function string.bold(s) return _deck.style.span(s):bold() end

---Apply italic style for display
---@param s string content
---@return Span An italic span widget
function string.italic(s) return _deck.style.span(s):italic() end

---Apply underline style for display
---@param s string content
---@return Span An underlined span widget
function string.underline(s) return _deck.style.span(s):underline() end

---Parse ANSI escape sequences into a TUI Text widget
---@param s string content
---@return Text A Text widget with parsed ANSI codes
function string.ansi(s) return _deck.style.ansi(s) end

---Split string by separator
---@param s string content
---@param sep string The separator
---@return string[] The split parts
function string.split(s, sep) return _deck.split(s, sep) end

---Trim leading and trailing whitespace from a string
---@param s string content
---@return string trimmed The trimmed string
function string.trim(s) return string.match(s, '^%s*(.-)%s*$') end

local utf8lib = rawget(_G, 'utf8')

local function utf8_char_positions(s)
  local positions = {}
  local pos = 1
  local bytes = #s

  while pos <= bytes do
    local b1 = string.byte(s, pos)
    local width

    if b1 <= 0x7F then
      width = 1
    elseif b1 >= 0xC2 and b1 <= 0xDF then
      width = 2
    elseif b1 >= 0xE0 and b1 <= 0xEF then
      width = 3
    elseif b1 >= 0xF0 and b1 <= 0xF4 then
      width = 4
    else
      return nil
    end

    for offset = 1, width - 1 do
      local continuation = string.byte(s, pos + offset)
      if not continuation or continuation < 0x80 or continuation > 0xBF then return nil end
    end

    if width == 3 then
      local b2 = string.byte(s, pos + 1)
      if (b1 == 0xE0 and b2 < 0xA0) or (b1 == 0xED and b2 > 0x9F) then return nil end
    elseif width == 4 then
      local b2 = string.byte(s, pos + 1)
      if (b1 == 0xF0 and b2 < 0x90) or (b1 == 0xF4 and b2 > 0x8F) then return nil end
    end

    positions[#positions + 1] = pos
    pos = pos + width
  end

  positions[#positions + 1] = bytes + 1
  return positions
end

--- Extracts a substring from a UTF-8 encoded string, operating on characters rather than bytes.
---
--- @param s string The UTF-8 encoded string.
--- @param i integer The starting character position (1-indexed). Negative values count from the end of the string (-1 = last character).
--- @param j integer|nil The ending character position (inclusive). If nil, defaults to the last character. Negative values count from the end.
---
--- @return string Returns the substring containing characters from position i to j (inclusive).
---                    Returns nil and an error message if the input is not a valid UTF-8 string.
---                    Returns an empty string if i > j.
---
--- @note This function behaves similarly to string.sub but operates on Unicode characters instead of bytes.
---       On Lua 5.3+/5.4 it uses the built-in utf8 library; on LuaJIT it falls back to a compatible pure-Lua implementation.
---
--- @example
---   local str = "Hello 世界！🌍"
---   utf8_sub(str, 1, 5)    -- "Hello"
---   utf8_sub(str, 7, 8)    -- "世界"
---   utf8_sub(str, 7)       -- "世界！🌍"
---   utf8_sub(str, -3, -1)  -- "界！🌍"
function string.utf8_sub(s, i, j)
  local len
  local positions

  if utf8lib then
    len = utf8lib.len(s)
    if not len then error 'invalid UTF-8 string' end
  else
    positions = utf8_char_positions(s)
    if not positions then error 'invalid UTF-8 string' end
    len = #positions - 1
  end

  -- 处理负索引（从字符串末尾开始计数）
  if i < 0 then i = len + i + 1 end
  if j then
    if j < 0 then j = len + j + 1 end
  else
    j = len
  end

  -- 边界检查
  if i < 1 then i = 1 end
  if j > len then j = len end
  if i > j then return '' end

  -- 获取起始和结束位置的字节偏移
  local start_pos = utf8lib and utf8lib.offset(s, i) or positions[i]
  local end_pos

  if j == len then
    -- 如果是到字符串末尾，直接取到结尾
    end_pos = #s + 1
  else
    -- 获取第 j+1 个字符的起始位置，然后减1得到第 j 个字符的结束位置
    end_pos = utf8lib and utf8lib.offset(s, j + 1) or positions[j + 1]
    if end_pos then
      end_pos = end_pos - 1
    else
      -- 如果 j+1 超出范围，取到字符串末尾
      end_pos = #s
    end
  end

  -- 返回子串
  return string.sub(s, start_pos, end_pos)
end
