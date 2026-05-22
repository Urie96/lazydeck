use mlua::Lua;

use super::deck;

pub fn init_lua(lua: &Lua) -> mlua::Result<()> {
    deck::register(lua)?;

    macro_rules! preset {
        ($name:literal) => {{
            #[cfg(debug_assertions)]
            {
                std::fs::read(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/preset/lua/",
                    $name,
                    ".lua"
                ))
                .expect(concat!("Failed to read preset", $name, ".lua'"))
            }
            #[cfg(not(debug_assertions))]
            {
                &include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/preset/lua/",
                    $name,
                    ".lua"
                ))[..]
            }
        }};
    }

    // Load preset files
    macro_rules! load_preset {
        ($name:literal) => {{
            lua.load(preset!($name))
                .set_name(concat!("preset/lua/", $name, ".lua"))
                .call::<()>(())
        }};
    }

    load_preset!("system")?;
    load_preset!("copy_from_neovim")?;
    load_preset!("socket")?;
    load_preset!("component")?;
    load_preset!("api")?;
    load_preset!("style")?;
    load_preset!("interactive")?;
    load_preset!("string")?;
    load_preset!("inspect")?;
    load_preset!("json")?;
    load_preset!("promise")?;
    load_preset!("time")?;
    load_preset!("keymap")?;
    load_preset!("html")?;
    load_preset!("http")?;
    load_preset!("http_server")?;
    load_preset!("cache")?;
    load_preset!("fs")?;
    load_preset!("hash")?;
    load_preset!("util")?;
    load_preset!("base64")?;
    load_preset!("url")?;
    load_preset!("clipboard")?;
    load_preset!("secrets")?;
    load_preset!("yaml")?;
    load_preset!("plugin_manager")?;
    load_preset!("manager")?;
    load_preset!("config")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{LuaLine, LuaSpan, LuaText, Renderable};
    use mlua::{
        prelude::{LuaFunction, LuaString, LuaTable, LuaValue},
        Function, Table,
    };
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::Style,
        text::{Line, Span, Text},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    struct RemotePreviewTestEnv {
        lua: Lua,
        preview_call_count: Rc<Cell<usize>>,
        notifications: Rc<RefCell<Vec<String>>>,
    }

    fn install_test_widget(lua: &Lua, urls: &[&str]) -> mlua::Result<()> {
        let globals = lua.globals();
        let widget = lua.create_table()?;
        widget.set(
            1,
            lua.create_userdata(LuaSpan(Span::styled("before", Style::default())))?,
        )?;

        for (idx, url) in urls.iter().enumerate() {
            let remote = lua.create_table()?;
            remote.set("__deck_type", "image")?;
            remote.set("source", *url)?;
            widget.set(idx + 2, remote)?;
        }

        widget.set(
            urls.len() + 2,
            lua.create_userdata(LuaSpan(Span::styled("after", Style::default())))?,
        )?;
        globals.set("test_widget", widget)?;
        Ok(())
    }

    fn make_remote_preview_test_env(fail_on_refresh: bool) -> mlua::Result<RemotePreviewTestEnv> {
        let lua = Lua::new();
        let preview_call_count = Rc::new(Cell::new(0));
        let preview_call_count_for_lua = preview_call_count.clone();
        let notifications = Rc::new(RefCell::new(Vec::new()));
        let notifications_for_lua = notifications.clone();

        let globals = lua.globals();
        let deck = lua.create_table()?;
        deck.set("hook", lua.create_table()?)?;

        let base64 = lua.create_table()?;
        base64.set(
            "encode",
            lua.create_function(|_, value: String| {
                Ok(value.replace(|c: char| !c.is_ascii_alphanumeric(), "_"))
            })?,
        )?;
        deck.set("base64", base64)?;

        let existing_files = lua.create_table()?;
        globals.set("__existing_files", existing_files.clone())?;

        let fs = lua.create_table()?;
        fs.set(
            "stat",
            lua.create_function(|lua, path: String| {
                let globals = lua.globals();
                let existing_files: Table = globals.get("__existing_files")?;
                let exists = existing_files.get::<bool>(path).unwrap_or(false);
                let stat = lua.create_table()?;
                stat.set("exists", exists)?;
                stat.set("is_file", exists)?;
                Ok(stat)
            })?,
        )?;
        fs.set(
            "mkdir",
            lua.create_function(|_, _path: String| Ok((true, Option::<String>::None)))?,
        )?;
        fs.set(
            "write_file_sync",
            lua.create_function(|lua, (path, _content): (String, LuaString)| {
                let globals = lua.globals();
                let existing_files: Table = globals.get("__existing_files")?;
                existing_files.set(path, true)?;
                Ok((true, Option::<String>::None))
            })?,
        )?;
        deck.set("fs", fs)?;

        let http_callbacks = lua.create_table()?;
        globals.set("__http_callbacks", http_callbacks.clone())?;
        let http = lua.create_table()?;
        http.set(
            "get",
            lua.create_function(|lua, (url, callback): (String, LuaFunction)| {
                let globals = lua.globals();
                let callbacks: Table = globals.get("__http_callbacks")?;
                callbacks.set(url, callback)?;
                Ok(())
            })?,
        )?;
        deck.set("http", http)?;

        deck.set(
            "notify",
            lua.create_function(move |_, message: String| {
                notifications_for_lua.borrow_mut().push(message);
                Ok(())
            })?,
        )?;

        let style = lua.create_table()?;
        style.set(
            "span",
            lua.create_function(|lua, text: String| lua.create_userdata(LuaSpan(Span::raw(text))))?,
        )?;
        style.set(
            "line",
            lua.create_function(|lua, args: LuaTable| {
                let mut spans = Vec::with_capacity(args.raw_len());
                for value in args.sequence_values::<LuaValue>() {
                    match value? {
                        LuaValue::String(s) => spans.push(Span::raw(s.to_str()?.to_string())),
                        LuaValue::UserData(ud) => {
                            if let Ok(span) = ud.borrow::<LuaSpan>() {
                                spans.push(span.0.clone());
                            } else {
                                return Err(mlua::Error::runtime(
                                    "expected Span or string in style.line",
                                ));
                            }
                        }
                        _ => {
                            return Err(mlua::Error::runtime(
                                "expected Span or string in style.line",
                            ));
                        }
                    }
                }
                lua.create_userdata(LuaLine(Line::from(spans)))
            })?,
        )?;
        style.set(
            "text",
            lua.create_function(|lua, args: LuaTable| {
                let mut lines = Vec::with_capacity(args.raw_len());
                for value in args.sequence_values::<LuaValue>() {
                    match value? {
                        LuaValue::String(s) => lines.push(Line::raw(s.to_str()?.to_string())),
                        LuaValue::UserData(ud) => {
                            if let Ok(text) = ud.borrow::<LuaText>() {
                                lines.extend(text.0.lines.clone());
                            } else if let Ok(line) = ud.borrow::<LuaLine>() {
                                lines.push(line.0.clone());
                            } else if let Ok(span) = ud.borrow::<LuaSpan>() {
                                lines.push(Line::from(span.0.clone()));
                            } else {
                                return Err(mlua::Error::runtime(
                                    "expected Text, Line, Span, or string in style.text",
                                ));
                            }
                        }
                        _ => {
                            return Err(mlua::Error::runtime(
                                "expected Text, Line, Span, or string in style.text",
                            ));
                        }
                    }
                }
                lua.create_userdata(LuaText(Text::from(lines)))
            })?,
        )?;
        style.set(
            "image",
            lua.create_function(|lua, (path, opts): (String, Option<LuaTable>)| {
                let image = lua.create_table()?;
                image.set("__deck_type", "image")?;
                image.set("source", path)?;
                image.set(
                    "max_width",
                    opts.as_ref()
                        .and_then(|t: &LuaTable| t.get::<Option<u16>>("max_width").ok())
                        .flatten(),
                )?;
                image.set(
                    "max_height",
                    opts.as_ref()
                        .and_then(|t: &LuaTable| t.get::<Option<u16>>("max_height").ok())
                        .flatten(),
                )?;
                Ok(image)
            })?,
        )?;
        deck.set("style", style)?;
        globals.set("deck", deck)?;

        let raw_deck = lua.create_table()?;
        let api = lua.create_table()?;
        api.set(
            "set_preview",
            lua.create_function(
                move |_, (_path, preview): (Option<Vec<String>>, Option<Box<dyn Renderable>>)| {
                    let call_no = preview_call_count_for_lua.get() + 1;
                    preview_call_count_for_lua.set(call_no);
                    if fail_on_refresh && call_no > 1 {
                        return Err(mlua::Error::runtime("async image preview refresh failed"));
                    }
                    if let Some(mut preview) = preview {
                        let area = Rect::new(0, 0, 40, 10);
                        let mut buf = Buffer::empty(area);
                        preview.render(area, &mut buf);
                    }
                    Ok(())
                },
            )?,
        )?;
        raw_deck.set("api", api)?;
        globals.set("_deck", raw_deck)?;

        lua.load(
            r#"
            Promise = {}
            Promise.__index = Promise

            function Promise.new(executor)
              local self = setmetatable({
                state = 'pending',
                value = nil,
                reason = nil,
                fulfilled = {},
                rejected = {},
              }, Promise)

              local function resolve(value)
                if self.state ~= 'pending' then return end
                self.state = 'fulfilled'
                self.value = value
                for _, callback in ipairs(self.fulfilled) do
                  callback(value)
                end
              end

              local function reject(reason)
                if self.state ~= 'pending' then return end
                self.state = 'rejected'
                self.reason = reason
                for _, callback in ipairs(self.rejected) do
                  callback(reason)
                end
              end

              executor(resolve, reject)
              return self
            end

            function Promise.resolve(value)
              if type(value) == 'table' and getmetatable(value) == Promise then
                return value
              end
              return Promise.new(function(resolve, _reject)
                resolve(value)
              end)
            end

            function Promise.reject(reason)
              return Promise.new(function(_resolve, reject)
                reject(reason)
              end)
            end

            function Promise:next(on_fulfilled, on_rejected)
              return Promise.new(function(resolve, reject)
                local function handle_fulfilled(value)
                  if not on_fulfilled then
                    resolve(value)
                    return
                  end

                  local ok, result = pcall(on_fulfilled, value)
                  if ok then
                    resolve(result)
                  else
                    reject(result)
                  end
                end

                local function handle_rejected(reason)
                  if not on_rejected then
                    reject(reason)
                    return
                  end

                  local ok, result = pcall(on_rejected, reason)
                  if ok then
                    resolve(result)
                  else
                    reject(result)
                  end
                end

                if self.state == 'fulfilled' then
                  handle_fulfilled(self.value)
                elseif self.state == 'rejected' then
                  handle_rejected(self.reason)
                else
                  table.insert(self.fulfilled, handle_fulfilled)
                  table.insert(self.rejected, handle_rejected)
                end
              end)
            end

            function Promise:catch(on_rejected)
              return self:next(nil, on_rejected)
            end

            function Promise.allSettled(promises)
              return Promise.new(function(resolve)
                local results = {}
                local remaining = #promises

                if remaining == 0 then
                  resolve(results)
                  return
                end

                for i, value in ipairs(promises) do
                  Promise.resolve(value):next(
                    function(value)
                      results[i] = { status = 'fulfilled', value = value }
                    end,
                    function(reason)
                      results[i] = { status = 'rejected', reason = reason }
                    end
                  ):next(function()
                    remaining = remaining - 1
                    if remaining == 0 then
                      resolve(results)
                    end
                  end)
                end
              end)
            end
            "#,
        )
        .exec()?;

        lua.load(&include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/preset/lua/api.lua"))[..])
            .set_name("preset/lua/api.lua")
            .exec()?;

        Ok(RemotePreviewTestEnv {
            lua,
            preview_call_count,
            notifications,
        })
    }

    #[test]
    fn util_preset_provides_unpack_compatibility() -> mlua::Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();
        globals.set("deck", lua.create_table()?)?;

        let raw_deck = lua.create_table()?;
        raw_deck.set(
            "osc52_copy",
            lua.create_function(|_, _text: String| Ok(()))?,
        )?;
        globals.set("_deck", raw_deck)?;

        lua.load(&include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/preset/lua/util.lua"))[..])
            .set_name("preset/lua/util.lua")
            .exec()?;

        let table_unpack_exists: bool =
            lua.load("return type(table.unpack) == 'function'").eval()?;
        let unpack_exists: bool = lua.load("return type(unpack) == 'function'").eval()?;
        let unpack_works: i64 = lua
            .load("return select(2, table.unpack({ 10, 20, 30 }))")
            .eval()?;

        assert!(table_unpack_exists);
        assert!(unpack_exists);
        assert_eq!(unpack_works, 20);

        Ok(())
    }

    #[test]
    fn string_preset_utf8_sub_works() -> mlua::Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();

        globals.set("deck", lua.create_table()?)?;

        let raw_deck = lua.create_table()?;
        let style = lua.create_table()?;
        style.set("span", lua.create_function(|_, s: String| Ok(s))?)?;
        style.set("ansi", lua.create_function(|_, s: String| Ok(s))?)?;
        raw_deck.set("style", style)?;
        raw_deck.set(
            "split",
            lua.create_function(|lua, (s, sep): (String, String)| {
                let parts = lua.create_table()?;
                for (idx, part) in s.split(&sep).enumerate() {
                    parts.set(idx + 1, part)?;
                }
                Ok(parts)
            })?,
        )?;
        globals.set("_deck", raw_deck)?;

        lua.load(
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/preset/lua/string.lua"
            ))[..],
        )
        .set_name("preset/lua/string.lua")
        .exec()?;

        let result: String = lua
            .load(r#"return string.utf8_sub("Hello 世界！🌍", 7, 8)"#)
            .eval()?;
        let tail: String = lua
            .load(r#"return string.utf8_sub("Hello 世界！🌍", -3, -1)"#)
            .eval()?;

        assert_eq!(result, "世界");
        assert_eq!(tail, "界！🌍");

        Ok(())
    }

    #[test]
    fn remote_image_preview_refreshes_after_all_downloads_finish() -> mlua::Result<()> {
        let env = make_remote_preview_test_env(false)?;
        install_test_widget(
            &env.lua,
            &["http://example.com/a.png", "http://example.com/b.png"],
        )?;

        env.lua
            .load("deck.api.set_preview({ 'demo' }, test_widget)")
            .exec()?;
        assert_eq!(env.preview_call_count.get(), 1);

        let callbacks: Table = env.lua.globals().get("__http_callbacks")?;
        let callback_a: Function = callbacks.get("http://example.com/a.png")?;
        let response_a = env.lua.create_table()?;
        response_a.set("success", true)?;
        response_a.set("status", 200)?;
        response_a.set("body", "image-a")?;
        callback_a.call::<()>(response_a)?;
        assert_eq!(env.preview_call_count.get(), 1);

        let callback_b: Function = callbacks.get("http://example.com/b.png")?;
        let response_b = env.lua.create_table()?;
        response_b.set("success", true)?;
        response_b.set("status", 200)?;
        response_b.set("body", "image-b")?;
        callback_b.call::<()>(response_b)?;

        assert_eq!(env.preview_call_count.get(), 2);
        assert!(env.notifications.borrow().is_empty());

        Ok(())
    }

    #[test]
    fn remote_image_preview_notifies_when_async_refresh_fails() -> mlua::Result<()> {
        let env = make_remote_preview_test_env(true)?;
        install_test_widget(&env.lua, &["http://example.com/a.png"])?;

        env.lua
            .load("deck.api.set_preview({ 'demo' }, test_widget)")
            .exec()?;
        assert_eq!(env.preview_call_count.get(), 1);

        let callbacks: Table = env.lua.globals().get("__http_callbacks")?;
        let callback_a: Function = callbacks.get("http://example.com/a.png")?;
        let response_a = env.lua.create_table()?;
        response_a.set("success", true)?;
        response_a.set("status", 200)?;
        response_a.set("body", "image-a")?;
        callback_a.call::<()>(response_a)?;

        assert_eq!(env.preview_call_count.get(), 2);
        let notifications = env.notifications.borrow();
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].contains("Image preview error"));
        assert!(notifications[0].contains("async image preview refresh failed"));

        Ok(())
    }
}
