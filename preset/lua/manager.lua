--- Plugin Manager UI: built-in plugin that displays the plugin list
--- and allows installing, updating, and restoring plugins.

local M = {}
local pm -- will be set to deck._pm

local PLUGIN_USAGE_CACHE_NS = 'lazydeck.plugin.usage'

local function normalize_plugin_usage(usage)
  if type(usage) ~= 'table' then usage = {} end
  return {
    count = tonumber(usage.count) or 0,
    last_used = tonumber(usage.last_used) or 0,
  }
end

local function plugin_usage(name)
  return normalize_plugin_usage(deck.cache.get(PLUGIN_USAGE_CACHE_NS, name))
end

local function format_last_used(ts)
  ts = tonumber(ts) or 0
  if ts <= 0 then return 'never' end
  return deck.time.format(ts, 'relative')
end

--- Setup the plugin manager UI with keybindings.
--- @param plugins table Array of plugin spec tables from user config
function M.setup(plugins)
  pm = deck._pm
  M.plugins = pm.flatten_plugins(plugins or {})

  M._update_status = {} -- Track per-plugin update check results

  local root_path = {}

  -- U: Update all plugins
  deck.keymap.set('main', 'U', function()
    deck.notify(deck.style.line {
      deck.style.span('⟳ '):fg 'cyan',
      deck.style.span 'Updating all plugins...',
    })
    pm.update_all(plugins or {}, function()
      M._update_status = {}
      deck.cmd 'reload'
    end)
  end, { path = root_path, desc = 'update all plugins' })

  -- S: Restore all plugins from lock file
  deck.keymap.set('main', 'S', function()
    deck.confirm {
      title = 'Restore from Lock File',
      prompt = 'Restore all plugins to locked versions?',
      on_confirm = function()
        deck.notify(deck.style.line {
          deck.style.span('⟳ '):fg 'cyan',
          deck.style.span 'Restoring from lock file...',
        })
        pm.restore_all(plugins or {}, function()
          M._update_status = {}
          deck.cmd 'reload'
        end)
      end,
    }
  end, { path = root_path, desc = 'restore plugins from lock file' })

  -- u: Update current plugin
  deck.keymap.set('main', 'u', function()
    local entry = deck.api.get_hovered()
    if not entry then return end

    local spec = M.find_spec_by_name(entry.key)
    if not spec or not spec.is_remote then
      deck.notify(deck.style.line {
        deck.style.span('⊘ '):fg 'yellow',
        deck.style.span(entry.key .. ' is a local plugin'),
      })
      return
    end

    deck.notify(deck.style.line {
      deck.style.span('⟳ '):fg 'cyan',
      deck.style.span('Updating ' .. spec.name .. '...'),
    })
    pm.update(spec, function(success)
      if success then
        M._update_status[spec.name] = nil -- Clear cached status
        deck.notify(deck.style.line {
          deck.style.span('✓ '):fg 'green',
          deck.style.span(spec.name .. ' updated'),
        })
      end
      deck.cmd 'reload'
    end)
  end, { path = root_path, desc = 'update plugin' })

  -- i: Install current missing plugin
  deck.keymap.set('main', 'i', function()
    local entry = deck.api.get_hovered()
    if not entry then return end

    if entry.status ~= 'missing' then
      deck.notify(deck.style.line {
        deck.style.span('⊘ '):fg 'yellow',
        deck.style.span(entry.key .. ' is already installed'),
      })
      return
    end

    local spec = M.find_spec_by_name(entry.key)
    if not spec then return end

    deck.notify(deck.style.line {
      deck.style.span('⟳ '):fg 'cyan',
      deck.style.span('Installing ' .. spec.name .. '...'),
    })
    pm.install(spec, function(success)
      if success then
        deck.notify(deck.style.line {
          deck.style.span('✓ '):fg 'green',
          deck.style.span(spec.name .. ' installed'),
        })
      end
      deck.cmd 'reload'
    end)
  end, { path = root_path, desc = 'install plugin' })
end

--- Find a parsed plugin spec by plugin name.
--- @param name string Plugin name
--- @return table|nil Parsed plugin spec
function M.find_spec_by_name(name)
  for _, spec in ipairs(M.plugins) do
    if spec.name == name then return spec end
  end
end

--- List all configured plugins for the UI.
--- @param path table Current path
--- @param cb function Callback with entries
function M.list(path, cb)
  -- Debug: track calls
  M._call_count = (M._call_count or 0) + 1

  local entries = {}
  local lock = pm.read_lock()

  for _, spec in ipairs(M.plugins) do
    local installed = pm.is_installed(spec)
    local status = installed and 'installed' or 'missing'

    -- Build constraint label
    local constraint = ''
    if spec.tag then
      constraint = ' [tag:' .. spec.tag .. ']'
    elseif spec.branch then
      constraint = ' [branch:' .. spec.branch .. ']'
    elseif spec.commit then
      constraint = ' [commit:' .. spec.commit:sub(1, 7) .. ']'
    end

    -- Build source label
    local source_label = ''
    if spec.is_remote then
      source_label = ' (' .. spec.repo .. ')'
    else
      source_label = ' (local)'
    end

    -- Lock info
    local lock_info = ''
    local lock_entry = lock[spec.name]
    if lock_entry and lock_entry.commit then lock_info = ' @' .. lock_entry.commit:sub(1, 7) end

    -- Status icon and color
    local status_icon = installed and '✓' or '✗'
    local icon_color = installed and 'green' or 'red'

    table.insert(entries, {
      key = spec.name,
      status = status,
      repo = spec.repo,
      is_remote = spec.is_remote,
      display = deck.style.line {
        deck.style.span(status_icon .. ' '):fg(icon_color),
        deck.style.span(spec.name):fg 'white',
        deck.style.span(source_label):fg 'gray',
        deck.style.span(constraint):fg 'yellow',
        deck.style.span(lock_info):fg 'cyan',
      },
    })
  end

  cb(entries)
end

--- Show preview for the hovered plugin.
--- @param entry table Hovered entry
--- @param cb function Callback with preview widget
function M.preview(entry, cb)
  local spec = M.find_spec_by_name(entry.key)
  if not spec then
    cb(deck.style.text { deck.style.line { deck.style.span('Plugin not found'):fg 'red' } })
    return
  end

  local lock = pm.read_lock()
  local lock_entry = lock[spec.name]
  local usage = plugin_usage(spec.name)
  if entry.usage_count ~= nil then usage.count = tonumber(entry.usage_count) or usage.count end
  if entry.last_used ~= nil then usage.last_used = tonumber(entry.last_used) or usage.last_used end

  -- Helper: build the preview lines array as LuaLine objects
  local function build_lines(extra_lines)
    local lines = {}

    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Plugin: '):fg 'cyan',
        deck.style.span(spec.name):fg 'white',
      }
    )

    if spec.is_remote then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Repo:   '):fg 'cyan',
          deck.style.span(spec.repo):fg 'white',
        }
      )
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('URL:    '):fg 'cyan',
          deck.style.span(spec.url):fg 'gray',
        }
      )
    else
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Source: '):fg 'cyan',
          deck.style.span('local'):fg 'white',
        }
      )
    end

    local status_color = entry.status == 'installed' and 'green' or 'red'
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Status: '):fg 'cyan',
        deck.style.span(entry.status):fg(status_color),
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Usage:  '):fg 'cyan',
        deck.style.span(tostring(usage.count)):fg 'white',
        deck.style.span(' time(s)'):fg 'gray',
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Recent: '):fg 'cyan',
        deck.style.span(format_last_used(usage.last_used)):fg(usage.last_used > 0 and 'white' or 'gray'),
      }
    )

    if spec.tag then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Tag:    '):fg 'cyan',
          deck.style.span(spec.tag):fg 'yellow',
        }
      )
    elseif spec.branch then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Branch: '):fg 'cyan',
          deck.style.span(spec.branch):fg 'yellow',
        }
      )
    elseif spec.commit then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Commit: '):fg 'cyan',
          deck.style.span(spec.commit):fg 'yellow',
        }
      )
    end

    if lock_entry then
      table.insert(lines, deck.style.line {})
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Lock File:'):fg 'magenta',
        }
      )
      if lock_entry.commit then
        table.insert(
          lines,
          deck.style.line {
            deck.style.span('   Commit: '):fg 'gray',
            deck.style.span(lock_entry.commit):fg 'white',
          }
        )
      end
      if lock_entry.branch then
        table.insert(
          lines,
          deck.style.line {
            deck.style.span('   Branch: '):fg 'gray',
            deck.style.span(lock_entry.branch):fg 'white',
          }
        )
      end
      if lock_entry.tag then
        table.insert(
          lines,
          deck.style.line {
            deck.style.span('   Tag:    '):fg 'gray',
            deck.style.span(lock_entry.tag):fg 'white',
          }
        )
      end
    end

    table.insert(lines, deck.style.line {})
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Keybindings:'):fg 'magenta',
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('   U'):fg 'green',
        deck.style.span(' Update all plugins'):fg 'gray',
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('   u'):fg 'green',
        deck.style.span(' Update this plugin'):fg 'gray',
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('   S'):fg 'green',
        deck.style.span(' Restore all from lock file'):fg 'gray',
      }
    )
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('   i'):fg 'green',
        deck.style.span(' Install missing plugin'):fg 'gray',
      }
    )

    -- Append extra lines (e.g., update status)
    if extra_lines then
      for _, v in ipairs(extra_lines) do
        table.insert(lines, v)
      end
    end

    return deck.style.text(lines)
  end

  -- Build update status lines helper
  local function build_update_lines(has_update, remote_info)
    local lines = {}
    table.insert(lines, deck.style.line {})
    if has_update then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('New version available!'):fg 'yellow',
        }
      )
      if remote_info then
        table.insert(
          lines,
          deck.style.line {
            deck.style.span('  Remote: '):fg 'gray',
            deck.style.span(remote_info):fg 'white',
          }
        )
      end
    else
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('Up to date'):fg 'green',
        }
      )
    end
    return lines
  end

  -- Async: check for updates (only for installed remote plugins)
  if spec.is_remote and entry.status == 'installed' then
    -- Show basic info first
    cb(build_lines(nil))

    -- Use cached result if available
    if M._update_status[spec.name] ~= nil then
      local cached = M._update_status[spec.name]
      cb(build_lines(build_update_lines(cached.has_update, cached.remote_info)))
      return
    end

    pm.check_update(spec, function(has_update, remote_info)
      M._update_status[spec.name] = {
        has_update = has_update,
        remote_info = remote_info,
      }

      local current_entry = deck.api.get_hovered()
      if current_entry and current_entry.key == entry.key then
        cb(build_lines(build_update_lines(has_update, remote_info)))
      end
    end)
  else
    cb(build_lines(nil))
  end
end

-- Attach _manager to the same underlying table that _deck points to.
deck._manager = M
