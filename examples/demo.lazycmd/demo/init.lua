local config = require 'demo.config'
local action = require 'demo.action'

local M = {}

local function span(text, color)
  local s = deck.style.span(tostring(text or ''))
  if color and color ~= '' then s = s:fg(color) end
  return s
end

local function line(parts) return deck.style.line(parts) end
local function text(lines) return deck.style.text(lines) end

local function fs_segments(path)
  local cfg = config.get()
  local route_name = cfg.route_name or 'demo'
  local segments = {}

  for i, segment in ipairs(path or {}) do
    if not (i == 1 and segment == route_name) then
      segments[#segments + 1] = segment
    end
  end

  return segments
end

local function join_path(path)
  local segments = fs_segments(path)
  if #segments == 0 then
    return config.get().root_dir or '.'
  end
  return deck.path.join(segments)
end

local function join_entry_path(dir, name)
  return deck.path.join({ dir, name })
end

local function build_dir_entries(path, stdout)
  local entries = {}
  local dir = join_path(path)

  for raw in (tostring(stdout or '') .. '\n'):gmatch('(.-)\n') do
    if raw ~= '' then
      local is_dir = raw:sub(-1) == '/'
      local name = is_dir and raw:sub(1, -2) or raw

      if name ~= '' then
        table.insert(entries, {
          key = name,
          kind = is_dir and 'dir' or 'file',
          path = join_entry_path(dir, name),
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

local function register_page_keymaps()
  deck.keymap.set('main', 'o', function()
    action.open_file(deck.api.get_hovered())
  end, {
    path = '/**',
    desc = 'open file externally',
  })

  deck.keymap.set('main', 'gr', function()
    deck.cmd('reload')
  end, {
    path = '/**',
    desc = 'reload current page',
  })
end

function M.setup(opt)
  config.setup(opt or {})
  register_page_keymaps()
end

function M.meta()
  return {
    icon = '󰋜',
    desc = 'Demo file browser for page keymap and preview behavior',
    color = 'cyan',
  }
end

function M.list(path, cb)
  local dir = join_path(path)

  deck.system.exec({ 'ls', '-1A', '-p', dir }, function(output)
    if output.code ~= 0 then
      deck.notify('Failed to list directory: ' .. (output.stderr or 'unknown error'))
      return
    end

    cb(build_dir_entries(path, output.stdout))
  end)
end

function M.preview(entry, cb)
  if not entry then return end

  local cfg = config.get()
  local entry_path = entry.path or entry.key

  if entry.kind == 'dir' then
    deck.system.exec({ 'ls', '-la', entry_path }, function(output)
      if output.code ~= 0 then
        cb(text {
          line { span('Failed to list directory', 'red') },
          line { span(output.stderr or 'unknown error', 'darkgray') },
        })
        return
      end

      cb(text {
        line { span(entry_path, 'cyan') },
        line { '' },
        line { span(output.stdout or '', 'white') },
      })
    end)
    return
  end

  deck.fs.read_file(entry_path, { max_chars = cfg.preview_max_chars }, function(content, err, meta)
    if err then
      cb(text {
        line { span('Failed to read file', 'red') },
        line { span(err, 'darkgray') },
      })
      return
    end

    local lines = {
      line { span(entry_path, 'cyan') },
      line { '' },
    }

    for raw in (tostring(content or '') .. '\n'):gmatch('(.-)\n') do
      table.insert(lines, line { raw })
    end

    if meta and meta.truncated then
      table.insert(lines, line { '' })
      table.insert(lines, line { span('[truncated]', 'yellow') })
    end

    cb(text(lines))
  end)
end

return M
