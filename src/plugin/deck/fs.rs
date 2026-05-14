use mlua::prelude::*;
use tokio::io::AsyncReadExt;
use tokio::task::spawn_local;

async fn read_file_limited(
    path: String,
    max_chars: Option<usize>,
) -> Result<(String, bool), String> {
    let Some(limit) = max_chars else {
        return tokio::fs::read_to_string(&path)
            .await
            .map(|content| (content, false))
            .map_err(|err| err.to_string());
    };

    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|err| err.to_string())?;
    let mut output = String::new();
    let mut truncated = false;
    let mut pending = Vec::new();
    let mut char_count = 0usize;
    let mut buf = vec![0u8; 8192];

    loop {
        let read = file.read(&mut buf).await.map_err(|err| err.to_string())?;
        if read == 0 {
            if pending.is_empty() {
                break;
            }
            let tail = std::str::from_utf8(&pending)
                .map_err(|err| format!("stream did not contain valid UTF-8: {}", err))?;
            for ch in tail.chars() {
                if char_count >= limit {
                    truncated = true;
                    break;
                }
                output.push(ch);
                char_count += 1;
            }
            break;
        }

        pending.extend_from_slice(&buf[..read]);

        loop {
            match std::str::from_utf8(&pending) {
                Ok(valid) => {
                    for ch in valid.chars() {
                        if char_count >= limit {
                            truncated = true;
                            break;
                        }
                        output.push(ch);
                        char_count += 1;
                    }
                    pending.clear();
                    break;
                }
                Err(err) => {
                    let valid_up_to = err.valid_up_to();
                    if valid_up_to == 0 {
                        if err.error_len().is_some() {
                            return Err(format!("stream did not contain valid UTF-8: {}", err));
                        }
                        break;
                    }

                    let valid = std::str::from_utf8(&pending[..valid_up_to])
                        .map_err(|utf8_err| utf8_err.to_string())?;
                    for ch in valid.chars() {
                        if char_count >= limit {
                            truncated = true;
                            break;
                        }
                        output.push(ch);
                        char_count += 1;
                    }

                    let rest = pending.split_off(valid_up_to);
                    pending = rest;

                    if err.error_len().is_some() && !pending.is_empty() {
                        return Err(format!("stream did not contain valid UTF-8: {}", err));
                    }
                }
            }

            if truncated {
                break;
            }
        }

        if truncated || char_count >= limit {
            let mut extra = [0u8; 1];
            let has_more = file.read(&mut extra).await.map_err(|err| err.to_string())?;
            truncated = truncated || has_more > 0 || !pending.is_empty();
            break;
        }
    }

    Ok((output, truncated))
}

/// Check if a path is readable
fn is_readable(path: &std::path::Path) -> bool {
    // Try to open with read-only mode
    match std::fs::OpenOptions::new().read(true).open(path) {
        Ok(file) => {
            // Successfully opened, check if we can actually read metadata
            file.metadata().is_ok()
        }
        Err(_) => false,
    }
}

/// Check if a path is writable
fn is_writable(path: &std::path::Path) -> bool {
    // Try to open with write mode (without truncating)
    match std::fs::OpenOptions::new()
        .write(true)
        .create(false)
        .open(path)
    {
        Ok(file) => {
            // Successfully opened, can write
            drop(file);
            true
        }
        Err(e) => {
            // If the file doesn't exist, check if we can create it in the parent directory
            if e.kind() == std::io::ErrorKind::NotFound {
                if let Some(parent) = path.parent() {
                    return is_writable(parent);
                }
            }
            false
        }
    }
}

/// Check if a path is executable
fn is_executable(path: &std::path::Path) -> bool {
    // On Unix systems, check file permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                let mode = permissions.mode();
                // Check execute bits (owner, group, or others)
                (mode & 0o111) != 0
            }
            Err(_) => false,
        }
    }

    #[cfg(windows)]
    {
        // On Windows, check file extension or use other methods
        // This is a simplified check - Windows executable detection is more complex
        match path.extension() {
            Some(ext) => {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                matches!(
                    ext_lower.as_str(),
                    "exe" | "bat" | "cmd" | "ps1" | "com" | "msi" | "sh"
                )
            }
            None => false,
        }
    }
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let read_dir_sync = lua
        .create_function(|lua, path: String| {
            let f = || {
                std::fs::read_dir(path)?
                    .map(|v| {
                        v.into_lua_err().and_then(|e| {
                            let path = e.path();
                            let metadata = e.metadata().ok();
                            let is_dir = metadata
                                .as_ref()
                                .map(|m| m.is_dir())
                                .unwrap_or_else(|| path.is_dir());
                            let size = metadata.as_ref().map(|m| m.len());

                            let tbl = lua.create_table_with_capacity(0, 3)?;
                            tbl.raw_set("name", e.file_name())?;
                            tbl.raw_set("is_dir", is_dir)?;
                            tbl.raw_set("size", size)?;
                            Ok(tbl)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            match f() {
                Ok(entries) => (entries, LuaNil).into_lua_multi(lua),
                Err(e) => (LuaNil, e.to_string()).into_lua_multi(lua),
            }
        })?
        .into_lua(lua)?;

    let read_file_sync = lua
        .create_function(
            |_, path: String| -> mlua::Result<(String, Option<String>)> {
                match std::fs::read_to_string(&path) {
                    Ok(content) => Ok((content, None)),
                    Err(e) => Ok((String::new(), Some(e.to_string()))),
                }
            },
        )?
        .into_lua(lua)?;

    let read_file = lua
        .create_function(
            |lua,
             (path, opts, callback): (String, Option<LuaTable>, LuaFunction)|
             -> mlua::Result<()> {
                let max_chars = opts
                    .as_ref()
                    .and_then(|tbl| tbl.get::<Option<usize>>("max_chars").ok())
                    .flatten();
                let sender = crate::plugin::clone_sender(lua)?;

                spawn_local(async move {
                    let result = read_file_limited(path, max_chars).await;
                    let _ = sender.send(crate::Event::LuaCallback(Box::new(
                        move |_lua| match result {
                            Ok((content, truncated)) => {
                                let meta = _lua.create_table()?;
                                meta.set("truncated", truncated)?;
                                callback.call::<()>((content, LuaNil, meta))
                            }
                            Err(err) => {
                                callback.call::<()>((String::new(), err.to_string(), LuaNil))
                            }
                        },
                    )));
                });

                Ok(())
            },
        )?
        .into_lua(lua)?;

    let write_file_sync = lua
        .create_function(
            |_, (path, content): (String, LuaString)| -> mlua::Result<(bool, Option<String>)> {
                match std::fs::write(&path, content.as_bytes()) {
                    Ok(_) => Ok((true, None)),
                    Err(e) => Ok((false, Some(e.to_string()))),
                }
            },
        )?
        .into_lua(lua)?;

    let stat_sync = lua
        .create_function(|lua, path: String| -> mlua::Result<LuaTable> {
            let path_obj = std::path::Path::new(&path);
            let exists = path_obj.exists();
            let metadata = if exists {
                std::fs::metadata(&path).ok()
            } else {
                None
            };
            let (is_file, is_dir, size) = if let Some(metadata) = metadata.as_ref() {
                (metadata.is_file(), metadata.is_dir(), Some(metadata.len()))
            } else {
                (false, false, None)
            };

            let is_readable = exists && is_readable(path_obj);
            let is_writable = is_writable(path_obj);
            let is_executable = exists && is_executable(path_obj);

            lua.create_table_from([
                ("exists", exists.into_lua(lua)?),
                ("is_file", is_file.into_lua(lua)?),
                ("is_dir", is_dir.into_lua(lua)?),
                ("size", size.into_lua(lua)?),
                ("is_readable", is_readable.into_lua(lua)?),
                ("is_writable", is_writable.into_lua(lua)?),
                ("is_executable", is_executable.into_lua(lua)?),
            ])
        })?
        .into_lua(lua)?;

    let mkdir_sync = lua
        .create_function(|_, path: String| -> mlua::Result<(bool, Option<String>)> {
            match std::fs::create_dir_all(&path) {
                Ok(_) => Ok((true, None)),
                Err(e) => Ok((false, Some(e.to_string()))),
            }
        })?
        .into_lua(lua)?;

    let tempfile_sync = lua
        .create_function(
            |_lua, opts: Option<LuaTable>| -> mlua::Result<(String, Option<String>)> {
                // Parse options
                let prefix: Option<String> = opts.as_ref().and_then(|t| t.get("prefix").ok());
                let suffix: Option<String> = opts.as_ref().and_then(|t| t.get("suffix").ok());
                let content: Option<String> = opts.as_ref().and_then(|t| t.get("content").ok());

                // Generate random filename
                use std::time::{SystemTime, UNIX_EPOCH};
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let random_part = format!("{:x}", timestamp);

                // Build filename
                let filename = match (prefix, suffix) {
                    (Some(p), Some(s)) => {
                        format!("{}{}.{}", p, &random_part[..8], s.trim_start_matches('.'))
                    }
                    (Some(p), None) => format!("{}{}", p, &random_part[..8]),
                    (None, Some(s)) => {
                        format!("{}.{}", &random_part[..8], s.trim_start_matches('.'))
                    }
                    (None, None) => format!("tmp.{}", &random_part[..8]),
                };

                // Get temp directory
                let temp_dir = std::env::temp_dir();
                let temp_path = temp_dir.join(&filename);

                // Create file with optional content
                match std::fs::write(&temp_path, content.as_deref().unwrap_or("")) {
                    Ok(_) => {
                        // Convert to string path
                        let path_str = temp_path.to_string_lossy().to_string();
                        Ok((path_str, None))
                    }
                    Err(e) => Ok((String::new(), Some(e.to_string()))),
                }
            },
        )?
        .into_lua(lua)?;

    let remove_sync = lua
        .create_function(|_, path: String| -> mlua::Result<(bool, Option<String>)> {
            let path_obj = std::path::Path::new(&path);

            let result = if path_obj.is_dir() {
                // Remove directory and its contents recursively
                std::fs::remove_dir_all(&path)
            } else {
                // Remove file
                std::fs::remove_file(&path)
            };

            match result {
                Ok(_) => Ok((true, None)),
                Err(e) => Ok((false, Some(e.to_string()))),
            }
        })?
        .into_lua(lua)?;

    lua.create_table_from([
        ("read_dir_sync", read_dir_sync),
        ("read_file", read_file),
        ("read_file_sync", read_file_sync),
        ("write_file_sync", write_file_sync),
        ("stat", stat_sync),
        ("mkdir", mkdir_sync),
        ("tempfile", tempfile_sync),
        ("remove", remove_sync),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tempfile_generation() {
        use mlua::Lua;
        use std::path::Path;

        let lua = Lua::new();

        // Test basic tempfile (no options)
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(LuaNil).unwrap();
            assert!(err.is_none(), "Basic tempfile should not error");
            assert!(
                path.contains("/tmp/") || path.contains("/T/"),
                "Path should be in temp dir"
            );
            assert!(Path::new(&path).exists(), "File should exist");
            std::fs::remove_file(&path).ok();
        }

        // Test with prefix
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let opts = lua.create_table().unwrap();
            opts.set("prefix", "test").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(opts.clone()).unwrap();
            assert!(err.is_none(), "Tempfile with prefix should not error");
            assert!(path.contains("test"), "Path should contain prefix");
            assert!(Path::new(&path).exists(), "File should exist");
            std::fs::remove_file(&path).ok();
        }

        // Test with suffix
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let opts = lua.create_table().unwrap();
            opts.set("suffix", ".log").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(opts.clone()).unwrap();
            assert!(err.is_none(), "Tempfile with suffix should not error");
            assert!(path.contains(".log"), "Path should contain .log extension");
            assert!(Path::new(&path).exists(), "File should exist");
            std::fs::remove_file(&path).ok();
        }

        // Test with both prefix and suffix
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let opts = lua.create_table().unwrap();
            opts.set("prefix", "memo").unwrap();
            opts.set("suffix", ".md").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(opts.clone()).unwrap();
            assert!(err.is_none(), "Tempfile with both options should not error");
            assert!(path.contains("memo"), "Path should contain prefix");
            assert!(path.contains(".md"), "Path should contain .md extension");
            assert!(Path::new(&path).exists(), "File should exist");
            std::fs::remove_file(&path).ok();
        }

        // Test with content
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let opts = lua.create_table().unwrap();
            opts.set("content", "hello world").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(opts.clone()).unwrap();
            assert!(err.is_none(), "Tempfile with content should not error");
            assert!(Path::new(&path).exists(), "File should exist");

            // Verify content
            let content = std::fs::read_to_string(&path).unwrap();
            assert_eq!(
                content, "hello world",
                "File should contain the specified content"
            );
            std::fs::remove_file(&path).ok();
        }

        // Test with prefix, suffix and content
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let opts = lua.create_table().unwrap();
            opts.set("prefix", "test").unwrap();
            opts.set("suffix", ".txt").unwrap();
            opts.set("content", "# Test File\nContent here").unwrap();
            let (path, err): (String, Option<String>) = tempfile.call(opts.clone()).unwrap();
            assert!(err.is_none(), "Tempfile with all options should not error");
            assert!(path.contains("test"), "Path should contain prefix");
            assert!(path.contains(".txt"), "Path should contain extension");
            assert!(Path::new(&path).exists(), "File should exist");

            // Verify content
            let content = std::fs::read_to_string(&path).unwrap();
            assert_eq!(
                content, "# Test File\nContent here",
                "File should contain the specified content"
            );
            std::fs::remove_file(&path).ok();
        }

        // Test remove file
        {
            let table = new_table(&lua).unwrap();
            let tempfile: mlua::Function = table.get("tempfile").unwrap();
            let remove: mlua::Function = table.get("remove").unwrap();
            let (path, _): (String, Option<String>) = tempfile.call(LuaNil).unwrap();
            assert!(
                Path::new(&path).exists(),
                "File should exist before removal"
            );

            let (ok, err): (bool, Option<String>) = remove.call(path.clone()).unwrap();
            assert!(ok, "Remove file should succeed");
            assert!(err.is_none(), "Remove file should not error");
            assert!(
                !Path::new(&path).exists(),
                "File should not exist after removal"
            );
        }

        // Test remove directory
        {
            let table = new_table(&lua).unwrap();
            let mkdir: mlua::Function = table.get("mkdir").unwrap();
            let remove: mlua::Function = table.get("remove").unwrap();

            let temp_dir = std::env::temp_dir();
            let test_dir = temp_dir.join("test_remove_dir");
            let test_file = test_dir.join("nested_file.txt");

            // Create directory and nested file
            let _: (bool, Option<String>) =
                mkdir.call(test_dir.to_string_lossy().to_string()).unwrap();
            std::fs::write(&test_file, "test content").unwrap();
            assert!(test_dir.exists(), "Directory should exist");
            assert!(test_file.exists(), "Nested file should exist");

            // Remove directory recursively
            let (ok, err): (bool, Option<String>) =
                remove.call(test_dir.to_string_lossy().to_string()).unwrap();
            assert!(ok, "Remove directory should succeed");
            assert!(err.is_none(), "Remove directory should not error");
            assert!(
                !test_dir.exists(),
                "Directory should not exist after removal"
            );
        }
    }
}
