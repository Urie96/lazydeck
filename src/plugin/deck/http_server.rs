use crate::{plugin, Event};
use mlua::prelude::*;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use std::collections::HashMap;
use std::io;
use std::net::TcpListener as StdTcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::spawn_local;

const MAX_REQUEST_SIZE: usize = 64 * 1024;
const RESOLVER_PATH_PREFIX: &str = "/r/";
const LUA_RESOLVER_REGISTRY_KEY: &str = "http_server_resolvers";
const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b'?')
    .add(b'=')
    .add(b'/');

#[derive(Clone)]
struct ServerInfo {
    host: String,
    port: u16,
}

impl ServerInfo {
    fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

struct ServerState {
    info: Option<ServerInfo>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self { info: None }
    }
}

#[derive(Debug, Clone)]
struct HttpRequestData {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct HttpResponseData {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

static SERVER_STATE: OnceLock<Mutex<ServerState>> = OnceLock::new();

fn server_state() -> &'static Mutex<ServerState> {
    SERVER_STATE.get_or_init(|| Mutex::new(ServerState::default()))
}

fn lock_server_state() -> mlua::Result<std::sync::MutexGuard<'static, ServerState>> {
    server_state()
        .lock()
        .map_err(|_| LuaError::RuntimeError("http_server state lock poisoned".to_string()))
}

fn decode_component(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().to_string()
}

fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

fn ensure_registry_table(lua: &Lua) -> mlua::Result<LuaTable> {
    match lua.named_registry_value(LUA_RESOLVER_REGISTRY_KEY) {
        Ok(tbl) => Ok(tbl),
        Err(_) => {
            let tbl = lua.create_table()?;
            lua.set_named_registry_value(LUA_RESOLVER_REGISTRY_KEY, tbl.clone())?;
            Ok(tbl)
        }
    }
}

fn ensure_server_started(lua: &Lua) -> mlua::Result<ServerInfo> {
    {
        let state = lock_server_state()?;
        if let Some(info) = state.info.clone() {
            return Ok(info);
        }
    }

    let host = "127.0.0.1".to_string();
    let listener = StdTcpListener::bind((host.as_str(), 0))
        .map_err(|err| LuaError::RuntimeError(format!("failed to bind http_server: {}", err)))?;
    listener.set_nonblocking(true).map_err(|err| {
        LuaError::RuntimeError(format!("failed to configure http_server: {}", err))
    })?;
    let port = listener
        .local_addr()
        .map_err(|err| LuaError::RuntimeError(format!("failed to inspect http_server: {}", err)))?
        .port();
    let listener = TcpListener::from_std(listener).map_err(|err| {
        LuaError::RuntimeError(format!("failed to create tokio listener: {}", err))
    })?;

    let sender = plugin::clone_sender(lua)?;
    spawn_local(async move {
        accept_loop(listener, sender).await;
    });

    let info = ServerInfo { host, port };
    let mut state = lock_server_state()?;
    state.info = Some(info.clone());
    Ok(info)
}

async fn accept_loop(listener: TcpListener, sender: crate::events::EventSender) {
    loop {
        let accepted = listener.accept().await;
        let (stream, _) = match accepted {
            Ok(pair) => pair,
            Err(err) => {
                if err.kind() != io::ErrorKind::Interrupted {
                    eprintln!("http_server accept error: {}", err);
                }
                continue;
            }
        };

        let sender = sender.clone();
        spawn_local(async move {
            if let Err(err) = handle_connection(stream, sender).await {
                eprintln!("http_server connection error: {}", err);
            }
        });
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    sender: crate::events::EventSender,
) -> io::Result<()> {
    let request_bytes = read_http_request(&mut stream).await?;
    let response = match parse_request(&request_bytes) {
        Ok(request) => route_request(sender, request).await,
        Err(err) => simple_response(400, err),
    };
    write_response(&mut stream, response).await
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(buffer);
        }
        if buffer.len() > MAX_REQUEST_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request header too large",
            ));
        }
    }

    Ok(buffer)
}

fn parse_request(buffer: &[u8]) -> Result<HttpRequestData, String> {
    let text = std::str::from_utf8(buffer).map_err(|_| "request is not valid UTF-8".to_string())?;
    let header_end = text
        .find("\r\n\r\n")
        .ok_or_else(|| "incomplete http request".to_string())?;
    let header_text = &text[..header_end];
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?
        .to_string();
    let target = request_parts
        .next()
        .ok_or_else(|| "missing request target".to_string())?;
    let _version = request_parts
        .next()
        .ok_or_else(|| "missing http version".to_string())?;

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let path = decode_component(raw_path);
    let mut query = HashMap::new();
    if !raw_query.is_empty() {
        for pair in raw_query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            query.insert(decode_component(key), decode_component(value));
        }
    }

    Ok(HttpRequestData {
        method,
        path,
        query,
        headers,
    })
}

async fn route_request(
    sender: crate::events::EventSender,
    request: HttpRequestData,
) -> HttpResponseData {
    if request.path == "/health" {
        return simple_response(200, "ok");
    }

    let Some(raw_name) = request.path.strip_prefix(RESOLVER_PATH_PREFIX) else {
        return simple_response(404, "not found");
    };
    if raw_name.is_empty() {
        return simple_response(404, "resolver name is required");
    }
    let resolver_name = raw_name.to_string();

    let (tx, rx) = oneshot::channel::<HttpResponseData>();
    let send_result = sender.send(Event::LuaCallback(Box::new(move |lua| {
        invoke_resolver(lua, resolver_name, request, tx)
    })));
    if send_result.is_err() {
        return simple_response(500, "failed to dispatch resolver");
    }

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => response,
        Ok(Err(_)) => simple_response(500, "resolver dropped response"),
        Err(_) => simple_response(504, "resolver timed out"),
    }
}

fn invoke_resolver(
    lua: &Lua,
    resolver_name: String,
    request: HttpRequestData,
    tx: oneshot::Sender<HttpResponseData>,
) -> mlua::Result<()> {
    let resolvers = ensure_registry_table(lua)?;
    let resolver: Option<LuaFunction> = resolvers.get(resolver_name.clone()).ok();
    let Some(resolver) = resolver else {
        let _ = tx.send(simple_response(404, "resolver not found"));
        return Ok(());
    };

    let request_table = lua.create_table()?;
    request_table.set("method", request.method)?;
    request_table.set("path", request.path)?;

    let query_table = lua.create_table()?;
    for (key, value) in request.query.iter() {
        query_table.set(key.as_str(), value.as_str())?;
    }
    request_table.set("query", query_table.clone())?;
    request_table.set("params", query_table)?;

    let headers_table = lua.create_table()?;
    for (key, value) in request.headers.iter() {
        headers_table.set(key.as_str(), value.as_str())?;
    }
    request_table.set("headers", headers_table)?;

    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let respond_fn = {
        let tx = tx.clone();
        lua.create_function(move |_, response: LuaTable| {
            let response = parse_lua_response(response)?;
            if let Ok(mut guard) = tx.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(response);
                }
            }
            Ok(())
        })?
    };

    match resolver.call::<()>((request_table, respond_fn)) {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Ok(mut guard) = tx.lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(simple_response(500, err.to_string()));
                }
            }
            Ok(())
        }
    }
}

fn parse_lua_response(response: LuaTable) -> mlua::Result<HttpResponseData> {
    let status = response.get::<Option<u16>>("status")?.unwrap_or(200);
    let body = response
        .get::<Option<LuaString>>("body")?
        .map(|value| value.as_bytes().to_vec())
        .unwrap_or_default();

    let mut headers = Vec::new();
    if let Some(header_table) = response.get::<Option<LuaTable>>("headers")? {
        for pair in header_table.pairs::<String, String>() {
            let (key, value) = pair?;
            headers.push((key, value));
        }
    }

    Ok(HttpResponseData {
        status,
        headers,
        body,
    })
}

fn simple_response(status: u16, body: impl Into<String>) -> HttpResponseData {
    HttpResponseData {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        )],
        body: body.into().into_bytes(),
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        302 => "Found",
        307 => "Temporary Redirect",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    }
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    mut response: HttpResponseData,
) -> io::Result<()> {
    let has_content_length = response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-length"));
    let has_connection = response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("connection"));
    if !has_content_length {
        response.headers.push((
            "Content-Length".to_string(),
            response.body.len().to_string(),
        ));
    }
    if !has_connection {
        response
            .headers
            .push(("Connection".to_string(), "close".to_string()));
    }

    let mut data = Vec::new();
    data.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            response.status,
            reason_phrase(response.status)
        )
        .as_bytes(),
    );
    for (name, value) in response.headers {
        data.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
    }
    data.extend_from_slice(b"\r\n");
    data.extend_from_slice(&response.body);
    stream.write_all(&data).await?;
    stream.shutdown().await
}

fn build_query_string(params: Option<LuaTable>, lua: &Lua) -> mlua::Result<String> {
    let Some(params) = params else {
        return Ok(String::new());
    };

    let mut pairs = Vec::new();
    for pair in params.pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair?;
        let key = String::from_lua(key, lua)?;
        let value = match value {
            LuaValue::String(value) => value.to_string_lossy().to_string(),
            LuaValue::Integer(value) => value.to_string(),
            LuaValue::Number(value) => value.to_string(),
            LuaValue::Boolean(value) => {
                if value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            LuaValue::Nil => continue,
            _ => {
                return Err(LuaError::RuntimeError(
                    "http_server.url params only support string/number/boolean values".to_string(),
                ))
            }
        };
        pairs.push(format!(
            "{}={}",
            encode_component(&key),
            encode_component(&value)
        ));
    }
    Ok(pairs.join("&"))
}

pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let table = lua.create_table()?;

    table.set(
        "register_resolver",
        lua.create_function(|lua, (name, handler): (String, LuaFunction)| {
            if name.trim().is_empty() {
                return Err(LuaError::RuntimeError(
                    "http_server resolver name cannot be empty".to_string(),
                ));
            }
            ensure_server_started(lua)?;
            let resolvers = ensure_registry_table(lua)?;
            resolvers.set(name, handler)?;
            Ok(())
        })?,
    )?;

    table.set(
        "unregister_resolver",
        lua.create_function(|lua, name: String| {
            let resolvers = ensure_registry_table(lua)?;
            resolvers.set(name, LuaNil)?;
            Ok(())
        })?,
    )?;

    table.set(
        "url",
        lua.create_function(|lua, (name, params): (String, Option<LuaTable>)| {
            if name.trim().is_empty() {
                return Err(LuaError::RuntimeError(
                    "http_server resolver name cannot be empty".to_string(),
                ));
            }
            let info = ensure_server_started(lua)?;
            let mut url = format!("{}/r/{}", info.base_url(), encode_component(&name));
            let query = build_query_string(params, lua)?;
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }
            Ok(url)
        })?,
    )?;

    table.set(
        "info",
        lua.create_function(|lua, ()| {
            let info = ensure_server_started(lua)?;
            let payload = lua.create_table()?;
            payload.set("host", info.host.clone())?;
            payload.set("port", info.port)?;
            payload.set("base_url", info.base_url())?;
            Ok(payload)
        })?,
    )?;

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::{parse_request, simple_response};

    #[test]
    fn parse_request_decodes_path_and_query() {
        let request = parse_request(
            b"GET /r/netease-song?id=123&title=hello%20world HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/r/netease-song");
        assert_eq!(request.query.get("id").map(String::as_str), Some("123"));
        assert_eq!(
            request.query.get("title").map(String::as_str),
            Some("hello world")
        );
        assert_eq!(
            request.headers.get("host").map(String::as_str),
            Some("localhost")
        );
    }

    #[test]
    fn simple_response_sets_plain_text_body() {
        let response = simple_response(404, "missing");
        assert_eq!(response.status, 404);
        assert_eq!(response.body, b"missing");
        assert!(response
            .headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case("content-type")));
    }
}
