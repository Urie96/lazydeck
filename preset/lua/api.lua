---@class deck.api
local api = {}
local preview_runtime = {
  pending_image_downloads = {},
  failed_image_downloads = {},
}
local preview_image_cache_dir = os.getenv 'HOME' .. '/.cache/lazydeck/preview-images'

---@class PageEntry
---@field key string The unique key for the entry
---@field display? string|Span|Line The display text or styled line widget shown in the list
---@field bottom_line? string|Span|Line Extra line rendered at the bottom-left when this entry is hovered
---@field keymap? table<string, fun()|{callback: fun(), desc?: string}> Entry-local keymap table, resolved from the entry/metatable and preferred over global keymaps when matched
---@field preview? fun(self: PageEntry, cb: fun(widget: string|Span|Text|Line|Image|(string|Span|Text|Line|Image)[])) Entry-local preview callback, preferred over plugin.preview when present
---@field [string] any Additional custom fields

---Set the entries for a page
---@param path string[]|nil The page path, or nil for the current page
---@param entries PageEntry[]|nil The list of page entries, or nil to clear the page
function api.set_entries(path, entries) return _deck.api.set_entries(path, entries) end

---Get the currently hovered entry
---@return PageEntry? entry The hovered entry or nil
function api.get_hovered() return _deck.api.get_hovered() end

---Get page-level selected entries; falls back to the hovered entry when none are selected
---@return PageEntry[] entries Selected entries, or {hovered_entry}, or an empty list
function api.get_selected() return _deck.api.get_selected() end

---Toggle the hovered entry in the current page selection and move down by one entry
function api.toggle_selected() return _deck.api.toggle_selected() end

---Clear current page selection
function api.clear_selected() return _deck.api.clear_selected() end

---Set hovered entry by full path
---@param path string[] The full path including the entry key
function api.set_hovered(path) return _deck.api.set_hovered(path) end

---Get the full entry list for a page before filtering
---@param path string[]|nil The page path, or nil for the current page
---@return PageEntry[]|nil entries The page entries
function api.get_entries(path) return _deck.api.get_entries(path) end

---Set the preview panel content
---@param path string[]|nil The hovered entry path, or nil for the current hovered entry
---@param widget string|Span|Text|Line|Image|(string|Span|Text|Line|Image)[]|nil The widget to display, or nil to clear the preview
local function is_image_widget(value)
  return type(value) == 'table' and value.__deck_type == 'image' and type(value.source) == 'string'
end

local function is_remote_image_source(source)
  return type(source) == 'string' and (source:match '^https://' or source:match '^http://')
end

local function image_cache_key(url)
  local ext = url:match('%.([%w]+)[^./]*$') or ''
  ext = ext:lower()
  if ext == 'jpg' then ext = 'jpeg' end
  local supported = { png = true, jpeg = true, gif = true, webp = true, bmp = true, tiff = true, tga = true }
  if not supported[ext] then ext = '' end
  return deck.base64.encode(url):gsub('[+/=]', '_') .. (ext ~= '' and '.' .. ext or '')
end

local function cached_image_path(url)
  return preview_image_cache_dir .. '/' .. image_cache_key(url)
end

local function placeholder_for_image(message)
  return deck.style.text {
    deck.style.line {
      deck.style.span(message or 'Loading image...'):fg 'dark_gray',
    },
  }
end

local function fetch_remote_image(url)
  local key = image_cache_key(url)
  local existing_path = cached_image_path(url)
  local existing_stat = deck.fs.stat(existing_path)
  if existing_stat and existing_stat.exists and existing_stat.is_file then
    return Promise.resolve(existing_path)
  end

  local failed = preview_runtime.failed_image_downloads[key]
  if failed then return Promise.reject(failed) end

  if preview_runtime.pending_image_downloads[key] then
    return preview_runtime.pending_image_downloads[key]
  end

  local promise = Promise.new(function(resolve, reject)
    deck.http.get(url, function(response)
      preview_runtime.pending_image_downloads[key] = nil

      if not response.success or response.status < 200 or response.status >= 300 then
        local reason = response.error or ('request failed with status ' .. tostring(response.status))
        preview_runtime.failed_image_downloads[key] = reason
        reject(reason)
        return
      end

      local path = cached_image_path(url)
      local ok_mkdir, mkdir_err = deck.fs.mkdir(preview_image_cache_dir)
      if not ok_mkdir then
        preview_runtime.failed_image_downloads[key] = mkdir_err
        reject(mkdir_err)
        return
      end

      local ok_write, write_err = deck.fs.write_file_sync(path, response.body)
      if not ok_write then
        preview_runtime.failed_image_downloads[key] = write_err
        reject(write_err)
        return
      end

      preview_runtime.failed_image_downloads[key] = nil
      resolve(path)
    end)
  end)

  preview_runtime.pending_image_downloads[key] = promise
  return promise
end

local function normalize_preview_value(value, pending_downloads)
  if is_image_widget(value) then
    if not is_remote_image_source(value.source) then return value end

    local key = image_cache_key(value.source)
    local cached_path = cached_image_path(value.source)
    local stat = deck.fs.stat(cached_path)
    if stat and stat.exists and stat.is_file then
      return deck.style.image(cached_path, {
        max_width = value.max_width,
        max_height = value.max_height,
      })
    end

    if preview_runtime.failed_image_downloads[key] then
      return placeholder_for_image 'Failed to load image'
    end

    table.insert(pending_downloads, fetch_remote_image(value.source))
    return placeholder_for_image()
  end

  if type(value) ~= 'table' then return value end

  local normalized = {}
  for i, item in ipairs(value) do
    normalized[i] = normalize_preview_value(item, pending_downloads)
  end
  return normalized
end

local function resolve_preview_images(preview)
  if preview == nil then return nil, {} end

  local pending_downloads = {}
  local normalized = normalize_preview_value(preview, pending_downloads)
  return normalized, pending_downloads
end

function api.set_preview(path, widget)
  local target_path = path or api.get_hovered_path()
  local normalized, pending_downloads = resolve_preview_images(widget)
  _deck.api.set_preview(target_path, normalized)

  if #pending_downloads == 0 then return end

  local function notify_preview_error(prefix, err)
    deck.notify(prefix .. ': ' .. tostring(err or 'unknown error'))
  end

  Promise.allSettled(pending_downloads):next(function(results)
    local first_error = nil
    for _, result in ipairs(results or {}) do
      if result.status == 'rejected' then
        first_error = result.reason
        break
      end
    end

    local refreshed = resolve_preview_images(widget)
    _deck.api.set_preview(target_path, refreshed)

    if first_error ~= nil then
      notify_preview_error('Failed to load image', first_error)
    end
  end):catch(function(err)
    notify_preview_error('Image preview error', err)
  end)
end

---Navigate to a specific path
---@param path string[] The path as an array of strings
function api.go_to(path) return _deck.api.go_to(path) end

---Clear the cached page for a specific path so the next navigation reloads it
---@param path string[] The path as an array of strings
function api.clear_page_cache(path) return _deck.api.clear_page_cache(path) end

---Get the current navigation path
---@return string[] path The current path
function api.get_current_path() return _deck.api.get_current_path() end

---Get the full path of the currently hovered entry
---@return string[]|nil path The full path or nil
function api.get_hovered_path() return _deck.api.get_hovered_path() end

---Get command line arguments
---@return string[] args Command line arguments (first element is program name)
function api.argv() return _deck.api.argv() end

---Set the filter string for the current page
---The page entries will be filtered based on this string
---If empty string, no filter is applied (show all entries)
---@param filter string The filter string to apply
function api.set_filter(filter) _deck.api.set_filter(filter) end

---Get the current filter string for the current page
---@return string filter The current filter string, or empty string if none
function api.get_filter() return _deck.api.get_filter() end

---@class AvailableKeymap
---@field key string
---@field desc? string
---@field callback fun()
---@field source "entry"|"page"|"global"

---Get all currently available keymaps in the current context
---Entry-local keymaps are returned before page keymaps, then global keymaps
---@return AvailableKeymap[]
function api.get_available_keymaps() return _deck.api.get_available_keymaps() end

---@class deck.path
local path = deck.path or {}

---Split a path into segments
---@param path string
---@return string[]
function path.split(path) return _deck.path.split(path) end

---Join path segments into a path
---@param path_list string[]
---@return string
function path.join(path_list) return _deck.path.join(path_list) end

---Check whether a path matches a pattern
---@param path string[]
---@param pattern string
---@return boolean
function path.match(path, pattern) return _deck.path.match(path, pattern) end

deck.path = path

deck.api = api
deck.hook = deck.hook or {}

---Append a hook callback to be called before reload command
---@param callback fun() The callback function to execute before reload
function deck.hook.pre_reload(callback) _deck.api.append_hook_pre_reload(callback) end

---Append a hook callback to be called before quit command
---@param callback fun() The callback function to execute before quit
function deck.hook.pre_quit(callback) _deck.api.append_hook_pre_quit(callback) end

---Append a hook callback to be called after entering a page
---@param callback fun(ctx: {path: string[]}) The callback function to execute
function deck.hook.post_page_enter(callback) _deck.api.append_hook_post_page_enter(callback) end

---Send an internal command to Rust
---@param command string The command string (e.g., "quit", "reload", "scroll_by 1")
function deck.cmd(command) return _deck.cmd(command) end

---Execute a function after a delay
---@param callback fun() The function to execute
---@param delay_ms number Delay in milliseconds
function deck.defer_fn(callback, delay_ms) return _deck.defer_fn(callback, delay_ms) end
