---@class CommandOutput
---@field code number Exit code
---@field stdout string Standard output
---@field stderr string Standard error

---@class SystemOptions
---@field stdin string? Optional standard input to provide to the command
---@field env table<string, string>? Optional environment variables to set for the command
---@field callback fun(output: CommandOutput)? Callback function called on completion

---@class deck.system
local system = {}

---Execute an external command asynchronously (Lua wrapper)
---This wrapper provides multiple convenient call formats:
---Usage 1: deck.system.exec({cmd, callback})
---Usage 2: deck.system.exec(cmd, callback)
---Usage 3: deck.system.exec(cmd, opts, callback)
---
---The wrapper calls deck.system._exec internally after parameter processing
---@param cmd table The arguments table or command array
---@param opts_or_callback SystemOptions|fun(output: CommandOutput)? Options table or callback function
---@param callback fun(output: CommandOutput)? Callback function
function system.exec(cmd, opts_or_callback, callback)
  -- Parse arguments:
  -- deck.system.exec(cmd, callback)
  -- deck.system.exec(cmd, opts, callback)

  local args_table = { cmd = cmd }

  if type(opts_or_callback) == 'function' then
    -- deck.system.exec(cmd, callback)
    args_table.callback = opts_or_callback
  elseif type(opts_or_callback) == 'table' then
    -- deck.system.exec(cmd, opts, callback)
    if opts_or_callback.stdin ~= nil then args_table.stdin = opts_or_callback.stdin end
    if opts_or_callback.env ~= nil then args_table.env = opts_or_callback.env end
    if type(callback) == 'function' then
      args_table.callback = callback
    elseif opts_or_callback.callback ~= nil then
      args_table.callback = opts_or_callback.callback
    else
      error 'Callback function is required when providing options'
    end
  else
    error 'Callback function is required'
  end

  -- Call the Rust implementation
  _deck.system.exec(args_table)
end

---Check if a command is executable (synchronous)
---@param cmd string The command name to check
---@return boolean executable Whether the command exists and is executable
function system.executable(cmd) return _deck.system.executable(cmd) end

---Spawn a detached background process without waiting for completion.
---@param cmd string[] The command and its arguments
---@return integer pid Spawned process id
function system.spawn(cmd) return _deck.system.spawn({ cmd = cmd }) end

---Send a signal to a process.
---@param pid integer Process id
---@param signal integer? Signal number, defaults to SIGTERM
function system.kill(pid, signal) return _deck.system.kill(pid, signal) end

---Open a file using the system's default application
---Cross-platform support: uses 'open' on macOS, 'xdg-open' on Linux, 'start' on Windows
---@param file_path string The path to the file to open
function system.open(file_path) return _deck.system.open(file_path) end

---@class SystemEditOptions
---@field path string? Optional file path to edit directly; when provided, editor opens this file in place instead of a temp file
---@field content string? Optional initial content; when path is also provided, this content is written to path before opening editor
---@field ext string? Optional temp file suffix/extension used when path is not provided, e.g. "rs" or ".rs"

---Open external editor and optionally return edited content plus optional error.
---@param opts SystemEditOptions
---@param callback fun(content: string|nil, error: string|nil)?
function system.edit(opts, callback) return _deck.system.edit(opts, callback) end

deck.system = system

-- Set metatable on deck.system to handle multiple argument formats
setmetatable(deck.system, {
  __call = function(self, cmd, opts_or_callback, callback) deck.system.exec(cmd, opts_or_callback, callback) end,
})
