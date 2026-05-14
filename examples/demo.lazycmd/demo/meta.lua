local M = {}
local action = require 'demo.action'
local config = require 'demo.config'

local function span(text, color)
  local s = deck.style.span(tostring(text or ''))
  if color and color ~= '' then s = s:fg(color) end
  return s
end
local function line(parts) return deck.style.line(parts) end
local function text(lines) return deck.style.text(lines) end
local function lines_from_string(content)
  local lines = {}

  for raw in (tostring(content or '') .. '\n'):gmatch '(.-)\n' do
    table.insert(lines, line { raw })
  end

  return lines
end

local function text_from_string(content) return text(lines_from_string(content)) end

local metas = {
  dir = {
    __index = {
      keymap = {},
      preview = function(entry, cb)
        deck.system.exec({ 'ls', '-la', entry.key }, function(output)
          if output.code ~= 0 then
            cb(text {
              line { span('Failed to list directory', 'red') },
              line { span(output.stderr or 'unknown error', 'darkgray') },
            })
            return
          end

          cb(text_from_string(output.stdout))
        end)
      end,
    },
  },
  file = {
    __index = {
      keymap = {},
      preview = function(entry, cb)
        local cfg = config.get()
        deck.fs.read_file(entry.key, { max_chars = cfg.preview_max_chars }, function(content, err, meta)
          if err then
            cb(text {
              line { span('Failed to read file', 'red') },
              line { span(err, 'darkgray') },
            })
            return
          end

          local lines = {
            line { span(entry.path, 'cyan') },
            line { '' },
          }

          for _, preview_line in ipairs(lines_from_string(content)) do
            table.insert(lines, preview_line)
          end

          if meta and meta.truncated then
            table.insert(lines, line { '' })
            table.insert(lines, line { span('[truncated]', 'yellow') })
          end

          cb(text(lines))
        end)
      end,
    },
  },
  info = {
    __index = {
      keymap = {},
      preview = function(entry, cb)
        cb(text {
          line { span(entry.message or 'Info', entry.color or 'darkgray') },
        })
      end,
    },
  },
}

local function add_keymap(targets, key, callback, desc)
  if not key or key == '' then return end
  for _, target in ipairs(targets) do
    target[key] = { callback = callback, desc = desc }
  end
end

function M.setup()
  local cfg = config.get()
  local keymap = cfg.keymap or {}

  local file_keymap = metas.file.__index.keymap

  add_keymap({ file_keymap }, keymap.open_file, action.open_file, 'open file externally')
end

function M.attach(entries)
  for i, entry in ipairs(entries or {}) do
    local mt = metas[entry.kind]
    if mt then entries[i] = setmetatable(entry, mt) end
  end
  return entries
end

return M
