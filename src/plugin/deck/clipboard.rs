use ::base64::engine::general_purpose;
use ::base64::Engine;
use mlua::prelude::*;
use std::io::{self, Write};
use std::process::Command;

struct ClipboardCommand {
    program: &'static str,
    args: &'static [&'static str],
}

const CLIPBOARD_GET_COMMANDS: &[ClipboardCommand] = &[
    ClipboardCommand {
        program: "termux-clipboard-get",
        args: &[],
    },
    ClipboardCommand {
        program: "pbpaste",
        args: &[],
    },
    ClipboardCommand {
        program: "wl-paste",
        args: &["--no-newline"],
    },
    ClipboardCommand {
        program: "xclip",
        args: &["-selection", "clipboard", "-out"],
    },
    ClipboardCommand {
        program: "xsel",
        args: &["--clipboard", "--output"],
    },
    ClipboardCommand {
        program: "powershell.exe",
        args: &["-NoProfile", "-Command", "Get-Clipboard -Raw"],
    },
    ClipboardCommand {
        program: "powershell",
        args: &["-NoProfile", "-Command", "Get-Clipboard -Raw"],
    },
    ClipboardCommand {
        program: "pwsh",
        args: &["-NoProfile", "-Command", "Get-Clipboard -Raw"],
    },
];

fn command_exists(program: &str) -> bool {
    which::which(program).is_ok()
}

fn platform_clipboard_get() -> mlua::Result<String> {
    let mut failures = Vec::new();
    let mut tried = Vec::new();

    for command in CLIPBOARD_GET_COMMANDS {
        if !command_exists(command.program) {
            continue;
        }

        tried.push(command.program);
        match Command::new(command.program).args(command.args).output() {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    failures.push(format!("{} exited with {}", command.program, output.status));
                } else {
                    failures.push(format!(
                        "{} exited with {}: {}",
                        command.program, output.status, stderr
                    ));
                }
            }
            Err(err) => {
                failures.push(format!("{} failed to run: {}", command.program, err));
            }
        }
    }

    if tried.is_empty() {
        return Err(LuaError::RuntimeError(format!(
            "Failed to get clipboard content: no clipboard command available (tried: {})",
            CLIPBOARD_GET_COMMANDS
                .iter()
                .map(|command| command.program)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    Err(LuaError::RuntimeError(format!(
        "Failed to get clipboard content: {}",
        failures.join("; ")
    )))
}

fn osc52_clipboard_set(text: &str) -> mlua::Result<()> {
    let encoded = general_purpose::STANDARD.encode(text);
    let osc_sequence = format!("\x1b]52;c;{}\x07", encoded);

    io::stdout()
        .write_all(osc_sequence.as_bytes())
        .map_err(|e| LuaError::RuntimeError(format!("Failed to write OSC 52 sequence: {}", e)))?;
    io::stdout()
        .flush()
        .map_err(|e| LuaError::RuntimeError(format!("Failed to flush stdout: {}", e)))?;

    Ok(())
}

/// Get clipboard content
fn get(_lua: &Lua, _: ()) -> mlua::Result<String> {
    platform_clipboard_get()
}

/// Set clipboard content
fn set(_lua: &Lua, text: String) -> mlua::Result<()> {
    osc52_clipboard_set(&text)
}

/// Create the deck.clipboard table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get_fn = lua.create_function(get)?.into_lua(lua)?;
    let set_fn = lua.create_function(set)?.into_lua(lua)?;

    lua.create_table_from([("get", get_fn), ("set", set_fn)])
}
