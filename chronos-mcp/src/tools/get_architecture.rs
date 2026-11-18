//! # `chronos_get_architecture` Tool Handler
//!
//! Returns a high-level architecture overview: community clusters (Label Propagation + Cohesion),
//! language breakdown, entry points, top PageRank hubs, and hidden O(N^2) hotspot candidates.
//! Automatically indexes supported workspace files if the graph is currently empty or `auto_index` is true.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::GraphQueryEngine;
use serde_json::{json, Value};
use std::path::Path;

pub async fn handle_get_architecture(state: SharedState, params: Option<Value>) -> CallToolResult {
    let path_prefix = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let auto_index = params
        .as_ref()
        .and_then(|p| p.get("auto_index"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    {
        let mut st = state.write().await;
        if auto_index {
            let scope = path_prefix.as_deref().map(Path::new);
            let _ = st.session.ensure_indexed(scope, 300);
        }
    }

    let st = state.read().await;
    let qe = GraphQueryEngine::new(&st.graph);
    let report = qe.get_architecture(path_prefix.as_deref());

    let output = json!({
        "status": "ok",
        "architecture": report,
        "_meta": {
            "provenance": {
                "untrusted_content": true,
                "source": "chronos-architecture-communities"
            }
        }
    });

    CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
}
