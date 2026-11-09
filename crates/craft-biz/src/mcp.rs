//! The one MCP `2024-11-05` stdio loop shared by every back-office craft app.
//!
//! Invariants (RUST_ARTCRAFT_STANDARD gate 4):
//! - stdout carries only line-delimited JSON-RPC; diagnostics go to stderr.
//! - Notifications (no `id`) never get a response.
//! - `handle_line` is a pure function so the protocol is unit-tested without
//!   spawning processes.
//! - Tool names are `app_verb`; dotted aliases (`app.verb`) are accepted.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2024-11-05";
/// Longest request line accepted. Hostile-input fence.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// What an app implements to be served over MCP.
pub trait ToolHost {
    fn server_name(&self) -> &str;
    fn server_version(&self) -> &str;
    /// Tool descriptors (`name`, `description`, `inputSchema`). Build with [`tool`].
    fn tools(&self) -> Vec<Value>;
    /// Run a tool. `Ok` value is returned as JSON text; `Err` becomes `isError: true`.
    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String>;
}

/// Build a tool descriptor.
pub fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}

/// `buch.entry_book` -> `buch_entry_book`.
pub fn normalize_tool_name(name: &str) -> String {
    name.replace('.', "_")
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn ok_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Handle one request line. Returns the response, or `None` for notifications
/// and blank lines.
pub fn handle_line<H: ToolHost>(host: &mut H, line: &str) -> Option<Value> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() > MAX_LINE_BYTES {
        return Some(error_response(Value::Null, -32600, "request too large"));
    }
    let req: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Some(error_response(Value::Null, -32700, "parse error")),
    };
    if !req.is_object() {
        return Some(error_response(
            Value::Null,
            -32600,
            "request must be a JSON object",
        ));
    }
    let id = match req.get("id") {
        Some(v) if !v.is_null() => v.clone(),
        // Notifications (initialized, cancelled, ...) get no reply.
        _ => return None,
    };
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Null);

    Some(match method {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": host.server_name(), "version": host.server_version() }
            }),
        ),
        "ping" => ok_response(id, json!({})),
        "tools/list" => ok_response(id, json!({ "tools": host.tools() })),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error_response(id, -32602, "tools/call needs params.name"));
            };
            let name = normalize_tool_name(name);
            let known = host
                .tools()
                .iter()
                .any(|t| t.get("name").and_then(Value::as_str) == Some(name.as_str()));
            if !known {
                return Some(error_response(id, -32602, &format!("unknown tool {name}")));
            }
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return Some(error_response(id, -32602, "arguments must be an object"));
            }
            let (text, is_error) = match host.call(&name, &args) {
                Ok(v) => (
                    serde_json::to_string_pretty(&v).unwrap_or_else(|e| e.to_string()),
                    false,
                ),
                Err(e) => (json!({ "error": e }).to_string(), true),
            };
            ok_response(
                id,
                json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
            )
        }
        other => error_response(id, -32601, &format!("method not found: {other}")),
    })
}

/// Serve `host` until `reader` closes. Each response is one line on `writer`.
pub fn run_stdio<H: ToolHost, R: BufRead, W: Write>(
    host: &mut H,
    reader: R,
    mut writer: W,
) -> std::io::Result<()> {
    eprintln!(
        "[{}] MCP {PROTOCOL_VERSION} stdio ready",
        host.server_name()
    );
    for line in reader.lines() {
        let line = line?;
        if let Some(resp) = handle_line(host, &line) {
            let s = serde_json::to_string(&resp).unwrap_or_else(|e| {
                format!("{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32603,\"message\":\"{e}\"}}}}")
            });
            writer.write_all(s.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Convenience: serve on the process's real stdin/stdout.
pub fn serve_stdio<H: ToolHost>(host: &mut H) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_stdio(host, stdin.lock(), stdout.lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl ToolHost for Echo {
        fn server_name(&self) -> &str {
            "echo-test"
        }
        fn server_version(&self) -> &str {
            "0.0.1"
        }
        fn tools(&self) -> Vec<Value> {
            vec![
                tool(
                    "echo_say",
                    "Echo text",
                    json!({ "text": { "type": "string" } }),
                    &["text"],
                ),
                tool("echo_fail", "Always fails", json!({}), &[]),
            ]
        }
        fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
            match name {
                "echo_say" => {
                    Ok(json!({ "said": args.get("text").cloned().unwrap_or(Value::Null) }))
                }
                _ => Err("nope".into()),
            }
        }
    }

    fn call(line: &str) -> Option<Value> {
        handle_line(&mut Echo, line)
    }

    #[test]
    fn initialize_list_call_probe() {
        let init = call(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(init["result"]["serverInfo"]["name"], "echo-test");
        assert!(call(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let list = call(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert_eq!(list["result"]["tools"].as_array().map(Vec::len), Some(2));
        let res = call(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo.say","arguments":{"text":"hi"}}}"#).unwrap();
        assert_eq!(res["result"]["isError"], false);
        let text = res["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(serde_json::from_str::<Value>(text).unwrap()["said"], "hi");
    }

    #[test]
    fn errors_are_structured() {
        assert_eq!(call("{oops").unwrap()["error"]["code"], -32700);
        assert_eq!(call("[1,2]").unwrap()["error"]["code"], -32600);
        assert_eq!(
            call(r#"{"jsonrpc":"2.0","id":4,"method":"nope"}"#).unwrap()["error"]["code"],
            -32601
        );
        assert_eq!(
            call(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"missing"}}"#)
                .unwrap()["error"]["code"],
            -32602
        );
        assert_eq!(
            call(r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"echo_say","arguments":[1]}}"#).unwrap()["error"]["code"],
            -32602
        );
        let failed =
            call(r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"echo_fail"}}"#)
                .unwrap();
        assert_eq!(failed["result"]["isError"], true);
        assert!(call("   ").is_none());
    }

    #[test]
    fn run_stdio_writes_one_line_per_response() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n";
        let mut out = Vec::new();
        run_stdio(&mut Echo, &input[..], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        for l in lines {
            assert!(serde_json::from_str::<Value>(l).is_ok());
        }
    }
}
