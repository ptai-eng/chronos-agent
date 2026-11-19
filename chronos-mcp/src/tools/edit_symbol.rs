//! # `chronos_edit_symbol` MCP Tool Handler (13th MCP Tool)
//!
//! Dispatches `chronos_edit_symbol` (and Serena compatibility alias `chronos_replace_symbol_body`)
//! through `WorkspaceSession::edit_symbol` with 2-stage Tree-sitter syntax + blast-radius gates.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::ast::symbol_editor::{EditOperation, EditSymbolRequest};
use serde_json::Value;
use std::path::PathBuf;

pub async fn handle_edit_symbol(state: SharedState, params: Option<Value>) -> CallToolResult {
    let symbol = match params
        .as_ref()
        .and_then(|p| p.get("symbol"))
        .and_then(|v| v.as_str())
    {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return CallToolResult::error("Missing required non-empty parameter 'symbol'"),
    };

    let file = match params
        .as_ref()
        .and_then(|p| p.get("file").or_else(|| p.get("file_path")))
        .and_then(|v| v.as_str())
    {
        Some(f) if !f.trim().is_empty() => PathBuf::from(f.trim()),
        _ => return CallToolResult::error("Missing required non-empty parameter 'file'"),
    };

    let op_raw = params
        .as_ref()
        .and_then(|p| p.get("operation"))
        .and_then(|v| v.as_str())
        .unwrap_or("replace_body");
    let operation = match EditOperation::parse(op_raw) {
        Ok(op) => op,
        Err(e) => return CallToolResult::error(e.to_string()),
    };

    let new_code = match params
        .as_ref()
        .and_then(|p| p.get("new_code").or_else(|| p.get("body")))
        .and_then(|v| v.as_str())
    {
        Some(code) => code.to_string(),
        None => return CallToolResult::error("Missing required parameter 'new_code'"),
    };

    let dry_run = params
        .as_ref()
        .and_then(|p| p.get("dry_run"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let force = params
        .as_ref()
        .and_then(|p| p.get("force"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let req = EditSymbolRequest {
        symbol,
        file,
        operation,
        new_code,
        dry_run,
        force,
    };

    // Ensure workspace graph is indexed on cold start so cross-file callers (`fan_in`) are known in Stage 2
    {
        let needs_index = {
            let st = state.read().await;
            st.graph.node_count() == 0
        };
        if needs_index {
            let mut st = state.write().await;
            if st.graph.node_count() == 0 {
                let _ = st.session.ensure_indexed(None, 300);
            }
        }
    }

    let mut st = state.write().await;
    match st.session.edit_symbol(req).await {
        Ok(outcome) => {
            let text = serde_json::to_string_pretty(&outcome).unwrap_or_default();
            if outcome.is_error {
                CallToolResult::error(text)
            } else {
                CallToolResult::text(text)
            }
        }
        Err(e) => CallToolResult::error(format!("chronos_edit_symbol failed: {e}")),
    }
}
