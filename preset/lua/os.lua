---Return the operating system this lazydeck binary was compiled for.
---The value is fixed at compile time and never changes at runtime.
---@return string target_os One of "linux", "macos", "android", "windows", "ios", "freebsd", ...
function deck.target_os() return _deck.target_os() end
