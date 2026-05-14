local config = require 'demo.config'
local meta = require 'demo.meta'

local M = {}

local function span(text, color)
  local s = deck.style.span(tostring(text or ''))
  if color and color ~= '' then s = s:fg(color) end
  return s
end

local function line(parts) return deck.style.line(parts) end

local function build_dir_entries(stdout)
  local entries = {}

  for raw in (tostring(stdout or '') .. '\n'):gmatch '(.-)\n' do
    if raw ~= '' then
      local is_dir = raw:sub(-1) == '/'
      local name = is_dir and raw:sub(1, -2) or raw

      if name ~= '' then
        table.insert(entries, {
          key = name,
          kind = is_dir and 'dir' or 'file',
          display = line {
            span(is_dir and '[D] ' or '[F] ', is_dir and 'cyan' or 'yellow'),
            span(name, 'white'),
          },
        })
      end
    end
  end

  return entries
end

function M.setup(opt)
  config.setup(opt or {})
  meta.setup {}
end

function M.list(path, cb)
  deck.system.exec({ 'ls', '-1A', '-p' }, function(output)
    if output.code ~= 0 then
      deck.notify('Failed to list directory' .. output.stderr)
      return
    end

    cb(meta.attach(build_dir_entries(output.stdout)))
  end)
end

return M
