//! # `chronos_refactor_symbol` MCP Tool Handler (14th MCP Tool)
//!
//! Dispatches `chronos_refactor_symbol` (plus Serena compatibility aliases `chronos_rename_symbol`
//! and `chronos_safe_delete`) through `WorkspaceSession::refactor_symbol` with edge-verified
//! multi-file transactional execution and explicit ambiguous/dependent blockers.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::ast::refactor::{RefactorAction, RefactorSymbolRequest};
use serde_json::Value;
use std::path::PathBuf;

pub async fn handle_refactor_symbol(state: SharedState, params: Option<Value>) -> CallToolResult {
    let symbol = match params
        .as_ref()
        .and_then(|p| p.get("symbol"))
        .and_then(|v| v.as_str())
    {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return CallToolResult::error("Missing required non-empty parameter 'symbol'"),
    };

    let action_raw = params
        .as_ref()
        .and_then(|p| p.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("preview_usages");
    let action = match RefactorAction::parse(action_raw) {
        Ok(a) => a,
        Err(e) => return CallToolResult::error(e.to_string()),
    };

    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let file = params
        .as_ref()
        .and_then(|p| p.get("file").or_else(|| p.get("file_path")))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| PathBuf::from(s.trim()));

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
    let use_lsp = params
        .as_ref()
        .and_then(|p| p.get("use_lsp"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Ensure graph is populated before multi-file cross-reference analysis on cold start
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

    let req = RefactorSymbolRequest {
        symbol,
        action,
        new_name,
        file,
        dry_run,
        force,
        use_lsp,
    };

    let mut st = state.write().await;
    match st.session.refactor_symbol(req).await {
        Ok(outcome) => {
            let text = serde_json::to_string_pretty(&outcome).unwrap_or_default();
            if outcome.is_error {
                CallToolResult::error(text)
            } else {
                CallToolResult::text(text)
            }
        }
        Err(e) => CallToolResult::error(format!("chronos_refactor_symbol failed: {e}")),
    }
}
