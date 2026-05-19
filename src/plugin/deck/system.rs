use crate::{plugin, Event};
use anyhow::{bail, Context};
use mlua::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

fn external_editor_command() -> anyhow::Result<Vec<String>> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());
    let cmd = shell_words::split(&editor).context("Failed to parse $VISUAL/$EDITOR")?;
    if cmd.is_empty() {
        bail!("Empty editor command");
    }
    Ok(cmd)
}

fn editor_tempfile_path(path_hint: Option<&str>, ext_hint: Option<&str>) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let suffix = ext_hint
        .map(|ext| ext.trim())
        .filter(|ext| !ext.is_empty())
        .map(|ext| ext.trim_start_matches('.'))
        .map(|ext| format!(".{}", ext))
        .or_else(|| {
            path_hint
                .and_then(|p| std::path::Path::new(p).extension().and_then(|s| s.to_str()))
                .map(|ext| format!(".{}", ext))
        })
        .unwrap_or_default();

    path.push(format!(
        "lazydeck-edit-{}-{}{}",
        std::process::id(),
        nanos,
        suffix
    ));
    path
}

/// Create the deck.system table with executable, open, exec, spawn, and kill functions
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let system_tbl = lua.create_table()?;

    // deck.system.executable: check if a command is executable (synchronous)
    let executable_fn = lua.create_function(|_, cmd: String| {
        // Check if command exists and is executable
        Ok(which::which(&cmd).is_ok())
    })?;

    // deck.system.open: open a file using the system's default application
    let open_fn = lua.create_function(|_, file_path: String| {
        // Use the `open` crate to open the file with the system's default application
        open::that(&file_path).map_err(|e| {
            LuaError::RuntimeError(format!("Failed to open file '{}': {}", file_path, e))
        })
    })?;

    let edit_fn =
        lua.create_function(|lua, (opts, callback): (LuaTable, Option<LuaFunction>)| {
            let path: Option<String> = opts.get("path").ok();
            let content: Option<LuaString> = opts.get("content").ok();
            let ext: Option<String> = opts.get("ext").ok();

            let (edit_path, cleanup_after): (PathBuf, bool) = if let Some(path) = path.as_ref() {
                if let Some(content) = content {
                    fs::write(path, content.as_bytes()).map_err(|e| {
                        LuaError::RuntimeError(format!("Failed to write file '{}': {}", path, e))
                    })?;
                }
                (PathBuf::from(path), false)
            } else {
                let temp_path = editor_tempfile_path(None, ext.as_deref());
                let initial_bytes = content.map(|c| c.as_bytes().to_vec()).unwrap_or_default();
                fs::write(&temp_path, &initial_bytes).map_err(|e| {
                    LuaError::RuntimeError(format!(
                        "Failed to prepare editor temp file '{}': {}",
                        temp_path.display(),
                        e
                    ))
                })?;
                (temp_path, true)
            };

            let mut cmd =
                external_editor_command().map_err(|e| LuaError::RuntimeError(format!("{}", e)))?;
            cmd.push(edit_path.to_string_lossy().to_string());

            let on_complete = if let Some(callback) = callback {
                Some(lua.create_function(move |lua, exit_code: i32| {
                    let read_result = fs::read(&edit_path);
                    if cleanup_after {
                        let _ = fs::remove_file(&edit_path);
                    }

                    let mut error: Option<String> = None;
                    if exit_code != 0 {
                        error = Some(format!("Editor exited with code {}", exit_code));
                    }

                    let content = match read_result {
                        Ok(bytes) => Some(lua.create_string(&bytes)?),
                        Err(e) => {
                            if error.is_none() {
                                error = Some(format!(
                                    "Failed to read edited file '{}': {}",
                                    edit_path.display(),
                                    e
                                ));
                            }
                            None
                        }
                    };

                    callback.call::<()>((content, error))
                })?)
            } else if cleanup_after {
                Some(lua.create_function(move |_lua, _exit_code: i32| {
                    let _ = fs::remove_file(&edit_path);
                    Ok(())
                })?)
            } else {
                None
            };

            plugin::send_event(
                lua,
                Event::InteractiveCommand {
                    cmd,
                    on_complete,
                    wait_confirm: None,
                },
            )
        })?;

    // Add executable function
    system_tbl.set("executable", executable_fn)?;

    // Add open function
    system_tbl.set("open", open_fn)?;
    system_tbl.set("edit", edit_fn)?;

    // Add _exec function for executing commands (internal implementation)
    // The args table contains: cmd, callback, stdin, env
    let system_exec = lua.create_function(|lua, args: LuaTable| {
        let cmd: Vec<String> = args.get("cmd")?;

        if cmd.is_empty() {
            return Err(LuaError::RuntimeError(
                "Command cannot be empty".to_string(),
            ));
        }

        let callback: LuaFunction = args.get("callback")?;

        // Parse options table
        let stdin_data: Option<String> = args.get("stdin").ok();
        let mut env_vars: Vec<(String, String)> = Vec::new();

        if let Ok(env_table) = args.get::<LuaTable>("env") {
            for pair in env_table.pairs::<String, String>() {
                let (k, v) = pair?;
                env_vars.push((k, v));
            }
        }

        let sender = plugin::clone_sender(lua)?;

        tokio::task::spawn_local(async move {
            let mut it = cmd.into_iter();
            let command = it.next().unwrap();
            let args: Vec<String> = it.collect();

            let mut cmd_builder = Command::new(&command);
            cmd_builder.args(&args);

            // Set environment variables
            for (k, v) in env_vars {
                cmd_builder.env(&k, &v);
            }

            // Execute with or without stdin
            let output = if let Some(stdin) = stdin_data {
                // Spawn process with piped stdin
                match cmd_builder
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        // Write to stdin if available
                        let result = if let Some(mut stdin_handle) = child.stdin.take() {
                            match tokio::io::AsyncWriteExt::write_all(
                                &mut stdin_handle,
                                stdin.as_bytes(),
                            )
                            .await
                            {
                                Ok(_) => {
                                    drop(stdin_handle);
                                    child.wait_with_output().await
                                }
                                Err(e) => Err(e),
                            }
                        } else {
                            child.wait_with_output().await
                        };
                        result
                    }
                    Err(e) => Err(e),
                }
            } else {
                cmd_builder.output().await
            };

            let _ = sender.send(Event::LuaCallback(Box::new(move |lua| {
                let out = match output {
                    Ok(output) => lua.create_table_from([
                        ("code", output.status.code().into_lua(lua)?),
                        ("stdout", lua.create_string(output.stdout)?.into_lua(lua)?),
                        ("stderr", lua.create_string(output.stderr)?.into_lua(lua)?),
                    ]),
                    Err(e) => {
                        let (code, err) = if e.kind() == std::io::ErrorKind::NotFound {
                            (127, format!("command not found: {}", command))
                        } else {
                            (1, e.to_string())
                        };
                        lua.create_table_from([
                            ("code", code.into_lua(lua)?),
                            ("stdout", "".into_lua(lua)?),
                            ("stderr", err.into_lua(lua)?),
                        ])
                    }
                };
                let out = out?;
                callback.call(out)
            })));
        });

        Ok(())
    })?;

    system_tbl.set("exec", system_exec)?;

    system_tbl.set(
        "spawn",
        lua.create_function(|_lua, args: LuaTable| -> mlua::Result<u32> {
            let cmd: Vec<String> = args.get("cmd")?;

            if cmd.is_empty() {
                return Err(LuaError::RuntimeError(
                    "Command cannot be empty".to_string(),
                ));
            }

            let mut it = cmd.into_iter();
            let command = it.next().unwrap();
            let args: Vec<String> = it.collect();

            let mut cmd_builder = Command::new(&command);
            cmd_builder.args(&args);
            cmd_builder.stdin(Stdio::null());
            cmd_builder.stdout(Stdio::null());
            cmd_builder.stderr(Stdio::null());
            cmd_builder.kill_on_drop(false);

            let child = cmd_builder.spawn().map_err(|e| {
                LuaError::RuntimeError(format!(
                    "Failed to spawn background command '{}': {}",
                    command, e
                ))
            })?;

            Ok(child.id().unwrap_or(0))
        })?
        .into_lua(lua)?,
    )?;

    system_tbl.set(
        "kill",
        lua.create_function(
            |_lua, (pid, signal): (u32, Option<i32>)| -> mlua::Result<()> {
                let sig = signal.unwrap_or(libc::SIGTERM);
                let rc = unsafe { libc::kill(pid as i32, sig) };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(LuaError::RuntimeError(format!(
                        "Failed to kill process {}: {}",
                        pid,
                        std::io::Error::last_os_error()
                    )))
                }
            },
        )?
        .into_lua(lua)?,
    )?;

    system_tbl.set(
        "interactive",
        lua.create_function(|lua, args: LuaTable| {
            let cmd: Vec<String> = args.get("cmd")?;

            if cmd.is_empty() {
                return Err(LuaError::RuntimeError(
                    "Command cannot be empty".to_string(),
                ));
            }

            let on_complete: Option<LuaFunction> = args.get("on_complete").ok();
            let wait_confirm: Option<LuaFunction> = args.get("wait_confirm").ok();

            plugin::send_event(
                lua,
                Event::InteractiveCommand {
                    cmd,
                    on_complete,
                    wait_confirm,
                },
            )
        })?
        .into_lua(lua)?,
    )?;

    Ok(system_tbl)
}
