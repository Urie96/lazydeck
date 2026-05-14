local M = {}

function M.list(path, cb)
  cb {
    {
      key = 'alpha',
      display = ('Alpha Task'):fg 'cyan',
      keymap = {
        ['<right>'] = {
          callback = function() deck.notify 'This demo keeps everything on /minimal' end,
          desc = 'stay on current page',
        },
      },
      preview = function() return ('A fixed entry with its own local keymaps and inline preview.'):fg 'cyan' end,
    },
    {
      key = 'beta',
      display = ('Beta Job'):fg 'magenta',
      keymap = {
        ['<right>'] = {
          callback = function() deck.notify 'This demo keeps everything on /minimal' end,
          desc = 'stay on current page',
        },
      },
      preview = function()
        return ('Each entry defines preview directly on itself, without metatable injection.'):fg 'magenta'
      end,
    },
    {
      key = 'gamma',
      display = ('Gamma Note'):fg 'yellow',
      keymap = {
        ['<right>'] = {
          callback = function() deck.notify 'This demo keeps everything on /minimal' end,
          desc = 'stay on current page',
        },
      },
      preview = function()
        return ('Useful as the smallest possible plugin example: only init.lua and list().'):fg 'yellow'
      end,
    },
  }
end

return M
