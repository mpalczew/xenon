//! MCP (JSON-RPC 2.0) message handling for the IDE server. See PROTOCOL.md for
//! the captured VSCode protocol this mirrors.

use std::path::PathBuf;

use async_channel::Sender;
use serde_json::{Value, json};

use crate::IdeCommand;

/// Handle one incoming JSON-RPC message; return a reply to send, or `None` for
/// notifications that need no response.
pub fn handle(text: &str, roots: &[PathBuf], commands: &Sender<IdeCommand>) -> Option<String> {
    let message: Value = serde_json::from_str(text).ok()?;
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str)?;

    let result = match method {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": true } },
            "serverInfo": { "name": "xenon", "version": env!("CARGO_PKG_VERSION") },
        })),
        "notifications/initialized" => return None,
        "tools/list" => Some(json!({ "tools": tool_list() })),
        "tools/call" => Some(call_tool(&message, roots, commands)),
        _ => None,
    };

    let id = id?; // notifications (no id) get no reply
    let reply = match result {
        Some(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        None => json!({
            "jsonrpc": "2.0", "id": id,
            "error": { "code": -32601, "message": format!("method not found: {method}") },
        }),
    };
    Some(reply.to_string())
}

/// The tools xero advertises. Schemas mirror VSCode's; only `openFile` and
/// `getWorkspaceFolders` do real work in the MVP.
fn tool_list() -> Value {
    let obj = || json!({ "type": "object", "properties": {} });
    json!([
        { "name": "openFile", "description": "Open a file in the editor",
          "inputSchema": { "type": "object",
            "properties": { "filePath": { "type": "string" } },
            "required": ["filePath"] } },
        { "name": "getWorkspaceFolders", "description": "List workspace folders",
          "inputSchema": obj() },
        { "name": "getCurrentSelection", "description": "Active editor selection",
          "inputSchema": obj() },
        { "name": "getOpenEditors", "description": "List open editors",
          "inputSchema": obj() },
        { "name": "getDiagnostics", "description": "Language diagnostics",
          "inputSchema": { "type": "object",
            "properties": { "uri": { "type": "string" } } } },
    ])
}

fn call_tool(message: &Value, roots: &[PathBuf], commands: &Sender<IdeCommand>) -> Value {
    let params = message.get("params");
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let args = params.and_then(|p| p.get("arguments"));

    match name {
        "openFile" => {
            let path = args
                .and_then(|a| a.get("filePath").or_else(|| a.get("path")))
                .and_then(Value::as_str);
            match path {
                Some(path) => {
                    let _ = commands.send_blocking(IdeCommand::OpenFile(PathBuf::from(path)));
                    text_result(&format!("Opened {path}"))
                }
                None => text_result("openFile: missing filePath"),
            }
        }
        "getWorkspaceFolders" => {
            let folders: Vec<&str> = roots.iter().filter_map(|r| r.to_str()).collect();
            text_result(&serde_json::to_string(&folders).unwrap_or_default())
        }
        // Stubs: valid empty responses keep the CLI happy in the MVP.
        "getCurrentSelection" | "getOpenEditors" | "getDiagnostics" => text_result("[]"),
        other => text_result(&format!("unsupported tool: {other}")),
    }
}

fn text_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn request(method: &str, params: Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        })
        .to_string()
    }

    #[test]
    fn initialize_returns_server_info() {
        let (tx, _rx) = async_channel::unbounded();
        let reply = handle(&request("initialize", json!({})), &[], &tx).expect("reply");
        let value: Value = serde_json::from_str(&reply).expect("json");

        assert_eq!(value["result"]["serverInfo"]["name"], "xenon");
        assert_eq!(
            value["result"]["capabilities"]["tools"]["listChanged"],
            true
        );
    }

    #[test]
    fn tools_list_advertises_open_file() {
        let (tx, _rx) = async_channel::unbounded();
        let reply = handle(&request("tools/list", json!({})), &[], &tx).expect("reply");
        let value: Value = serde_json::from_str(&reply).expect("json");
        let tools = value["result"]["tools"].as_array().expect("tools");

        assert!(tools.iter().any(|tool| tool["name"] == "openFile"));
    }

    #[test]
    fn open_file_sends_command() {
        let (tx, rx) = async_channel::unbounded();
        let params = json!({
            "name": "openFile",
            "arguments": { "filePath": "/tmp/example.rs" },
        });

        let reply = handle(&request("tools/call", params), &[], &tx).expect("reply");
        let command = rx.try_recv().expect("command");

        assert!(reply.contains("Opened /tmp/example.rs"));
        assert!(
            matches!(command, IdeCommand::OpenFile(path) if path.as_path() == Path::new("/tmp/example.rs"))
        );
    }
}
