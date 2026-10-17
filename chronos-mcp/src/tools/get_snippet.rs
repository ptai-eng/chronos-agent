//! # `chronos_get_snippet` Tool Handler
//!
//! Retrieves the exact source code snippet (`start_line..=end_line`) for a symbol
//! alongside its callers, callees, covering tests, PageRank, and AST complexity metrics.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::GraphQueryEngine;
use serde_json::{json, Value};

pub async fn handle_get_snippet(state: SharedState, params: Option<Value>) -> CallToolResult {
    let symbol_name = match params
        .as_ref()
        .and_then(|p| p.get("symbol_name"))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => {
            return CallToolResult::error("Missing required parameter 'symbol_name'");
        }
    };

    let file_path = params
        .as_ref()
        .and_then(|p| p.get("file_path"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let st = state.read().await;
    let qe = GraphQueryEngine::new(&st.graph);

    match qe.get_code_snippet(&symbol_name, file_path.as_deref()) {
        Some(snippet) => {
            let output = json!({
                "status": "ok",
                "snippet": snippet,
                "_meta": {
                    "provenance": {
                        "untrusted_content": true,
                        "source": "chronos-code-snippet"
                    }
                }
            });
            CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        None => CallToolResult::error(format!(
            "Symbol '{}' not found in graph. Call chronos_analyze_file or chronos_get_architecture first.",
            symbol_name
        )),
    }
}
