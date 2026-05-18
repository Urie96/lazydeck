--- Plugin Manager: core logic for installing, updating, and managing plugins
--- Handles GitHub repo parsing, git operations, and lock file management.

local pm = {}

-- Plugin data directory and lock file path
pm.config_dir = os.getenv('HOME') .. '/.config/lazydeck'
pm.data_dir = os.getenv('HOME') .. '/.local/share/lazydeck/plugins'
pm.lock_file = pm.config_dir .. '/plugins.lock'

local git_env = {
  GIT_TERMINAL_PROMPT = '0',
  GCM_INTERACTIVE = 'never',
}

local function explain_git_error(stderr)
  local err = (stderr or ''):trim()
  if err == '' then return 'unknown git error' end

  if err:find('terminal prompts disabled', 1, true)
      or err:find("could not read Username", 1, true)
      or err:find("could not read Password", 1, true)
      or err:find("Username for 'https://github.com'", 1, true)
      or err:find("Password for 'https://", 1, true) then
    return err
      .. ' (GitHub HTTPS clone failed: the repository may not exist, may be private/inaccessible, or local GitHub credentials are not configured; remote plugin install runs in non-interactive mode, so use the correct repo, configure credentials, switch to SSH, or install via local dir)'
  end

  if err:find('Repository not found', 1, true)
      or err:find('repository not found', 1, true)
      or err:find('not found', 1, true) then
    return err .. ' (the repository does not exist or you do not have access)'
  end

  return err
end

local function git(cmd, callback)
  deck.system(cmd, { env = git_env }, callback)
end

local function is_absolute_path(path)
  return path:match '^/' or path:match '^%a:[/\\]'
end

local function resolve_local_dir(dir)
  if type(dir) ~= 'string' or dir == '' then
    error 'plugin dir must be a non-empty string'
  end

  if dir:find '://' then
    error('plugin dir must be a relative or absolute path: ' .. dir)
  end

  if is_absolute_path(dir) then return dir end

  local base_dir = os.getenv('HOME') .. '/.config/lazydeck'
  return base_dir .. '/' .. dir
end

local function plugin_name_from_dir(dir)
  local normalized = dir:gsub('[\\/]+$', '')
  local basename = normalized:match '([^/\\]+)$' or normalized
  return basename:match('^(.+)%.lazydeck$') or basename
end

--- Parse a plugin spec into a normalized structure.
--- Supports four input formats:
---   1. String: 'owner/plugin.lazydeck' or 'local-plugin'
---   2. Table with single string: { 'owner/plugin.lazydeck' }
---   3. Local dir: { dir='plugins/my-plugin.lazydeck' }
---   4. Full table: { 'owner/plugin.lazydeck', branch='main', config=fn }
--- @param spec string|table Plugin declaration
--- @return table|nil Parsed spec with fields: name, repo, branch, tag, commit, config, url, install_path, is_remote, dir
function pm.parse_plugin_spec(spec)
  local source
  local dir

  if type(spec) == 'string' then
    source = spec
  elseif type(spec) == 'table' then
    source = spec[1]
    dir = spec.dir
  else
    return nil
  end

  if not source and not dir then return nil end

  local name
  if dir then
    name = source or plugin_name_from_dir(dir)
  elseif source:find('/') then
    local repo_name = source:match('^[^/]+/(.+)$')
    name = repo_name:match('^(.+)%.lazydeck$') or repo_name
  else
    name = source
  end

  local branch, tag, commit, config_fn, keys
  if type(spec) == 'table' then
    branch = spec.branch
    tag = spec.tag
    commit = spec.commit
    config_fn = spec.config
    keys = spec.keys
    if spec.dependencies ~= nil then
      error("plugin spec no longer supports 'dependencies'; list all plugins directly in deck.config.plugins")
    end
  end

  if not config_fn then
    config_fn = function()
      local ok, mod = pcall(require, name)
      if ok and mod and mod.setup then
        mod.setup()
      end
    end
  end

  local result = {
    name = name,
    branch = branch,
    tag = tag,
    commit = commit,
    config = config_fn,
    keys = keys,
  }

  if dir then
    result.dir = resolve_local_dir(dir)
    result.is_remote = false
  elseif source:find('/') then
    result.repo = source
    result.url = 'https://github.com/' .. source .. '.git'
    result.install_path = pm.data_dir .. '/' .. source:match('^[^/]+/(.+)$')
    result.is_remote = true
  else
    result.is_remote = false
  end

  return result
end

--- Flatten a plugin list into parsed specs (no duplicates, preserves first occurrence order).
--- @param plugins table Array of plugin specs (raw, before parsing)
--- @return table Array of parsed specs
function pm.flatten_plugins(plugins)
  local seen = {}
  local result = {}

  for _, p in ipairs(plugins) do
    local spec = pm.parse_plugin_spec(p)
    if spec and not seen[spec.name] then
      seen[spec.name] = true
      result[#result + 1] = spec
    end
  end

  return result
end

--- Read the lock file.
--- @return table Lock data: { plugin_name = { commit=..., branch=..., ... }, ... }
function pm.read_lock()
  local content, err = deck.fs.read_file_sync(pm.lock_file)
  if err or not content or content == '' then
    return {}
  end
  local ok, data = pcall(deck.json.decode, content)
  if ok and type(data) == 'table' then
    return data
  end
  return {}
end

--- Write lock data to the lock file.
--- @param lock_data table Lock data to write
function pm.write_lock(lock_data)
  deck.fs.mkdir(pm.config_dir)
  local content = deck.json.encode(lock_data)
  deck.fs.write_file_sync(pm.lock_file, content)
end

--- Check if a remote plugin is installed (directory exists).
--- @param spec table Parsed plugin spec
--- @return boolean
function pm.is_installed(spec)
  if not spec.is_remote then return true end
  if not spec.install_path then return false end
  local stat = deck.fs.stat(spec.install_path)
  return stat.exists and stat.is_dir
end

--- Update the lock file entry for a single plugin (async: gets current HEAD).
--- @param spec table Parsed plugin spec
--- @param callback function|nil Called when done
function pm.update_lock_for_plugin(spec, callback)
  git({ 'git', '-C', spec.install_path, 'rev-parse', 'HEAD' }, function(out)
    if out.code == 0 then
      local lock = pm.read_lock()
      lock[spec.name] = {
        repo = spec.repo,
        commit = out.stdout:trim(),
        branch = spec.branch,
        tag = spec.tag,
        url = spec.url,
      }
      pm.write_lock(lock)
    end
    if callback then callback() end
  end)
end

--- Update lock entries for multiple plugins and write the lock file once.
--- @param specs table Array of parsed plugin specs
--- @param callback function|nil Called when done
function pm.update_lock_for_plugins(specs, callback)
  local pending = {}
  for _, spec in ipairs(specs or {}) do
    if spec and spec.is_remote and spec.install_path then
      pending[#pending + 1] = spec
    end
  end

  if #pending == 0 then
    if callback then callback() end
    return
  end

  local lock = pm.read_lock()
  local remaining = #pending

  local function finish_one()
    remaining = remaining - 1
    if remaining == 0 then
      pm.write_lock(lock)
      if callback then callback() end
    end
  end

  for _, spec in ipairs(pending) do
    git({ 'git', '-C', spec.install_path, 'rev-parse', 'HEAD' }, function(out)
      if out.code == 0 then
        lock[spec.name] = {
          repo = spec.repo,
          commit = out.stdout:trim(),
          branch = spec.branch,
          tag = spec.tag,
          url = spec.url,
        }
      end
      finish_one()
    end)
  end
end

--- Install a single plugin via git clone.
--- @param spec table Parsed plugin spec
--- @param callback function|nil Called with (boolean success)
function pm.install(spec, callback)
  if not spec.is_remote then
    if callback then callback(true) end
    return
  end

  deck.fs.mkdir(pm.data_dir)

  local cmd = { 'git', 'clone' }

  if spec.tag then
    -- Clone specific tag
    table.insert(cmd, '--branch')
    table.insert(cmd, spec.tag)
    table.insert(cmd, '--single-branch')
    table.insert(cmd, '--depth')
    table.insert(cmd, '1')
  elseif spec.branch then
    -- Clone specific branch
    table.insert(cmd, '--branch')
    table.insert(cmd, spec.branch)
    table.insert(cmd, '--single-branch')
    table.insert(cmd, '--depth')
    table.insert(cmd, '1')
  elseif not spec.commit then
    -- No constraint: shallow clone default branch
    table.insert(cmd, '--depth')
    table.insert(cmd, '1')
  end

  table.insert(cmd, spec.url)
  table.insert(cmd, spec.install_path)

  git(cmd, function(out)
    if out.code ~= 0 then
      local err = explain_git_error(out.stderr)
      deck.log('error', 'Failed to install {}: {}', spec.name, err)
      deck.notify(deck.style.line({
        deck.style.span('✗ '):fg('red'),
        deck.style.span('Failed to install ' .. spec.name .. ': ' .. err),
      }))
      if callback then callback(false) end
      return
    end

    if spec.commit then
      -- Need full history to checkout a specific commit
      git({ 'git', '-C', spec.install_path, 'fetch', '--unshallow' }, function()
        git({ 'git', '-C', spec.install_path, 'checkout', spec.commit }, function(out2)
          if out2.code ~= 0 then
            deck.log('error', 'Failed to checkout commit {} for {}: {}', spec.commit, spec.name, explain_git_error(out2.stderr))
          end
          if callback then callback(out2.code == 0) end
        end)
      end)
    else
      if callback then callback(true) end
    end
  end)
end

--- Update a single plugin within its constraints.
--- @param spec table Parsed plugin spec
--- @param callback function|nil Called with (boolean success)
function pm.update(spec, callback)
  if not spec.is_remote then
    if callback then callback(false) end
    return
  end

  if spec.commit then
    -- Commit-pinned plugins cannot be updated
    deck.notify(deck.style.line({
      deck.style.span('⊘ '):fg('yellow'),
      deck.style.span(spec.name .. ' is pinned to commit ' .. spec.commit:sub(1, 7)),
    }))
    if callback then callback(false) end
    return
  end

  if not pm.is_installed(spec) then
    -- Not installed yet, install instead
    pm.install(spec, callback)
    return
  end

  local install_path = spec.install_path

  -- git fetch
  git({ 'git', '-C', install_path, 'fetch', '--tags', '--force' }, function(out)
    if out.code ~= 0 then
      local err = explain_git_error(out.stderr)
      deck.log('error', 'Failed to fetch {}: {}', spec.name, err)
      deck.notify(deck.style.line({
        deck.style.span('✗ '):fg('red'),
        deck.style.span('Failed to fetch ' .. spec.name .. ': ' .. err),
      }))
      if callback then callback(false) end
      return
    end

    local function on_done(out2)
      if out2.code ~= 0 then
        local err = explain_git_error(out2.stderr)
        deck.log('error', 'Failed to update {}: {}', spec.name, err)
        deck.notify(deck.style.line({
          deck.style.span('✗ '):fg('red'),
          deck.style.span('Failed to update ' .. spec.name .. ': ' .. err),
        }))
        if callback then callback(false) end
      else
        if callback then callback(true) end
      end
    end

    if spec.tag then
      -- Tag constraint: checkout the tag (tracks the exact commit the tag points to)
      git({ 'git', '-C', install_path, 'checkout', 'tags/' .. spec.tag }, on_done)
    elseif spec.branch then
      -- Branch constraint: reset to the latest of that branch
      git({ 'git', '-C', install_path, 'reset', '--hard', 'origin/' .. spec.branch }, on_done)
    else
      -- No constraint: reset to default remote branch
      -- First, determine the default remote branch
      git({ 'git', '-C', install_path, 'symbolic-ref', 'refs/remotes/origin/HEAD' }, function(ref_out)
        local default_ref = 'origin/HEAD'
        if ref_out.code == 0 then
          -- Extract branch name from refs/remotes/origin/main -> origin/main
          local ref = ref_out.stdout:trim()
          default_ref = ref:gsub('^refs/remotes/', '')
        end
        git({ 'git', '-C', install_path, 'reset', '--hard', default_ref }, on_done)
      end)
    end
  end)
end

--- Restore a plugin to the version recorded in the lock file.
--- @param spec table Parsed plugin spec
--- @param lock_entry table|nil Lock file entry for this plugin
--- @param callback function|nil Called with (boolean success)
function pm.restore(spec, lock_entry, callback)
  if not spec.is_remote then
    if callback then callback(false) end
    return
  end

  if not lock_entry or not lock_entry.commit then
    deck.notify(deck.style.line({
      deck.style.span('⊘ '):fg('yellow'),
      deck.style.span(spec.name .. ': no lock entry found'),
    }))
    if callback then callback(false) end
    return
  end

  local install_path = spec.install_path
  local stat = deck.fs.stat(install_path)

  if not stat.exists then
    -- Clone then checkout to locked commit
    git({ 'git', 'clone', spec.url, install_path }, function(out)
      if out.code == 0 then
        git({ 'git', '-C', install_path, 'checkout', lock_entry.commit }, function(out2)
          if callback then callback(out2.code == 0) end
        end)
      else
        deck.log('error', 'Failed to clone {} for restore: {}', spec.name, explain_git_error(out.stderr))
        if callback then callback(false) end
      end
    end)
  else
    -- Fetch then checkout to locked commit
    git({ 'git', '-C', install_path, 'fetch' }, function()
      git({ 'git', '-C', install_path, 'checkout', lock_entry.commit }, function(out2)
        if out2.code == 0 then
          git({ 'git', '-C', install_path, 'reset', '--hard', lock_entry.commit }, function(out3)
            if callback then callback(out3.code == 0) end
          end)
        else
          if callback then callback(false) end
        end
      end)
    end)
  end
end

--- Check whether a plugin has a newer version available (within constraints).
--- @param spec table Parsed plugin spec
--- @param callback function Called with (boolean has_update, string|nil remote_info)
function pm.check_update(spec, callback)
  if not spec.is_remote or not pm.is_installed(spec) then
    callback(false, nil)
    return
  end

  if spec.commit then
    -- Commit-pinned: never has updates
    callback(false, nil)
    return
  end

  local install_path = spec.install_path

  -- Fetch latest
  git({ 'git', '-C', install_path, 'fetch', '--tags', '--force' }, function(fetch_out)
    if fetch_out.code ~= 0 then
      callback(false, nil)
      return
    end

    -- Get local HEAD
    git({ 'git', '-C', install_path, 'rev-parse', 'HEAD' }, function(local_out)
      if local_out.code ~= 0 then
        callback(false, nil)
        return
      end
      local local_commit = local_out.stdout:trim()

      -- Determine remote ref
      local remote_ref
      if spec.tag then
        remote_ref = 'tags/' .. spec.tag
      elseif spec.branch then
        remote_ref = 'origin/' .. spec.branch
      else
        remote_ref = 'origin/HEAD'
      end

      git({ 'git', '-C', install_path, 'rev-parse', remote_ref }, function(remote_out)
        if remote_out.code ~= 0 then
          callback(false, nil)
          return
        end
        local remote_commit = remote_out.stdout:trim()
        local has_update = local_commit ~= remote_commit

        if has_update then
          -- Get log between local and remote
          git({
            'git', '-C', install_path, 'log',
            '--oneline', '--no-decorate',
            local_commit .. '..' .. remote_commit,
          }, function(log_out)
            local info = remote_commit:sub(1, 7)
            if log_out.code == 0 and log_out.stdout:trim() ~= '' then
              local lines = log_out.stdout:trim():split('\n')
              info = info .. ' (' .. #lines .. ' new commits)'
            end
            callback(true, info)
          end)
        else
          callback(false, nil)
        end
      end)
    end)
  end)
end

--- Get the list of remote plugins from a plugins config list.
--- @param plugins table Array of plugin specs
--- @return table Array of parsed remote plugin specs
function pm.get_remote_plugins(plugins)
  local result = {}
  for _, spec in ipairs(pm.flatten_plugins(plugins or {})) do
    if spec and spec.is_remote then
      table.insert(result, spec)
    end
  end
  return result
end

--- Install a list of parsed plugin specs concurrently, skipping ones already present.
--- @param specs table Array of parsed plugin specs
--- @param callback function|nil Called with (boolean success)
function pm.install_specs(specs, callback)
  local missing = {}
  for _, spec in ipairs(specs or {}) do
    if not pm.is_installed(spec) then
      table.insert(missing, spec)
    end
  end

  if #missing == 0 then
    if callback then callback(true) end
    return
  end

  local all_ok = true
  local completed = 0
  local total = #missing
  local successful = {}

  deck.notify(deck.style.line({
    deck.style.span('⟳ '):fg('cyan'),
    deck.style.span('Installing ' .. total .. ' plugin(s) in parallel...'),
  }))

  local function on_one_done(spec, success)
    completed = completed + 1
    all_ok = all_ok and success
    if success then successful[#successful + 1] = spec end

    if completed >= total then
      pm.update_lock_for_plugins(successful, function()
        local installed = #successful
        local icon = '✓ '
        local color = 'green'
        local message = 'Installed ' .. installed .. '/' .. total .. ' plugin(s)'

        if installed == 0 then
          icon = '✗ '
          color = 'red'
          message = 'Failed to install all ' .. total .. ' plugin(s)'
        elseif installed < total then
          icon = '△ '
          color = 'yellow'
          message = 'Installed ' .. installed .. '/' .. total .. ' plugin(s)'
        end

        deck.notify(deck.style.line({
          deck.style.span(icon):fg(color),
          deck.style.span(message),
        }))
        if callback then callback(all_ok) end
      end)
    end
  end

  for _, spec in ipairs(missing) do
    pm.install(spec, function(success)
      on_one_done(spec, success)
    end)
  end
end

--- Install all missing remote plugins concurrently.
--- @param plugins table Array of plugin spec tables
--- @param callback function|nil Called with (boolean success) when all done
function pm.install_missing(plugins, callback)
  pm.install_specs(pm.get_remote_plugins(plugins), callback)
end

--- Update all remote plugins concurrently.
--- @param plugins table Array of plugin spec tables
--- @param callback function|nil Called when all done
function pm.update_all(plugins, callback)
  local remote = pm.get_remote_plugins(plugins)
  if #remote == 0 then
    if callback then callback() end
    return
  end

  local updated = 0
  local completed = 0
  local total = #remote
  local successful = {}

  deck.notify(deck.style.line({
    deck.style.span('⟳ '):fg('cyan'),
    deck.style.span('Updating ' .. total .. ' plugin(s) in parallel...'),
  }))

  local function on_one_done(spec, success)
    completed = completed + 1
    if success then updated = updated + 1 end
    if success then successful[#successful + 1] = spec end

    if completed >= total then
      pm.update_lock_for_plugins(successful, function()
        deck.notify(deck.style.line({
          deck.style.span('✓ '):fg('green'),
          deck.style.span('Updated ' .. updated .. '/' .. total .. ' plugin(s)'),
        }))
        if callback then callback() end
      end)
    end
  end

  for _, spec in ipairs(remote) do
    pm.update(spec, function(success)
      on_one_done(spec, success)
    end)
  end
end

--- Restore all remote plugins from the lock file (sequentially).
--- @param plugins table Array of plugin spec tables
--- @param callback function|nil Called when all done
function pm.restore_all(plugins, callback)
  local remote = pm.get_remote_plugins(plugins)
  local lock = pm.read_lock()

  if #remote == 0 then
    if callback then callback() end
    return
  end

  local idx = 0
  local restored = 0
  local function restore_next()
    idx = idx + 1
    if idx > #remote then
      deck.notify(deck.style.line({
        deck.style.span('✓ '):fg('green'),
        deck.style.span('Restored ' .. restored .. '/' .. #remote .. ' plugin(s) from lock file'),
      }))
      if callback then callback() end
      return
    end

    local spec = remote[idx]
    local lock_entry = lock[spec.name]
    deck.notify(deck.style.line({
      deck.style.span('⟳ '):fg('cyan'),
      deck.style.span('Restoring ' .. spec.name .. ' (' .. idx .. '/' .. #remote .. ')...'),
    }))
    pm.restore(spec, lock_entry, function(success)
      if success then restored = restored + 1 end
      restore_next()
    end)
  end

  restore_next()
end

-- Attach _pm to the same underlying table that _deck points to.
-- Use the global 'deck' directly since _deck and deck reference the same table.
-- The global 'deck' was registered by Rust's deck::register() via lua.globals().raw_set("_deck", deck).
deck._pm = pm
