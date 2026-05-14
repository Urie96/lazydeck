---@class InteractiveOptions
---@field wait_confirm boolean|fun(exit_code: number):boolean? Whether to wait for user to press Enter before returning to lazydeck.
---  If true: always wait
---  If false: never wait (default)
---  If function: called with exit code, return true to wait, false to skip waiting
---@field on_complete fun(exit_code: number)? Optional callback function called when command exits

---@class InteractiveArgs
---@field cmd string[] The command and its arguments
---@field wait_confirm fun(exit_code: number):boolean? Function to decide whether to wait for confirmation based on exit code
---@field on_complete fun(exit_code: number)? Optional callback function called when command exits

---Execute a command in interactive mode (with terminal access)
---This Lua wrapper provides multiple convenient call formats:
---Usage 1: deck.interactive({"cmd", "arg1", "arg2"})
---Usage 2: deck.interactive({"cmd", "arg1", "arg2"}, callback)
---Usage 3: deck.interactive({"cmd", "arg1", "arg2"}, {wait_confirm = true})
---Usage 4: deck.interactive({"cmd", "arg1", "arg2"}, {wait_confirm = function(code) return code ~= 0 end})
---Usage 5: deck.interactive({"cmd", "arg1", "arg2"}, {wait_confirm = true}, callback)
---
---The underlying Rust implementation receives a table with all fields:
---  _interactive_rust({cmd = ..., wait_confirm = ..., on_complete = ...})
---
---The wait_confirm option:
---  - If boolean true: always wait for Enter press after command exits
---  - If boolean false or nil: never wait (default)
---  - If function: called with exit_code as argument, should return boolean
---    - Example: {wait_confirm = function(code) return code ~= 0 end} -- wait only on error
---    - Example: {wait_confirm = function(code) return code > 1 end} -- wait only on severe errors
---@param cmd string[] The command and its arguments
---@param opts_or_callback InteractiveOptions|fun(exit_code: number)? Either options table or callback function
---@param callback fun(exit_code: number)? Optional callback function called when command exits
function deck.interactive(cmd, opts_or_callback, callback)
  -- Parse arguments:
  -- deck.interactive(cmd)
  -- deck.interactive(cmd, callback)
  -- deck.interactive(cmd, opts)
  -- deck.interactive(cmd, opts, callback)

  local args_table = {
    cmd = cmd,
  }

  if type(opts_or_callback) == 'function' then
    -- deck.interactive(cmd, callback)
    args_table.on_complete = opts_or_callback
  elseif type(opts_or_callback) == 'table' then
    -- deck.interactive(cmd, opts) or deck.interactive(cmd, opts, callback)
    if type(opts_or_callback.wait_confirm) == 'function' then
      -- wait_confirm 是一个函数
      args_table.wait_confirm = opts_or_callback.wait_confirm
    elseif opts_or_callback.wait_confirm == true then
      -- wait_confirm 为 true，创建一个总是返回 true 的函数
      args_table.wait_confirm = function() return true end
    end
    if type(callback) == 'function' then
      args_table.on_complete = callback
    elseif opts_or_callback.on_complete ~= nil then
      args_table.on_complete = opts_or_callback.on_complete
    end
  end

  -- Call the Rust implementation with the table
  _deck.system.interactive(args_table)
end
