use crate::{plugin, Event};
use mlua::prelude::*;
use reqwest::header::HeaderMap;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::spawn_local;

/// Global HTTP client (singleton)
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Get or create the global HTTP client
fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Execute an HTTP request and call the Lua callback with the response
async fn execute_request(
    client: &reqwest::Client,
    method: &str,
    url: String,
    headers: Option<HeaderMap>,
    body: Option<String>,
) -> (bool, u16, Vec<u8>, HeaderMap, Option<String>) {
    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => {
            return (
                false,
                0,
                Vec::new(),
                HeaderMap::new(),
                Some(format!("Invalid method: {}", method)),
            )
        }
    };

    // Add headers if provided
    if let Some(h) = headers {
        request = request.headers(h);
    }

    // Add body if provided
    if let Some(b) = body {
        request = request.body(b);
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let response_headers = response.headers().clone();
            match response.bytes().await {
                Ok(body) => (true, status, body.to_vec(), response_headers, None),
                Err(e) => (false, 0, Vec::new(), HeaderMap::new(), Some(e.to_string())),
            }
        }
        Err(e) => (false, 0, Vec::new(), HeaderMap::new(), Some(e.to_string())),
    }
}

/// Convert HeaderMap to Lua table
fn headers_to_lua(lua: &Lua, headers: &HeaderMap) -> mlua::Result<LuaTable> {
    let table = lua.create_table()?;
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            table.set(name.as_str(), value_str)?;
        }
    }
    Ok(table)
}

/// Create a response table for Lua
fn create_response_table(
    lua: &Lua,
    success: bool,
    status: u16,
    body: Vec<u8>,
    headers: &HeaderMap,
    error: Option<String>,
) -> mlua::Result<LuaTable> {
    let response = lua.create_table()?;
    response.set("success", success)?;
    response.set("status", status)?;
    response.set("body", lua.create_string(&body)?)?;
    response.set("headers", headers_to_lua(lua, headers)?)?;
    if let Some(err) = error {
        response.set("error", err)?;
    } else {
        response.set("error", LuaNil)?;
    }
    Ok(response)
}

/// GET request
fn get_fn(lua: &Lua, (url, callback): (String, LuaFunction)) -> mlua::Result<()> {
    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        let client = http_client();
        let (success, status, body, headers, error) =
            execute_request(client, "GET", url, None, None).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response = create_response_table(lua, success, status, body, &headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// POST request
fn post_fn(lua: &Lua, (url, body, callback): (String, String, LuaFunction)) -> mlua::Result<()> {
    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        let client = http_client();
        let (success, status, resp_body, headers, error) =
            execute_request(client, "POST", url, None, Some(body)).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response =
                    create_response_table(lua, success, status, resp_body, &headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// PUT request
fn put_fn(lua: &Lua, (url, body, callback): (String, String, LuaFunction)) -> mlua::Result<()> {
    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        let client = http_client();
        let (success, status, resp_body, headers, error) =
            execute_request(client, "PUT", url, None, Some(body)).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response =
                    create_response_table(lua, success, status, resp_body, &headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// DELETE request
fn delete_fn(lua: &Lua, (url, callback): (String, LuaFunction)) -> mlua::Result<()> {
    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        let client = http_client();
        let (success, status, body, headers, error) =
            execute_request(client, "DELETE", url, None, None).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response = create_response_table(lua, success, status, body, &headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// PATCH request
fn patch_fn(lua: &Lua, (url, body, callback): (String, String, LuaFunction)) -> mlua::Result<()> {
    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        let client = http_client();
        let (success, status, resp_body, headers, error) =
            execute_request(client, "PATCH", url, None, Some(body)).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response =
                    create_response_table(lua, success, status, resp_body, &headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// Generic request function with options
fn request_fn(lua: &Lua, (opts, callback): (LuaTable, LuaFunction)) -> mlua::Result<()> {
    let url: String = opts.get("url")?;
    let method: String = opts.get("method").unwrap_or_else(|_| "GET".to_string());
    let timeout: Option<u64> = opts.get("timeout").ok();

    // Parse headers
    let mut headers = HeaderMap::new();
    if let Ok(headers_table) = opts.get::<LuaTable>("headers") {
        for pair in headers_table.pairs::<LuaValue, LuaValue>() {
            let (name, value) = pair?;
            if let (LuaValue::String(name), LuaValue::String(value)) = (name, value) {
                let name_str: String = name.to_str()?.to_owned();
                let value_str: String = value.to_str()?.to_owned();
                if let Ok(header_name) =
                    reqwest::header::HeaderName::from_bytes(name_str.as_bytes())
                {
                    if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&value_str) {
                        headers.insert(header_name, header_value);
                    }
                }
            }
        }
    }

    // Parse body
    let body: Option<String> = opts.get("body").ok();

    let sender = plugin::clone_sender(lua)?;

    spawn_local(async move {
        // Build client with custom timeout if specified
        let client = if let Some(timeout_ms) = timeout {
            reqwest::Client::builder()
                .timeout(Duration::from_millis(timeout_ms))
                .build()
                .unwrap_or_else(|_| http_client().clone())
        } else {
            http_client().clone()
        };

        let (success, status, resp_body, resp_headers, error) =
            execute_request(&client, &method, url, Some(headers), body).await;

        sender
            .send(Event::LuaCallback(Box::new(move |lua| {
                let response =
                    create_response_table(lua, success, status, resp_body, &resp_headers, error)?;
                callback.call(response)
            })))
            .unwrap();
    });

    Ok(())
}

/// Create the deck.http table
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    let get = lua.create_function(get_fn)?.into_lua(lua)?;
    let post = lua.create_function(post_fn)?.into_lua(lua)?;
    let put = lua.create_function(put_fn)?.into_lua(lua)?;
    let delete = lua.create_function(delete_fn)?.into_lua(lua)?;
    let patch = lua.create_function(patch_fn)?.into_lua(lua)?;
    let request = lua.create_function(request_fn)?.into_lua(lua)?;

    lua.create_table_from([
        ("get", get),
        ("post", post),
        ("put", put),
        ("delete", delete),
        ("patch", patch),
        ("request", request),
    ])
}
