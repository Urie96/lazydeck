#[cfg(test)]
mod tests {
    use mlua::Lua;

    /// Inline Lua implementation of parse_plugin_spec and flatten_plugins for testing.
    /// Must be kept in sync with preset/lua/plugin_manager.lua
    const TEST_LUA: &str = r#"
local data_dir = os.getenv('HOME') .. '/.local/share/lazydeck/plugins'
local __lazydeck_config_base_dir = __lazydeck_test_tmpdir .. '/lazydeck-tests'

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

  local base_dir = rawget(_G, '__lazydeck_config_base_dir') or '.'
  return base_dir .. '/' .. dir
end

local function plugin_name_from_dir(dir)
  local normalized = dir:gsub('[\\/]+$', '')
  local basename = normalized:match '([^/\\]+)$' or normalized
  return basename:match('^(.+)%.lazydeck$') or basename
end

local function parse_plugin_spec(spec)
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

  local branch, tag, commit, config_fn
  if type(spec) == 'table' then
    branch = spec.branch
    tag = spec.tag
    commit = spec.commit
    config_fn = spec.config
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
  }

  if dir then
    result.dir = resolve_local_dir(dir)
    result.is_remote = false
  elseif source:find('/') then
    result.repo = source
    result.url = 'https://github.com/' .. source .. '.git'
    result.install_path = data_dir .. '/' .. source:match('^[^/]+/(.+)$')
    result.is_remote = true
  else
    result.is_remote = false
  end

  return result
end

local function flatten_plugins(plugins)
  local seen = {}
  local result = {}

  for _, p in ipairs(plugins or {}) do
    local spec = parse_plugin_spec(p)
    if spec and not seen[spec.name] then
      seen[spec.name] = true
      result[#result + 1] = spec
    end
  end

  return result
end

local function get_remote_plugins(plugins)
  local result = {}
  for _, spec in ipairs(flatten_plugins(plugins or {})) do
    if spec and spec.is_remote then
      result[#result + 1] = spec
    end
  end
  return result
end

return { parse = parse_plugin_spec, flatten = flatten_plugins, remotes = get_remote_plugins }
"#;

    fn load_test_module(lua: &Lua) -> mlua::Result<(mlua::Function, mlua::Function, mlua::Function)> {
        lua.globals().set(
            "__lazydeck_test_tmpdir",
            std::env::temp_dir().to_string_lossy().to_string(),
        )?;
        let module: mlua::Table = lua.load(TEST_LUA).eval()?;
        let parse: mlua::Function = module.get("parse")?;
        let flatten: mlua::Function = module.get("flatten")?;
        let remotes: mlua::Function = module.get("remotes")?;
        Ok((parse, flatten, remotes))
    }

    #[test]
    fn test_parse_string_input() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        // Test: simple string input
        let result: mlua::Table = parse.call("owner/my-plugin.lazydeck")?;
        let name: String = result.get("name")?;
        assert_eq!(name, "my-plugin");

        Ok(())
    }

    #[test]
    fn test_parse_table_with_string() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        // Test: table with single string
        let spec = lua.create_table()?;
        spec.set(1, "owner/my-plugin.lazydeck")?;
        let result: mlua::Table = parse.call(spec)?;
        let name: String = result.get("name")?;
        assert_eq!(name, "my-plugin");

        Ok(())
    }

    #[test]
    fn test_parse_github_repo() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "owner/my-plugin.lazydeck")?;
        spec.set("branch", "main")?;
        let result: mlua::Table = parse.call(spec)?;

        let name: String = result.get("name")?;
        let repo: String = result.get("repo")?;
        let branch: String = result.get("branch")?;
        let is_remote: bool = result.get("is_remote")?;
        let url: String = result.get("url")?;

        assert_eq!(name, "my-plugin");
        assert_eq!(repo, "owner/my-plugin.lazydeck");
        assert_eq!(branch, "main");
        assert!(is_remote);
        assert!(url.contains("github.com"));

        Ok(())
    }

    #[test]
    fn test_parse_github_repo_without_suffix() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "owner/plain-repo")?;
        let result: mlua::Table = parse.call(spec)?;

        let name: String = result.get("name")?;
        let is_remote: bool = result.get("is_remote")?;
        assert_eq!(name, "plain-repo");
        assert!(is_remote);

        Ok(())
    }

    #[test]
    fn test_parse_local_plugin() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "process")?;
        let result: mlua::Table = parse.call(spec)?;

        let name: String = result.get("name")?;
        let is_remote: bool = result.get("is_remote")?;
        let url: mlua::Value = result.get("url")?;

        assert_eq!(name, "process");
        assert!(!is_remote);
        assert!(url.is_nil());

        Ok(())
    }

    #[test]
    fn test_parse_local_plugin_with_dir() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set("dir", "plugins/my-local.lazydeck")?;
        let result: mlua::Table = parse.call(spec)?;

        let name: String = result.get("name")?;
        let dir: String = result.get("dir")?;
        let is_remote: bool = result.get("is_remote")?;

        assert_eq!(name, "my-local");
        let expected_dir = std::env::temp_dir()
            .join("lazydeck-tests/plugins/my-local.lazydeck")
            .to_string_lossy()
            .to_string();
        assert_eq!(dir, expected_dir);
        assert!(!is_remote);

        Ok(())
    }

    #[test]
    fn test_parse_local_plugin_with_dir_and_name() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "myplugin")?;
        spec.set("dir", "/opt/plugins/custom.lazydeck")?;
        let result: mlua::Table = parse.call(spec)?;

        let name: String = result.get("name")?;
        let dir: String = result.get("dir")?;
        assert_eq!(name, "myplugin");
        assert_eq!(dir, "/opt/plugins/custom.lazydeck");

        Ok(())
    }

    #[test]
    fn test_parse_with_tag() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "owner/versioned.lazydeck")?;
        spec.set("tag", "1.0.0")?;
        let result: mlua::Table = parse.call(spec)?;

        let tag: String = result.get("tag")?;
        assert_eq!(tag, "1.0.0");

        Ok(())
    }

    #[test]
    fn test_parse_with_commit() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "owner/pinned.lazydeck")?;
        spec.set("commit", "abc1234567890def")?;
        let result: mlua::Table = parse.call(spec)?;

        let commit: String = result.get("commit")?;
        assert_eq!(commit, "abc1234567890def");

        Ok(())
    }

    #[test]
    fn test_parse_nil_source() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        let result: mlua::Value = parse.call(spec)?;
        assert!(result.is_nil());

        Ok(())
    }

    #[test]
    fn test_parse_with_dependencies_errors() -> mlua::Result<()> {
        let lua = Lua::new();
        let (parse, _, _) = load_test_module(&lua)?;

        let spec = lua.create_table()?;
        spec.set(1, "owner/plugin.lazydeck")?;
        let deps = lua.create_table()?;
        deps.set(1, "owner/dep1.lazydeck")?;
        deps.set(2, "owner/dep2.lazydeck")?;
        spec.set("dependencies", deps)?;
        let err = parse.call::<mlua::Value>(spec).unwrap_err();
        assert!(err.to_string().contains("no longer supports 'dependencies'"));

        Ok(())
    }

    #[test]
    fn test_flatten_plugins() -> mlua::Result<()> {
        let lua = Lua::new();
        let (_, flatten, _) = load_test_module(&lua)?;

        let plugins = lua.create_table()?;
        plugins.set(1, "owner/main.lazydeck")?;
        plugins.set(2, "owner/other.lazydeck")?;

        let result: Vec<mlua::Table> = flatten.call(plugins)?;

        assert_eq!(result.len(), 2);
        let name0: String = result[0].get("name")?;
        let name1: String = result[1].get("name")?;
        assert_eq!(name0, "main");
        assert_eq!(name1, "other");

        Ok(())
    }

    #[test]
    fn test_flatten_no_duplicates() -> mlua::Result<()> {
        let lua = Lua::new();
        let (_, flatten, _) = load_test_module(&lua)?;

        let plugins = lua.create_table()?;

        plugins.set(1, "owner/shared.lazydeck")?;
        plugins.set(2, "owner/p1.lazydeck")?;
        plugins.set(3, "owner/shared.lazydeck")?;
        plugins.set(4, "owner/p2.lazydeck")?;

        let result: Vec<mlua::Table> = flatten.call(plugins)?;

        // Should have 3 unique plugins, preserving first occurrence order
        assert_eq!(result.len(), 3);

        let name0: String = result[0].get("name")?;
        let name1: String = result[1].get("name")?;
        let name2: String = result[2].get("name")?;
        assert_eq!(name0, "shared");
        assert_eq!(name1, "p1");
        assert_eq!(name2, "p2");

        Ok(())
    }

    #[test]
    fn test_get_remote_plugins_flattens_and_filters() -> mlua::Result<()> {
        let lua = Lua::new();
        let (_, _, remotes) = load_test_module(&lua)?;

        let plugins = lua.create_table()?;
        plugins.set(1, "owner/main.lazydeck")?;
        plugins.set(2, "local-helper")?;
        plugins.set(3, "owner/dep.lazydeck")?;

        let result: Vec<mlua::Table> = remotes.call(plugins)?;
        assert_eq!(result.len(), 2);

        let main_name: String = result[0].get("name")?;
        let dep_name: String = result[1].get("name")?;
        assert_eq!(main_name, "main");
        assert_eq!(dep_name, "dep");

        Ok(())
    }
}
