---@class deck.time
local time = {}

---Get the current Unix timestamp
---@return number timestamp The current Unix timestamp (seconds since epoch)
function time.now() return _deck.time.now() end

---Parse an ISO 8601 datetime string and return Unix timestamp
---@param time_str string The time string to parse (e.g., "2023-12-25T15:30:45Z", "2023-12-25T15:30:45+08:00")
---@return number timestamp The Unix timestamp (seconds since epoch)
function time.parse(time_str) return _deck.time.parse(time_str) end

---Format a Unix timestamp to an ISO 8601 string (or custom format)
---@param timestamp number The Unix timestamp (seconds since epoch)
---@param format_str string? Optional format string:
--- - "compact" - Compact format: HH:MM for today, MM-DD for this year, YYYY-MM for older dates
--- - "relative" - GitHub-style relative time: "47 minutes ago", "yesterday", "last week", "in 2 hours"
--- - "%Y-%m-%d" or any chrono format string
--- - Defaults to ISO 8601 (e.g., "2023-12-25T15:30:45Z")
---@return string formatted The formatted datetime string
function time.format(timestamp, format_str) return _deck.time.format(timestamp, format_str) end

deck.time = time
