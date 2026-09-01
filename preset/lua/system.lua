---@class CommandOutput
---@field code number Exit code
---@field stdout string Standard output
---@field stderr string Standard error

---@class SystemOptions
---@field stdin string? Optional standard input to provide to the command
---@field env table<string, string>? Optional environment variables to set for the command
---@field sudo boolean? Run the command with sudo. When true, first probes whether passwordless sudo is available (sudo -n true);
---  if yes, runs `sudo <cmd>` directly; otherwise shows a password input dialog and pipes the password to sudo's stdin (`sudo -S -p ''`).
---@field callback fun(output: CommandOutput)? Callback function called on completion

---@class deck.system
local system = {}

---Run a command with sudo, following the suda.vim reference logic:
---1. Probe passwordless sudo first: `sudo -n true` (succeeds when a valid sudo
---   timestamp cache exists or NOPASSWD rules allow it).
---2. If passwordless, run `sudo <cmd>` directly.
---3. Otherwise show a secret input dialog for the password, then run
---   `sudo -S -p '' <cmd>` with the password as the first line of stdin
---   (`-S` reads the password from stdin instead of /dev/tty; `-p ''` keeps
---   the prompt out of stdin). Any user-provided stdin is appended after the
---   password line so it is passed through to the wrapped command.
---@param args_table table Parsed args table (cmd, callback, stdin, env)
---@param cmd string[] Original command without sudo
local function exec_with_sudo(args_table, cmd)
  _deck.system.exec {
    cmd = { 'sudo', '-n', 'true' },
    callback = function(probe)
      if probe.code == 0 then
        -- Passwordless sudo available: run directly
        args_table.sudo = nil
        args_table.cmd = { 'sudo' }
        for _, part in ipairs(cmd) do
          table.insert(args_table.cmd, part)
        end
        _deck.system.exec(args_table)
        return
      end

      -- Password required: ask the user via a masked input dialog
      deck.input {
        prompt = 'sudo password',
        placeholder = 'Enter sudo password',
        secret = true,
        on_submit = function(password)
          if password == nil or password == '' then
            args_table.callback { code = 1, stdout = '', stderr = 'sudo: no password provided' }
            return
          end
          args_table.sudo = nil
          args_table.cmd = { 'sudo', '-S', '-p', '' }
          for _, part in ipairs(cmd) do
            table.insert(args_table.cmd, part)
          end
          -- Password goes first; sudo consumes only the first line as password
          args_table.stdin = password .. '\n' .. (args_table.stdin or '')
          _deck.system.exec(args_table)
        end,
        on_cancel = function()
          args_table.callback { code = 1, stdout = '', stderr = 'sudo: password entry cancelled' }
        end,
      }
    end,
  }
end

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
    if opts_or_callback.sudo ~= nil then args_table.sudo = opts_or_callback.sudo end
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

  -- Run with sudo: probe passwordless first, ask for password if needed
  if args_table.sudo then
    exec_with_sudo(args_table, cmd)
    return
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
