local cfg = {
  keymap = {
    up = '<up>',
    down = '<down>',
    top = 'gg',
    bottom = 'G',
    preview_up = '<pageup>',
    preview_down = '<pagedown>',
    reload = '<C-r>',
    history_back = '<C-o>',
    history_forward = '<C-i>',
    tab_new = 'tn',
    tab_close = 'tc',
    tab_next = 'gt',
    tab_prev = 'gT',
    quit = 'q',
    force_quit = '<C-q>',
    filter = '/',
    command_prompt = ':',
    clear_filter = '<esc>',
    back = '<left>',
    open = '<right>',
    enter = '<enter>',
    help = '?',
    delete = 'dd',
    new = 'n',
    append = 'a',
    input_submit = '<enter>',
    input_cancel = '<esc>',
    input_clear_before_cursor = '<C-u>',
    input_cursor_to_start = '<C-a>',
    input_cursor_to_end = '<C-e>',
    input_external_editor = '<C-g>',
  },
  image = {
    max_width = 40,
    max_height = 10,
  },
  plugin_sort = 'defined',
}

local function append_package_path(paths, path, seen)
  if path and path ~= '' and not seen[path] then
    seen[path] = true
    table.insert(paths, path)
  end
end

local function add_config_base_path()
  local package = require 'package'
  local base_dir = os.getenv 'LAZYDECK_CONFIG_BASE_DIR'
    or (os.getenv 'HOME' .. '/.config/lazydeck')

  local paths = { package.path }
  local seen = {}
  append_package_path(paths, base_dir .. '/?.lua', seen)
  append_package_path(paths, base_dir .. '/?/init.lua', seen)
  package.path = table.concat(paths, ';')
end

local function add_plugin_paths(plugins)
  local package = require 'package'
  local paths = { package.path }
  local seen = {}

  for _, p in ipairs(deck._pm.flatten_plugins(plugins or {})) do
    if p.dir and not seen[p.dir] then
      append_package_path(paths, p.dir .. '/?.lua', seen)
      append_package_path(paths, p.dir .. '/?/init.lua', seen)
    elseif p.is_remote and p.install_path then
      append_package_path(paths, p.install_path .. '/?.lua', seen)
      append_package_path(paths, p.install_path .. '/?/init.lua', seen)
    end
  end

  package.path = table.concat(paths, ';')
end

add_config_base_path()

local runtime = {
  explicit_plugin_specs = {},
  plugin_specs_by_name = {},
  loaded_plugins = {},
  configured_plugins = {},
}

local function rebuild_plugin_index()
  runtime.explicit_plugin_specs = {}
  runtime.plugin_specs_by_name = {}
  runtime.loaded_plugins = {}
  runtime.configured_plugins = {}

  for _, plugin_spec in ipairs(cfg.plugins or {}) do
    local spec = deck._pm.parse_plugin_spec(plugin_spec)
    if spec then
      table.insert(runtime.explicit_plugin_specs, spec)
      runtime.plugin_specs_by_name[spec.name] = spec
    end
  end
end

local PLUGIN_META_CACHE_NS = 'lazydeck.plugin.meta'
local PLUGIN_USAGE_CACHE_NS = 'lazydeck.plugin.usage'
local DEFAULT_PLUGIN_ICON = '󰏗'

local function plugin_status(spec)
  if spec.is_remote and not deck._pm.is_installed(spec) then return 'missing' end
  return 'installed'
end

local function normalize_plugin_meta(meta)
  if type(meta) ~= 'table' then meta = {} end
  local icon = meta.icon
  local desc = meta.desc
  local color = meta.color or 'cyan'
  if type(icon) ~= 'string' or icon == '' then icon = DEFAULT_PLUGIN_ICON end
  if type(desc) ~= 'string' then desc = '' end
  if type(color) ~= 'string' or color == '' then color = 'cyan' end
  return {
    icon = icon,
    desc = desc,
    color = color,
  }
end

local function collect_plugin_meta(plugin)
  if type(plugin.meta) ~= 'function' then return normalize_plugin_meta({}) end

  local ok, meta = pcall(plugin.meta)
  if not ok then
    deck.log('error', 'Failed to load plugin meta: {}', tostring(meta))
    return normalize_plugin_meta({})
  end

  return normalize_plugin_meta(meta)
end

local function cache_plugin_meta(name, plugin)
  local meta = collect_plugin_meta(plugin)
  deck.cache.set(PLUGIN_META_CACHE_NS, name, meta)
  return meta
end

local function load_plugin(name)
  local spec = runtime.plugin_specs_by_name[name]
  if not spec then return nil, 'Unknown plugin: ' .. tostring(name) end

  if runtime.loaded_plugins[name] == nil then
    local ok, plugin = pcall(require, name)
    if not ok then return nil, plugin end
    runtime.loaded_plugins[name] = plugin or {}
  end

  return runtime.loaded_plugins[name], spec
end

local function ensure_plugin_meta(name)
  local meta = deck.cache.get(PLUGIN_META_CACHE_NS, name)
  if type(meta) == 'table' then return normalize_plugin_meta(meta) end

  local plugin = load_plugin(name)
  if not plugin then return normalize_plugin_meta({}) end

  return cache_plugin_meta(name, plugin)
end

local function plugin_cached_meta(name)
  return ensure_plugin_meta(name)
end

local function normalize_plugin_usage(usage)
  if type(usage) ~= 'table' then usage = {} end
  local count = tonumber(usage.count) or 0
  local last_used = tonumber(usage.last_used) or 0
  return {
    count = count,
    last_used = last_used,
  }
end

local function plugin_usage(name)
  return normalize_plugin_usage(deck.cache.get(PLUGIN_USAGE_CACHE_NS, name))
end

local function touch_plugin_usage(name)
  local usage = plugin_usage(name)
  usage.count = usage.count + 1
  usage.last_used = deck.time.now()
  deck.cache.set(PLUGIN_USAGE_CACHE_NS, name, usage)
  return usage
end

local function plugin_sort_mode()
  local mode = cfg.plugin_sort or cfg.root_plugin_sort or cfg.root_sort or 'defined'
  if mode == 'recent' or mode == 'recently_used' or mode == 'last_used' then return 'recent' end
  if mode == 'most' or mode == 'most_used' or mode == 'usage' or mode == 'count' then return 'most' end
  return 'defined'
end

local function sorted_root_plugin_specs()
  local items = {}
  for index, spec in ipairs(runtime.explicit_plugin_specs) do
    table.insert(items, {
      spec = spec,
      index = index,
      usage = plugin_usage(spec.name),
      startup = spec.lazy == false,
    })
  end

  local mode = plugin_sort_mode()
  table.sort(items, function(a, b)
    if mode ~= 'defined' then
      -- startup plugins (lazy=false) are already loaded in the background;
      -- keep them after interactive/lazy plugins when sorting by usage.
      if a.startup ~= b.startup then return not a.startup end
    end

    if mode == 'recent' and a.usage.last_used ~= b.usage.last_used then
      return a.usage.last_used > b.usage.last_used
    end

    if mode == 'most' then
      if a.usage.count ~= b.usage.count then return a.usage.count > b.usage.count end
      if a.usage.last_used ~= b.usage.last_used then return a.usage.last_used > b.usage.last_used end
    end

    return a.index < b.index
  end)

  local result = {}
  for _, item in ipairs(items) do
    table.insert(result, item.spec)
  end
  return result
end

local function plugin_display(spec, meta)
  local status = plugin_status(spec)
  local name_color = status == 'missing' and 'yellow' or 'white'
  local parts = {
    deck.style.span(meta.icon .. ' '):fg(meta.color),
    deck.style.span(spec.name):fg(name_color):bold(),
  }

  local badge = ''
  local badge_color = 'yellow'
  if deck._manager and deck._manager.update_badge then
    badge, badge_color = deck._manager.update_badge(spec.name)
  end
  table.insert(parts, deck.style.span((badge and badge ~= '') and (' ' .. badge) or ''):fg(badge_color or 'yellow'))

  table.insert(parts, ' ')
  table.insert(parts, deck.style.span(meta.desc):fg 'DarkGray')
  return deck.style.line(parts)
end

local function list_root_plugins(cb)
  local entries = {}
  local lines = {}
  for _, spec in ipairs(sorted_root_plugin_specs()) do
    local meta = plugin_cached_meta(spec.name)
    local usage = plugin_usage(spec.name)
    local display = plugin_display(spec, meta)
    table.insert(lines, display)
    table.insert(entries, {
      key = spec.name,
      repo = spec.repo,
      url = spec.url,
      dir = spec.dir,
      is_remote = spec.is_remote,
      status = plugin_status(spec),
      icon = meta.icon,
      desc = meta.desc,
      color = meta.color,
      usage_count = usage.count,
      last_used = usage.last_used,
      display = display,
      bottom_line = meta.desc ~= '' and meta.desc or nil,
      preview = function(self, preview_cb) deck._manager.preview(self, preview_cb) end,
    })
  end
  deck.style.align_columns(lines)
  cb(entries)
end

local function ensure_plugin(name, opts)
  local plugin, spec_or_err = load_plugin(name)
  if not plugin then return nil, spec_or_err end
  local spec = spec_or_err

  opts = opts or {}
  if not runtime.configured_plugins[name] then
    local ok_config, config_err = pcall(spec.config)
    if not ok_config then return nil, config_err end
    cache_plugin_meta(name, plugin)
    runtime.configured_plugins[name] = true
    if opts.record_usage then touch_plugin_usage(name) end
  end

  return runtime.loaded_plugins[name]
end

local function setup_plugin(name) return ensure_plugin(name, { record_usage = true }) end

local function guarded_preview_callback(hovered_path)
  return function(preview) deck.api.set_preview(hovered_path, preview) end
end

local function open_help()
  local options = {}
  local lines = {}

  for _, item in ipairs(deck.api.get_available_keymaps()) do
    local source = item.source == 'entry' and '[entry]' or item.source == 'page' and '[page]' or '[global]'
    local desc = item.desc or 'no description'
    local line = deck.style.line {
      deck.style.span(item.key):fg 'yellow',
      '  ',
      deck.style.span(source):fg(item.source == 'entry' and 'cyan' or item.source == 'page' and 'green' or 'blue'),
      '  ',
      deck.style.span(desc):fg 'white',
    }
    table.insert(lines, line)
    table.insert(options, {
      value = item,
      display = line,
    })
  end

  deck.style.align_columns(lines)

  deck.select({ prompt = 'Available Keymaps', options = options }, function(choice)
    if choice and choice.callback then choice.callback() end
  end)
end

local function select_action()
  local options = {}
  local lines = {}

  for _, item in ipairs(deck.api.get_available_keymaps()) do
    if item.source == 'entry' and item.desc ~= 'Select action' then
      local line = deck.style.line {
        deck.style.span(item.key):fg 'yellow',
        '  ',
        deck.style.span(item.desc or 'no description'):fg 'white',
      }
      table.insert(lines, line)
      table.insert(options, {
        value = item,
        display = line,
      })
    end
  end

  if #options == 0 then return end
  deck.style.align_columns(lines)
  deck.select({ prompt = 'Actions', options = options }, function(choice)
    if choice and choice.callback then choice.callback() end
  end)
end

local function open_filter()
  deck.input {
    prompt = 'Filter:',
    placeholder = '输入筛选内容...',
    value = deck.api.get_filter(),
    on_change = function(input) deck.api.set_filter(input) end,
    on_submit = function(input) deck.api.set_filter(input) end,
    on_cancel = function() deck.api.set_filter '' end,
  }
end

local function edit_current_input_in_external_editor()
  local current = deck.input.get()
  if current == nil then return end

  deck.system.edit({ content = current }, function(content, err)
    if err then deck.notify(err) end
    if content ~= nil then deck.input.set(content:gsub('\r?\n$', '')) end
  end)
end

local function apply_configured_keymap()
  local map = function(key, cb, desc) deck.keymap.set('main', key, cb, { desc = desc }) end
  local map_input = function(key, cb, desc) deck.keymap.set('input', key, cb, { desc = desc }) end
  map(cfg.keymap.up, 'scroll_by -1', 'move up')
  map(cfg.keymap.down, 'scroll_by 1', 'move down')
  map(cfg.keymap.top, 'scroll_by -9999', 'go to top')
  map(cfg.keymap.bottom, 'scroll_by 9999', 'go to bottom')
  map(cfg.keymap.preview_up, 'scroll_preview_by -30', 'scroll preview up')
  map(cfg.keymap.preview_down, 'scroll_preview_by 30', 'scroll preview down')
  map(cfg.keymap.reload, 'reload', 'reload')
  map(cfg.keymap.history_back, 'history_back', 'history back')
  map(cfg.keymap.history_forward, 'history_forward', 'history forward')
  map(cfg.keymap.tab_new, 'tab_new', 'new tab')
  map(cfg.keymap.tab_close, 'tab_close', 'close tab')
  map(cfg.keymap.tab_next, 'tab_next', 'next tab')
  map(cfg.keymap.tab_prev, 'tab_prev', 'previous tab')
  map(cfg.keymap.quit, 'quit', 'quit')
  map(cfg.keymap.force_quit, 'quit', 'force quit')
  map(cfg.keymap.command_prompt, 'command_prompt', 'command prompt')
  map(cfg.keymap.filter, open_filter, 'filter')
  map(cfg.keymap.clear_filter, function() deck.api.set_filter '' end, 'clear filter')
  map(cfg.keymap.back, 'back', 'back')
  map(cfg.keymap.open, 'enter', 'open')
  map(cfg.keymap.help, open_help, 'help')
  map(cfg.keymap.enter, select_action, 'Select action')
  map('gr', function() deck.api.go_to {} end, 'go to /')

  map_input(cfg.keymap.input_submit, 'input_submit', 'submit input')
  map_input(cfg.keymap.input_cancel, 'input_cancel', 'cancel input')
  map_input(cfg.keymap.input_clear_before_cursor, 'input_clear_before_cursor', 'delete text before cursor')
  map_input(cfg.keymap.input_cursor_to_start, 'input_cursor_to_start', 'move cursor to start')
  map_input(cfg.keymap.input_cursor_to_end, 'input_cursor_to_end', 'move cursor to end')
  map_input(cfg.keymap.input_external_editor, edit_current_input_in_external_editor, 'edit input in external editor')
end

local function load_startup_plugins()
  for _, spec in ipairs(runtime.explicit_plugin_specs or {}) do
    if spec.lazy == false then
      local _, err = ensure_plugin(spec.name, { record_usage = true })
      if err then
        deck.log('error', 'Failed to load startup plugin {}: {}', spec.name, tostring(err))
        deck.notify(tostring(err))
      end
    end
  end
end

local function register_plugin_keymaps()
  for _, spec in ipairs(runtime.explicit_plugin_specs or {}) do
    local keys = spec.keys
    if type(keys) == 'table' then
      for _, item in ipairs(keys) do
        if type(item) == 'table' then
          local key = item[1]
          local callback = item[2]
          local desc = item.desc
          local path = item.path

          if type(key) == 'string' and type(callback) == 'function' then
            local plugin_name = spec.name
            local key_name = key
            local action_callback = callback
            local action_desc = desc
            local action_path = path or { plugin_name, '**' }
            deck.keymap.set('main', key_name, function()
              local plugin, err = deck.plugin.load(plugin_name)
              if not plugin then
                deck.notify(err)
                return
              end

              local ok, call_err = pcall(action_callback)
              if not ok then
                deck.notify(call_err)
              end
            end, {
              path = action_path,
              desc = action_desc or (plugin_name .. ' · ' .. key_name),
            })
          end
        end
      end
    end
  end
end

local config = {}

function config.get() return cfg end

function config.setup(opt)
  cfg = deck.tbl_deep_extend('force', cfg, opt or {})
  rebuild_plugin_index()
  add_plugin_paths(cfg.plugins)
  apply_configured_keymap()
  register_plugin_keymaps()
  deck._manager.setup(cfg.plugins)
  deck._pm.install_missing(cfg.plugins, function()
    load_startup_plugins()
    deck.cmd 'reload'
  end)

  function deck._list()
    local path = deck.api.get_current_path()
    if #path == 0 then
      list_root_plugins(function(entries)
        if deck.deep_equal(path, deck.api.get_current_path()) then deck.api.set_entries(nil, entries) end
      end)
      return
    end

    local plugin, err = ensure_plugin(path[1], { record_usage = true })
    if not plugin then
      deck.notify(tostring(err))
      deck.api.set_entries(nil, {})
      return
    end

    if plugin.list then
      plugin.list(path, function(entries)
        if deck.deep_equal(path, deck.api.get_current_path()) then deck.api.set_entries(nil, entries) end
      end)
    end
  end

  function deck._preview()
    local entry = deck.api.get_hovered()
    local path = deck.api.get_hovered_path()
    if not entry then return end

    local cb = guarded_preview_callback(path)
    if type(entry.preview) == 'function' then
      local ok, preview_text = pcall(function() return entry:preview(cb) end)
      if not ok then
        deck.log('error', 'Failed to render entry preview: {}', tostring(preview_text))
        cb(tostring(preview_text))
        return
      end
      if preview_text then cb(preview_text) end
      return
    end

    local current_path = deck.api.get_current_path()
    if #current_path == 0 then
      cb ''
      return
    end

    local plugin = ensure_plugin(current_path[1])
    if plugin and plugin.preview then
      local ok, err = pcall(plugin.preview, entry, cb)
      if not ok then
        deck.log('error', 'Failed to render plugin preview for {}: {}', tostring(current_path[1]), tostring(err))
        cb(tostring(err))
      end
    end
  end
end

deck.config = config
deck.plugin = deck.plugin or {}

function deck.plugin.load(name) return setup_plugin(name) end

-- Set metatable on deck.system to handle multiple argument formats
setmetatable(deck.config, {
  __call = function(self, opt) deck.config.setup(opt) end,
})

local config_file = os.getenv 'LAZYDECK_CONFIG_FILE'
if config_file and config_file ~= '' then
  local chunk, err = loadfile(config_file)
  if not chunk then error(err) end
  chunk()
else
  require 'init'
end
