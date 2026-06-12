--- Plugin Manager UI: built-in plugin that displays the plugin list
--- and allows installing, updating, and restoring plugins.

local M = {}
local pm -- will be set to deck._pm

local PLUGIN_USAGE_CACHE_NS = 'lazydeck.plugin.usage'
local MAX_UPDATE_COMMITS_PREVIEW = 10
local UPDATE_BADGE = '󰚰 '
local BREAKING_UPDATE_BADGE = ' '

local function normalize_plugin_usage(usage)
  if type(usage) ~= 'table' then usage = {} end
  return {
    count = tonumber(usage.count) or 0,
    last_used = tonumber(usage.last_used) or 0,
  }
end

local function plugin_usage(name) return normalize_plugin_usage(deck.cache.get(PLUGIN_USAGE_CACHE_NS, name)) end

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
  M._checking_updates = false
  M._check_generation = 0

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

  -- C: Check all plugins for updates
  deck.keymap.set(
    'main',
    'C',
    function() M.check_all_updates(plugins or {}) end,
    { path = root_path, desc = 'check plugin updates' }
  )

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

function M.update_status(name) return M._update_status and M._update_status[name] or nil end

function M.update_badge(name)
  local status = M.update_status(name)
  if not status or not status.has_update then return nil end
  if status.breaking then return UPDATE_BADGE .. BREAKING_UPDATE_BADGE, 'red' end
  return UPDATE_BADGE, 'yellow'
end

function M.check_all_updates(plugins)
  if M._checking_updates then
    deck.notify(deck.style.line {
      deck.style.span('⟳ '):fg 'cyan',
      deck.style.span 'Already checking plugin updates...',
    })
    return
  end

  local specs = {}
  for _, spec in ipairs(pm.flatten_plugins(plugins or {})) do
    if spec.is_remote and pm.is_installed(spec) then table.insert(specs, spec) end
  end

  M._update_status = {}
  M._checking_updates = true
  M._check_generation = (M._check_generation or 0) + 1
  local generation = M._check_generation

  if #specs == 0 then
    M._checking_updates = false
    deck.notify(deck.style.line {
      deck.style.span('✓ '):fg 'green',
      deck.style.span 'No installed remote plugins to check',
    })
    deck.cmd 'reload'
    return
  end

  deck.notify(deck.style.line {
    deck.style.span('⟳ '):fg 'cyan',
    deck.style.span('Checking updates for ' .. #specs .. ' plugin(s)...'),
  })
  deck.cmd 'reload'

  local remaining = #specs
  local updated = 0
  local breaking = 0

  local function finish_one(spec, result)
    if generation ~= M._check_generation then return end

    result = result or { name = spec.name, has_update = false, commits = {} }
    M._update_status[spec.name] = result
    if result.has_update then updated = updated + 1 end
    if result.has_update and result.breaking then breaking = breaking + 1 end

    remaining = remaining - 1
    if remaining <= 0 then
      M._checking_updates = false
      local message = 'Checked ' .. #specs .. ' plugin(s): ' .. updated .. ' update(s)'
      if breaking > 0 then message = message .. ', ' .. breaking .. ' breaking' end
      deck.notify(deck.style.line {
        deck.style.span('✓ '):fg 'green',
        deck.style.span(message),
      })
      deck.cmd 'reload'
    end
  end

  for _, spec in ipairs(specs) do
    pm.check_update(spec, function(_, _, result) finish_one(spec, result) end)
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
        deck.style.span('   C'):fg 'green',
        deck.style.span(' Check plugin updates'):fg 'gray',
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
  local function build_update_lines(status)
    local lines = {}
    table.insert(lines, deck.style.line {})
    table.insert(
      lines,
      deck.style.line {
        deck.style.span('Updates:'):fg 'magenta',
      }
    )

    if not spec.is_remote then
      table.insert(lines, deck.style.line { deck.style.span('   Local plugin'):fg 'gray' })
      return lines
    end

    if entry.status ~= 'installed' then
      table.insert(lines, deck.style.line { deck.style.span('   Plugin is missing'):fg 'yellow' })
      return lines
    end

    if M._checking_updates then
      table.insert(lines, deck.style.line { deck.style.span('   Checking updates...'):fg 'cyan' })
      return lines
    end

    if not status then
      table.insert(
        lines,
        deck.style.line { deck.style.span('   Not checked. Press C to check all plugins.'):fg 'gray' }
      )
      return lines
    end

    if status.error then
      table.insert(
        lines,
        deck.style.line { deck.style.span('   Check failed: '):fg 'red', deck.style.span(status.error):fg 'gray' }
      )
      return lines
    end

    if not status.has_update then
      table.insert(lines, deck.style.line { deck.style.span('   Up to date'):fg 'green' })
      return lines
    end

    table.insert(
      lines,
      deck.style.line {
        deck.style.span('   New version available '):fg(status.breaking and 'red' or 'yellow'),
        deck.style.span(status.remote_info or status.latest_ref or ''):fg 'gray',
      }
    )

    if status.local_commit and status.remote_commit then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('   Range:  '):fg 'gray',
          deck.style.span(status.local_commit:sub(1, 7)):fg 'white',
          deck.style.span(' → '):fg 'gray',
          deck.style.span(status.remote_commit:sub(1, 7)):fg 'white',
        }
      )
    end

    local breaking_commits = status.breaking_commits or {}
    if #breaking_commits > 0 then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('   Breaking changes:'):fg 'red',
        }
      )
      for _, commit in ipairs(breaking_commits) do
        table.insert(lines, deck.style.line { deck.style.span('     ' .. commit):fg 'red' })
      end
    end

    local commits = status.commits or {}
    if #commits > 0 then
      table.insert(
        lines,
        deck.style.line {
          deck.style.span('   Commits:'):fg 'cyan',
        }
      )
      for i, commit in ipairs(commits) do
        if i > MAX_UPDATE_COMMITS_PREVIEW then
          table.insert(
            lines,
            deck.style.line {
              deck.style.span('     ... and ' .. (#commits - MAX_UPDATE_COMMITS_PREVIEW) .. ' more'):fg 'gray',
            }
          )
          break
        end
        table.insert(lines, deck.style.line { deck.style.span('     ' .. commit):fg 'white' })
      end
    end

    return lines
  end

  cb(build_lines(build_update_lines(M.update_status(spec.name))))
end

-- Attach _manager to the same underlying table that _deck points to.
deck._manager = M
