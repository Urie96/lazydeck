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
---
--- @example
---   local str = "Hello 世界！🌍"
---   utf8_sub(str, 1, 5)    -- "Hello"
---   utf8_sub(str, 7, 8)    -- "世界"
---   utf8_sub(str, 7)       -- "世界！🌍"
---   utf8_sub(str, -3, -1)  -- "界！🌍"
function string.utf8_sub(s, i, j)
  local len = utf8.len(s)
  if not len then error 'invalid UTF-8 string' end

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
  local start_pos = utf8.offset(s, i)
  local end_pos

  if j == len then
    end_pos = #s + 1
  else
    end_pos = utf8.offset(s, j + 1)
    if end_pos then
      end_pos = end_pos - 1
    else
      end_pos = #s
    end
  end

  return string.sub(s, start_pos, end_pos)
end
