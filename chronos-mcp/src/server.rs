//! # MCP Server — JSON-RPC 2.0 stdin/stdout handler
//!
//! Implements the MCP protocol loop:
//! 1. Read a JSON-RPC request from stdin (one per line)
//! 2. Route to the appropriate handler
//! 3. Write the JSON-RPC response to stdout (one per line)
//!
//! Compatible with ALL MCP-capable AI clients:
//! Antigravity, Cursor, Codex CLI, OpenCode, Claude Desktop,
//! Cline, Windsurf, Continue, Zed.

use crate::protocol::{InitializeResult, JsonRpcRequest, JsonRpcResponse, ServerCapabilities, ServerInfo, ToolsCapability};
use crate::registry::McpToolRegistry;
use crate::state::SharedState;
use anyhow::Result;
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Negotiate MCP `protocolVersion` against supported spec revisions.
fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    if let Some(req_ver) = requested {
        for &supported in SUPPORTED_PROTOCOL_VERSIONS {
            if supported == req_ver {
                return supported;
            }
        }
    }
    SUPPORTED_PROTOCOL_VERSIONS[0]
}

fn percent_decode_utf8(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(val) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

/// Decode `file://` URIs (including Windows drive prefixes and `%20` escapes) into a local `PathBuf`.
pub(crate) fn parse_workspace_root_uri(raw: &str) -> PathBuf {
    let decoded = if let Some(rest) = raw
        .strip_prefix("file://localhost/")
        .map(|s| format!("/{s}"))
        .or_else(|| raw.strip_prefix("file://").map(ToString::to_string))
    {
        let mut decoded = percent_decode_utf8(&rest);
        // On `file:///C:/foo`, strip the leading slash before `<drive>:` so `PathBuf` treats it as native Windows drive path.
        let b = decoded.as_bytes();
        if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            decoded.remove(0);
        }
        decoded
    } else {
        raw.to_string()
    };
    PathBuf::from(decoded)
}

/// Main server loop — reads from stdin, writes to stdout.
/// This is the entry point called from `main.rs`.
pub async fn run_server(state: SharedState) -> Result<()> {
    info!("🚀 CHRONOS MCP Server started — listening on stdin");
    info!("   Compatible: Antigravity | Cursor | Codex | OpenCode | Claude | Cline | Windsurf | Continue | Zed");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_) => continue, // skip blank lines
            Err(e) => {
                error!("stdin read error: {}", e);
                break;
            }
        };

        debug!("← IN:  {}", &line[..line.len().min(200)]);

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse::err(None, -32700, format!("Parse error: {}", e));
                write_response(&mut stdout_lock, &err_resp);
                continue;
            }
        };

        if let Some(response) = handle_request(request, state.clone()).await {
            debug!("→ OUT: {}", serde_json::to_string(&response).unwrap_or_default().chars().take(200).collect::<String>());
            write_response(&mut stdout_lock, &response);
        }
    }

    info!("CHRONOS MCP Server shutting down (stdin closed)");
    Ok(())
}

/// Route a JSON-RPC request or notification to the appropriate handler.
/// Returns `None` for notifications (`req.id.is_none()`) per JSON-RPC 2.0.
async fn handle_request(req: JsonRpcRequest, state: SharedState) -> Option<JsonRpcResponse> {
    let id = req.id.clone();

    match req.method.as_str() {
        // ── MCP Lifecycle ────────────────────────────────────────────
        "initialize" => {
            let root_uri = req.params.as_ref().and_then(|p| {
                p.get("workspaceFolders")
                    .and_then(|folders| folders.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|first| first.get("uri"))
                    .or_else(|| p.get("rootUri"))
                    .or_else(|| p.get("rootPath"))
            });
            let workspace_root = root_uri.and_then(|v| v.as_str()).map(parse_workspace_root_uri);

            if let Some(root) = workspace_root {
                let mut st = state.write().await;
                if let Err(e) = st.session.rebind_workspace(root) {
                    warn!("Workspace rebind failed: {}", e);
                } else {
                    info!("Workspace root bound to: {}", st.workspace_root.display());
                }
            }

            let requested_version = req
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str());

            let result = InitializeResult {
                protocol_version: negotiate_protocol_version(requested_version),
                capabilities: ServerCapabilities {
                    tools: ToolsCapability { list_changed: false },
                },
                server_info: ServerInfo {
                    name: "chronos-agent",
                    version: env!("CARGO_PKG_VERSION"),
                },
            };
            Some(JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap()))
        }

        "initialized" | "notifications/initialized" | "notifications/cancelled" => {
            info!("MCP notification received: {}", req.method);
            if id.is_some() {
                Some(JsonRpcResponse::ok(id, json!({})))
            } else {
                None
            }
        }

        "ping" => Some(JsonRpcResponse::ok(id, json!({}))),

        // ── Tools ────────────────────────────────────────────────────
        "tools/list" => {
            let readonly = state.read().await.readonly;
            let tool_list = McpToolRegistry::list_tools(readonly);
            Some(JsonRpcResponse::ok(id, json!({ "tools": tool_list })))
        }

        "tools/call" => {
            let tool_name = req.params.as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let tool_params = req.params.as_ref()
                .and_then(|p| p.get("arguments"))
                .cloned();

            let resp = match tool_name.as_deref() {
                Some(name) => match McpToolRegistry::dispatch(state, name, tool_params).await {
                    Ok(result) => JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap()),
                    Err(err) => JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: None,
                        error: Some(err),
                    },
                },
                None => {
                    JsonRpcResponse::err(id, -32602, "Missing tool name in tools/call params")
                }
            };
            Some(resp)
        }

        // ── Unknown method ───────────────────────────────────────────
        unknown => {
            if id.is_none() {
                debug!("Ignoring unknown JSON-RPC notification: {}", unknown);
                None
            } else {
                warn!("Unknown method: {}", unknown);
                Some(JsonRpcResponse::err(
                    id,
                    -32601,
                    format!("Method not found: {}", unknown),
                ))
            }
        }
    }
}

fn write_response(stdout: &mut impl Write, response: &JsonRpcResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{}", json);
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::new_shared_state_with_options;

    #[test]
    fn test_parse_workspace_root_uri_handles_windows_unix_and_escapes() {
        assert_eq!(
            parse_workspace_root_uri("file:///C:/Users/PT/My%20Repo"),
            PathBuf::from("C:/Users/PT/My Repo")
        );
        assert_eq!(
            parse_workspace_root_uri("file://localhost/D:/Work/chronos-agent"),
            PathBuf::from("D:/Work/chronos-agent")
        );
        assert_eq!(
            parse_workspace_root_uri("file:///home/dev/my%20workspace"),
            PathBuf::from("/home/dev/my workspace")
        );
        assert_eq!(
            parse_workspace_root_uri("./local-dir"),
            PathBuf::from("./local-dir")
        );
    }

    #[tokio::test]
    async fn test_notifications_emit_no_response_and_initialize_negotiates_version() {
        let tmp = std::env::temp_dir().join(format!(
            "chronos_srv_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let state = new_shared_state_with_options(tmp.clone(), false);

        let notif = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        assert!(handle_request(notif, state.clone()).await.is_none());

        let unknown_notif = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/cancelled".to_string(),
            params: None,
        };
        assert!(handle_request(unknown_notif, state.clone()).await.is_none());

        let init_2025 = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2025-11-25",
                "rootUri": format!("file:///{}", tmp.to_string_lossy().replace('\\', "/").trim_start_matches('/'))
            })),
        };
        let resp = handle_request(init_2025, state.clone()).await.unwrap();
        assert_eq!(resp.result.unwrap()["protocolVersion"], "2025-11-25");

        let init_legacy = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "initialize".to_string(),
            params: Some(json!({ "protocolVersion": "2024-11-05" })),
        };
        let resp_legacy = handle_request(init_legacy, state).await.unwrap();
        assert_eq!(resp_legacy.result.unwrap()["protocolVersion"], "2024-11-05");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
