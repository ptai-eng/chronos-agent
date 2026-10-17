//! # `chronos_query_graph` Tool Handler
//!
//! Executes a Read-Only Cypher query against the embedded KuzuDB 4D Knowledge Graph.
//! Enforces `validate_readonly_cypher` to reject mutating or file-I/O statements.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::persistent::validate_readonly_cypher;
use serde_json::{json, Value};

pub async fn handle_query_graph(state: SharedState, params: Option<Value>) -> CallToolResult {
    let query = match params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|v| v.as_str())
    {
        Some(q) => q.to_string(),
        None => {
            return CallToolResult::error("Missing required parameter 'query' (Cypher query string)");
        }
    };

    let max_rows = params
        .as_ref()
        .and_then(|p| p.get("max_rows"))
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;

    if let Err(e) = validate_readonly_cypher(&query) {
        return CallToolResult::error(format!("Cypher security validation failed: {e}"));
    }

    let st = state.read().await;
    let Some(ref db) = st.db else {
        return CallToolResult::error("KuzuDB persistent engine is not initialized in current state");
    };

    // Ensure latest in-memory nodes/edges are synced to KuzuDB before running Cypher
    let _ = db.sync_from_graph(&st.graph);

    match db.execute_readonly_cypher(&query, max_rows) {
        Ok(res) => {
            let output = json!({
                "status": "ok",
                "query": query,
                "columns": res.columns,
                "total_rows": res.total_rows,
                "returned_rows": res.rows.len(),
                "truncated": res.truncated,
                "rows": res.rows,
                "_meta": {
                    "provenance": {
                        "untrusted_content": true,
                        "source": "chronos-kuzudb-cypher"
                    }
                }
            });
            CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
        }
        Err(e) => CallToolResult::error(format!("Cypher execution error: {e}")),
    }
}
