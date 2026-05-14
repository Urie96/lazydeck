use crate::{plugin, Event};
use mlua::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::task::JoinHandle;

enum SocketCommand {
    Write(String),
    Close,
}

struct SocketInner {
    sender: Mutex<Option<UnboundedSender<SocketCommand>>>,
    read_task: Mutex<Option<JoinHandle<()>>>,
    write_task: Mutex<Option<JoinHandle<()>>>,
    on_line: Arc<Mutex<Option<LuaFunction>>>,
}

impl SocketInner {
    fn close(&self) {
        if let Some(sender) = self.sender.lock().unwrap().take() {
            let _ = sender.send(SocketCommand::Close);
        }

        if let Some(task) = self.read_task.lock().unwrap().take() {
            task.abort();
        }
        self.write_task.lock().unwrap().take();
    }
}

impl Drop for SocketInner {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Clone)]
struct LuaSocket {
    inner: Arc<SocketInner>,
}

impl LuaSocket {
    fn connect(lua: &Lua, addr: String) -> mlua::Result<Self> {
        let path = parse_socket_addr(&addr)?;
        let stream = std::os::unix::net::UnixStream::connect(&path).map_err(|e| {
            LuaError::RuntimeError(format!("Failed to connect to socket '{}': {}", path, e))
        })?;
        stream.set_nonblocking(true).map_err(|e| {
            LuaError::RuntimeError(format!(
                "Failed to set socket nonblocking '{}': {}",
                path, e
            ))
        })?;
        let stream = UnixStream::from_std(stream).map_err(|e| {
            LuaError::RuntimeError(format!("Failed to use socket '{}' with tokio: {}", path, e))
        })?;

        let sender = plugin::clone_sender(lua)?;
        let (reader_half, mut writer_half) = stream.into_split();
        let (tx, mut rx) = unbounded_channel::<SocketCommand>();
        let on_line = Arc::new(Mutex::new(None::<LuaFunction>));
        let on_line_reader = Arc::clone(&on_line);

        let read_task = tokio::task::spawn_local(async move {
            let mut reader = BufReader::new(reader_half);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let callback = on_line_reader.lock().unwrap().clone();
                        if let Some(callback) = callback {
                            let line = line.trim_end_matches(['\r', '\n']).to_string();
                            let _ = sender.send(Event::LuaCallback(Box::new(move |_| {
                                callback.call::<()>(line)
                            })));
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let write_task = tokio::task::spawn_local(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    SocketCommand::Write(message) => {
                        if writer_half.write_all(message.as_bytes()).await.is_err() {
                            break;
                        }
                        if !message.ends_with('\n') && writer_half.write_all(b"\n").await.is_err() {
                            break;
                        }
                        if writer_half.flush().await.is_err() {
                            break;
                        }
                    }
                    SocketCommand::Close => {
                        let _ = writer_half.shutdown().await;
                        break;
                    }
                }
            }
        });

        Ok(Self {
            inner: Arc::new(SocketInner {
                sender: Mutex::new(Some(tx)),
                read_task: Mutex::new(Some(read_task)),
                write_task: Mutex::new(Some(write_task)),
                on_line,
            }),
        })
    }

    fn set_on_line(&self, callback: LuaFunction) {
        *self.inner.on_line.lock().unwrap() = Some(callback);
    }

    fn write(&self, message: String) -> mlua::Result<()> {
        let sender = self
            .inner
            .sender
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| LuaError::RuntimeError("Socket is already closed".to_string()))?;

        sender
            .send(SocketCommand::Write(message))
            .map_err(|_| LuaError::RuntimeError("Socket writer is no longer available".to_string()))
    }

    fn close(&self) {
        self.inner.close();
    }
}

impl LuaUserData for LuaSocket {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("on_line", |_, this, callback: LuaFunction| {
            this.set_on_line(callback);
            Ok(())
        });

        methods.add_method("write", |_, this, message: String| this.write(message));

        methods.add_method("close", |_, this, ()| {
            this.close();
            Ok(())
        });
    }
}

fn parse_socket_addr(addr: &str) -> mlua::Result<String> {
    if let Some(path) = addr.strip_prefix("unix:") {
        if path.is_empty() {
            return Err(LuaError::RuntimeError(
                "Socket address cannot be empty".to_string(),
            ));
        }
        return Ok(path.to_string());
    }

    if addr.contains(':') {
        return Err(LuaError::RuntimeError(format!(
            "Unsupported socket address '{}'",
            addr
        )));
    }

    if addr.is_empty() {
        return Err(LuaError::RuntimeError(
            "Socket address cannot be empty".to_string(),
        ));
    }

    Ok(addr.to_string())
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    lua.create_table_from([(
        "connect",
        lua.create_function(|lua, addr: String| {
            lua.create_userdata(LuaSocket::connect(lua, addr)?)
        })?,
    )])
}
