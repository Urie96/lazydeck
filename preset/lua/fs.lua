---@class ReadDirEntry
---@field name string Entry name
---@field is_dir boolean Whether the entry is a directory
---@field size integer? File size in bytes (when available)

---@class FileStat
---@field exists boolean Whether the file/directory exists
---@field is_file boolean Whether it's a file
---@field is_dir boolean Whether it's a directory
---@field size integer? File size in bytes (when available)
---@field is_readable boolean Whether it's readable
---@field is_writable boolean Whether it's writable
---@field is_executable boolean Whether it's executable

---@class TempfileOptions
---@field prefix string? File name prefix (e.g., "lazydeck")
---@field suffix string? File name suffix/extension (e.g., ".log" or "log")
---@field content string? Initial content to write to the file

---@class deck.fs
local fs = {}

---@class ReadFileOptions
---@field max_chars integer? Read at most this many characters

---@class ReadFileMeta
---@field truncated boolean Whether the returned content was truncated

---Read directory synchronously
---@param path string The directory path to read
---@return ReadDirEntry[] entries List of directory entries
---@return string|nil error Error message if failed
function fs.read_dir_sync(path) return _deck.fs.read_dir_sync(path) end

---Read file content synchronously
---@param path string The file path to read
---@return string content The file content
---@return string|nil error Error message if failed
function fs.read_file_sync(path) return _deck.fs.read_file_sync(path) end

---Read file content asynchronously
---@param path string The file path to read
---@param opts ReadFileOptions|fun(content: string, error: string|nil, meta: ReadFileMeta|nil)
---@param callback fun(content: string, error: string|nil, meta: ReadFileMeta|nil)?
function fs.read_file(path, opts, callback)
  if type(opts) == 'function' then return _deck.fs.read_file(path, nil, opts) end
  return _deck.fs.read_file(path, opts, callback)
end

---Write content to file synchronously
---@param path string The file path to write
---@param content string The content to write
---@return boolean success Whether the write succeeded
---@return string|nil error Error message if failed
function fs.write_file_sync(path, content) return _deck.fs.write_file_sync(path, content) end

---Get file/directory statistics synchronously
---@param path string The file or directory path
---@return FileStat stat Statistics about the path
function fs.stat(path) return _deck.fs.stat(path) end

---Create directory and all parent directories if they don't exist (like mkdir -p)
---@param path string The directory path to create
---@return boolean success Whether the creation succeeded
---@return string|nil error Error message if failed
function fs.mkdir(path) return _deck.fs.mkdir(path) end

---Create a temporary file in system temp directory
---@param opts TempfileOptions? Optional settings for filename
---@return string path The path to the created temporary file
---@return string|nil error Error message if failed
---[[
-- Examples:
--   local path = deck.fs.tempfile()                              -- → "/tmp/tmp.a1b2c3d4"
--   local path = deck.fs.tempfile({prefix = "memo"})             -- → "/tmp/memo.a1b2c3d4"
--   local path = deck.fs.tempfile({suffix = ".log"})             -- → "/tmp/tmp.a1b2c3d4.log"
--   local path = deck.fs.tempfile({prefix = "memo", suffix = ".md"}) -- → "/tmp/memo.a1b2c3d4.md"
--   local path = deck.fs.tempfile({content = "hello world"})    -- → "/tmp/tmp.a1b2c3d4" with content
--]]
function fs.tempfile(opts) return _deck.fs.tempfile(opts) end

---Remove a file or directory
---@param path string The file or directory path to remove (directories are removed recursively)
---@return boolean success Whether the removal succeeded
---@return string|nil error Error message if failed
---[[
-- Examples:
--   local ok, err = deck.fs.remove("/tmp/myfile.txt")     -- Remove file
--   local ok, err = deck.fs.remove("/tmp/mydir")          -- Remove directory recursively
--   if ok then
--     print("Removed successfully")
--   else
--     print("Error: " .. err)
--   end
--]]
function fs.remove(path) return _deck.fs.remove(path) end

deck.fs = fs
