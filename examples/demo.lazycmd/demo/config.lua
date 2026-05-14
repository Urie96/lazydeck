local M = {}

local cfg = {
  route_name = 'demo',
  title = 'Demo File Browser',
  root_dir = '.',
  preview_title = 'Preview',
  preview_max_chars = 20000,
  keymap = {
    open_file = 'o',
    reload = 'gr',
  },
}

function M.setup(opt)
  local global_keymap = deck.config.get().keymap or {}
  cfg = deck.tbl_deep_extend('force', cfg, global_keymap, opt or {})
end

function M.get() return cfg end

return M
