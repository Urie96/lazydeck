local M = {}

function M.open_file(entry)
  entry = entry or deck.api.get_hovered()
  if not entry or entry.kind ~= 'file' or not entry.path then return end
  deck.system.open(entry.path)
end

return M
