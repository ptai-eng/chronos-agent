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
use crate::state::SharedState;
use crate::tools;
use anyhow::Result;
use serde_json::json;
use std::io::{self, BufRead, Write};
use tracing::{debug, error, info, warn};

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

        let response = handle_request(request, state.clone()).await;

        debug!("→ OUT: {}", serde_json::to_string(&response).unwrap_or_default()[..].chars().take(200).collect::<String>());

        write_response(&mut stdout_lock, &response);
    }

    info!("CHRONOS MCP Server shutting down (stdin closed)");
    Ok(())
}

/// Route a JSON-RPC request to the appropriate handler.
async fn handle_request(req: JsonRpcRequest, state: SharedState) -> JsonRpcResponse {
    let id = req.id.clone();

    match req.method.as_str() {
        // ── MCP Lifecycle ────────────────────────────────────────────
        "initialize" => {
            let workspace_root = req.params
                .as_ref()
                .and_then(|p| p.get("rootPath").or_else(|| p.get("rootUri")))
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches("file://").to_string());

            if let Some(root) = workspace_root {
                let mut st = state.write().await;
                st.workspace_root = std::path::PathBuf::from(&root);
                info!("Workspace root set to: {}", root);
            }

            let result = InitializeResult {
                protocol_version: "2024-11-05",
                capabilities: ServerCapabilities {
                    tools: ToolsCapability { list_changed: false },
                },
                server_info: ServerInfo {
                    name: "chronos-agent",
                    version: env!("CARGO_PKG_VERSION"),
                },
            };
            JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
        }

        "initialized" | "notifications/initialized" => {
            // Notification — no response needed, but send empty ok to be safe
            info!("Client initialized handshake complete ✓");
            JsonRpcResponse::ok(id, json!({}))
        }

        "ping" => JsonRpcResponse::ok(id, json!({})),

        // ── Tools ────────────────────────────────────────────────────
        "tools/list" => {
            let tool_list = tools::all_tools();
            JsonRpcResponse::ok(id, json!({ "tools": tool_list }))
        }

        "tools/call" => {
            let tool_name = req.params.as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let tool_params = req.params.as_ref()
                .and_then(|p| p.get("arguments"))
                .cloned();

            match tool_name.as_deref() {
                Some("chronos_analyze_file") => {
                    let result = tools::handle_analyze_file(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_query_hotspots") => {
                    let result = tools::handle_query_hotspots(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_blast_radius") => {
                    let result = tools::handle_blast_radius(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_mutation_gate") => {
                    let result = tools::handle_mutation_gate(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_contract_status") => {
                    let result = tools::handle_contract_status(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_query_graph") => {
                    let result = tools::handle_query_graph(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_search_graph") => {
                    let result = tools::handle_search_graph(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_get_snippet") => {
                    let result = tools::handle_get_snippet(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_get_architecture") => {
                    let result = tools::handle_get_architecture(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_hydrate_telemetry") => {
                    let result = tools::handle_hydrate_telemetry(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some("chronos_generate_agents_md") => {
                    let result = tools::handle_generate_agents_md(state, tool_params).await;
                    JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap())
                }
                Some(unknown) => {
                    warn!("Unknown tool called: {}", unknown);
                    JsonRpcResponse::err(id, -32601, format!("Tool not found: {}", unknown))
                }
                None => {
                    JsonRpcResponse::err(id, -32602, "Missing tool name in tools/call params")
                }
            }
        }

        // ── Unknown method ───────────────────────────────────────────
        unknown => {
            warn!("Unknown method: {}", unknown);
            JsonRpcResponse::err(id, -32601, format!("Method not found: {}", unknown))
        }
    }
}

fn write_response(stdout: &mut impl Write, response: &JsonRpcResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{}", json);
        let _ = stdout.flush();
    }
}
