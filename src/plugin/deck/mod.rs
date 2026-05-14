mod api;
mod base64;
mod cache;
mod clipboard;
mod fs;
mod highlighter;
mod html;
mod http;
mod http_server;
mod json;
mod keymap;
mod path;
mod secrets;
mod socket;
mod style;
mod system;
mod time;
mod url;
mod yaml;

use crate::widgets::{LuaLine, LuaSpan};
use crate::{plugin, Event};
use ::base64::engine::general_purpose;
use ::base64::Engine;
use mlua::prelude::*;
use ratatui::text::Line;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Get the log file path for Lua plugin logs
fn get_log_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/state/lazydeck/lua.log")
    } else {
        PathBuf::from("/tmp/lazydeck.log")
    }
}

/// Write a log entry to the log file
fn write_log(level: &str, message: &str) {
    let log_path = get_log_path();

    // Ensure the directory exists
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Format the log entry with timestamp
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let log_entry = format!("[{}][{}] {}\n", timestamp, level, message);

    // Append to log file
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut file| file.write_all(log_entry.as_bytes()));
}

pub(crate) fn flush_pending_cache() -> mlua::Result<()> {
    cache::flush_dirty_namespaces()
}

pub(super) fn register(lua: &Lua) -> mlua::Result<()> {
    cache::start_background_flush_task();

    let keymap = keymap::new_table(lua)?.into_lua(lua)?;
    let api = api::new_table(lua)?.into_lua(lua)?;
    let cache = cache::new_table(lua)?.into_lua(lua)?;
    let clipboard = clipboard::new_table(lua)?.into_lua(lua)?;
    let fs = fs::new_table(lua)?.into_lua(lua)?;
    let html = html::new_table(lua)?.into_lua(lua)?;
    let http = http::new_table(lua)?.into_lua(lua)?;
    let http_server = http_server::new_table(lua)?.into_lua(lua)?;
    let path = path::new_table(lua)?.into_lua(lua)?;
    let secrets = secrets::new_table(lua)?.into_lua(lua)?;
    let socket = socket::new_table(lua)?.into_lua(lua)?;
    let time = time::new_table(lua)?.into_lua(lua)?;
    let url = url::new_table(lua)?.into_lua(lua)?;
    let json = json::new_table(lua)?.into_lua(lua)?;
    let yaml = yaml::new_table(lua)?.into_lua(lua)?;
    let base64 = base64::new_table(lua)?.into_lua(lua)?;

    let defer_fn = lua
        .create_function(|lua, (f, ms): (LuaFunction, u64)| {
            let sender = plugin::clone_sender(lua)?;

            tokio::task::spawn_local(async move {
                sleep(Duration::from_millis(ms)).await;
                sender
                    .send(Event::LuaCallback(Box::new(move |_| f.call(()))))
                    .unwrap();
            });
            Ok(())
        })?
        .into_lua(lua)?;

    let cmd = lua
        .create_function(|lua, cmd: String| plugin::send_event(lua, Event::Command(cmd)))?
        .into_lua(lua)?;

    let system_tbl = system::new_table(lua)?;

    let split = lua
        .create_function(|lua, (s, sep): (String, String)| lua.create_sequence_from(s.split(&sep)))?
        .into_lua(lua)?;

    let log_fn = lua
        .create_function(
            |lua, (level, format, args): (String, LuaString, LuaMultiValue)| {
                // Convert all args to strings
                let mut arg_strings = Vec::new();
                for arg in args {
                    match String::from_lua(arg, lua) {
                        Ok(s) => arg_strings.push(s),
                        Err(_) => arg_strings.push("[unconvertible]".to_string()),
                    }
                }

                // Format the message using the format string and args
                let message = if arg_strings.is_empty() {
                    format.to_string_lossy().to_string()
                } else {
                    // Simple format: replace {} with args sequentially
                    let fmt_str = format.to_string_lossy().to_string();
                    let mut result = fmt_str.clone();
                    let mut arg_idx = 0;
                    while let Some(pos) = result.find("{}") {
                        if arg_idx < arg_strings.len() {
                            result.replace_range(pos..pos + 2, &arg_strings[arg_idx]);
                            arg_idx += 1;
                        } else {
                            break;
                        }
                    }
                    result
                };

                write_log(&level, &message);
                Ok(())
            },
        )?
        .into_lua(lua)?;

    let osc52_copy = lua
        .create_function(|_, text: String| {
            // Encode text as base64
            let encoded = general_purpose::STANDARD.encode(&text);

            // Build OSC 52 escape sequence: ESC ] 52 ; c ; <base64_data> BEL
            let osc_sequence = format!("\x1b]52;c;{}\x07", encoded);

            // Write to terminal stdout
            if let Err(e) = io::stdout().write_all(osc_sequence.as_bytes()) {
                return Err(LuaError::RuntimeError(format!(
                    "Failed to write OSC 52 sequence: {}",
                    e
                )));
            }

            // Flush to ensure the sequence is sent
            if let Err(e) = io::stdout().flush() {
                return Err(LuaError::RuntimeError(format!(
                    "Failed to flush stdout: {}",
                    e
                )));
            }

            Ok(())
        })?
        .into_lua(lua)?;

    let notify_fn = lua
        .create_function(|lua, message: crate::widgets::LuaText| {
            plugin::send_event(lua, Event::Notify(message.0))
        })?
        .into_lua(lua)?;

    // deck.confirm: show a confirmation dialog
    let confirm_fn = lua.create_function(|lua, opts: LuaTable| -> mlua::Result<()> {
        // title is optional, defaults to "Confirm"
        let title: Option<String> = opts.get("title").ok();
        let title = title.or_else(|| Some("Confirm".to_string()));
        let prompt: String = opts.get("prompt")?;
        let on_confirm: LuaFunction = opts.get("on_confirm")?;
        let on_cancel = opts.get("on_cancel").ok();
        plugin::send_event(
            lua,
            Event::ShowConfirm {
                title,
                prompt,
                on_confirm,
                on_cancel,
            },
        )?;
        Ok(())
    })?;

    // deck.select: show a selection dialog
    let select_fn = lua.create_function(
        |lua, (opts, on_selection): (LuaTable, LuaFunction)| -> mlua::Result<()> {
            // Parse options: can be an array of strings or an array of tables
            let options_lua: LuaValue = opts.get("options")?;

            let mut select_options = Vec::new();

            match options_lua {
                LuaValue::Table(table) => {
                    // Iterate over the table
                    for pair in table.pairs::<LuaValue, LuaValue>() {
                        let (_, value) = pair?;
                        match value {
                            LuaValue::String(s) => {
                                // Simple string: value = display = string
                                let display = s.to_string_lossy().to_string();
                                // Create a new Lua string from the display text
                                let lua_string = lua.create_string(&display)?;
                                select_options.push(crate::SelectOption {
                                    value: LuaValue::String(lua_string),
                                    display: Line::from(display),
                                });
                            }
                            LuaValue::Table(t) => {
                                // Table with value and display fields
                                let display: Line = match t.get::<LuaValue>("display")? {
                                    LuaValue::Nil => {
                                        // Use value as display fallback
                                        match t.get::<LuaValue>("value")? {
                                            LuaValue::String(s) => Line::from(s.to_string_lossy()),
                                            _ => Line::from("?"),
                                        }
                                    }
                                    LuaValue::String(s) => Line::from(s.to_string_lossy()),
                                    LuaValue::UserData(ud) => {
                                        if let Ok(span) = ud.borrow::<LuaSpan>() {
                                            Line::from(span.0.clone())
                                        } else if let Ok(line) = ud.borrow::<LuaLine>() {
                                            line.0.clone()
                                        } else {
                                            return Err(LuaError::RuntimeError(
                                                "Display must be string, Span, or Line".to_string(),
                                            ));
                                        }
                                    }
                                    _ => {
                                        return Err(LuaError::RuntimeError(
                                            "Display must be string, Span, or Line".to_string(),
                                        ));
                                    }
                                };
                                // Get the value field
                                let value: LuaValue = t.get("value")?;
                                select_options.push(crate::SelectOption { value, display });
                            }
                            _ => {
                                return Err(LuaError::RuntimeError(
                                    "Options must be strings or tables".to_string(),
                                ));
                            }
                        }
                    }
                }
                _ => {
                    return Err(LuaError::RuntimeError(
                        "Options must be a table/array".to_string(),
                    ));
                }
            }

            if select_options.is_empty() {
                return Err(LuaError::RuntimeError(
                    "Options cannot be empty".to_string(),
                ));
            }

            // prompt is optional
            let prompt: Option<String> = opts.get("prompt").ok();

            plugin::send_event(
                lua,
                Event::ShowSelect {
                    prompt,
                    options: select_options,
                    on_selection,
                },
            )?;
            Ok(())
        },
    )?;

    let style_tbl = lua.create_table_from([
        ("span", style::span(lua)?),
        ("line", style::line(lua)?),
        ("text", style::text(lua)?),
        ("highlight", style::highlight(lua)?),
        ("ansi", style::ansi(lua)?),
        ("align_columns", style::align_columns(lua)?),
    ])?;

    let input_tbl = lua.create_table()?;
    input_tbl.set(
        "show",
        lua.create_function(|lua, opts: LuaTable| -> mlua::Result<()> {
            let prompt: String = opts.get("prompt").unwrap_or_else(|_| "".to_string());
            let placeholder: String = opts.get("placeholder").unwrap_or_else(|_| "".to_string());
            let value: String = opts.get("value").unwrap_or_else(|_| "".to_string());
            let on_submit: LuaFunction = opts.get("on_submit")?;

            let on_cancel: LuaFunction = opts
                .get("on_cancel")
                .unwrap_or_else(|_| lua.create_function(|_, ()| Ok(())).unwrap());
            let on_change: LuaFunction = opts
                .get("on_change")
                .unwrap_or_else(|_| lua.create_function(|_, ()| Ok(())).unwrap());

            plugin::send_event(
                lua,
                Event::ShowInput {
                    prompt,
                    placeholder,
                    value,
                    on_submit,
                    on_cancel,
                    on_change,
                },
            )?;
            Ok(())
        })?,
    )?;
    input_tbl.set(
        "get",
        lua.create_function(|lua, ()| -> mlua::Result<LuaValue> {
            let value: Option<String> =
                plugin::borrow_scope_state(lua, |state| Ok(state.input_dialog_get_text()))?;
            match value {
                Some(value) => Ok(value.into_lua(lua)?),
                None => Ok(LuaNil.into_lua(lua)?),
            }
        })?,
    )?;
    input_tbl.set(
        "set",
        lua.create_function(|lua, value: String| -> mlua::Result<()> {
            let result: Option<(String, LuaFunction, bool)> = plugin::mut_scope_state(lua, |state| {
                Ok(state.input_dialog_replace_text(value))
            })?;
            match result {
                Some((text, on_change, changed)) => {
                    if changed {
                        let sender = plugin::clone_sender(lua)?;
                        sender
                            .send(Event::LuaCallback(Box::new(move |_lua| on_change.call::<()>(text))))
                            .map_err(|e| LuaError::RuntimeError(format!("Failed to schedule input.set callback: {}", e)))?;
                    }
                    Ok(())
                }
                None => Err(LuaError::RuntimeError(
                    "No input dialog is currently open".to_string(),
                )),
            }
        })?,
    )?;

    let deck = lua.create_table_from([
        ("defer_fn", defer_fn),
        ("keymap", keymap),
        ("api", api),
        ("cache", cache),
        ("clipboard", clipboard),
        ("fs", fs),
        ("html", html),
        ("http", http),
        ("http_server", http_server),
        ("cmd", cmd),
        ("split", split),
        ("system", mlua::Value::Table(system_tbl)),
        ("socket", socket),
        ("path", path),
        ("secrets", secrets),
        ("time", time),
        ("url", url),
        ("json", json),
        ("yaml", yaml),
        ("base64", base64),
        ("log", log_fn),
        ("osc52_copy", osc52_copy),
        ("notify", notify_fn),
        ("confirm", mlua::Value::Function(confirm_fn)),
        ("select", mlua::Value::Function(select_fn)),
        ("input", mlua::Value::Table(input_tbl)),
        ("style", mlua::Value::Table(style_tbl)),
    ])?;
    lua.globals().raw_set("deck", lua.create_table()?)?;
    lua.globals().raw_set("_deck", deck)
}
