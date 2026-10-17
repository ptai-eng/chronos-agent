//! # `chronos_search_graph` Tool Handler
//!
//! Searches the 4D Knowledge Graph using BM25 identifier tokenization (`camelCase` / `snake_case`),
//! regex matching, degree/loop filters, and structural label boosting.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::{GraphQueryEngine, GraphSearchRequest};
use serde_json::{json, Value};

pub async fn handle_search_graph(state: SharedState, params: Option<Value>) -> CallToolResult {
    let query = params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let name_pattern = params
        .as_ref()
        .and_then(|p| p.get("name_pattern"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let file_pattern = params
        .as_ref()
        .and_then(|p| p.get("file_pattern"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let kind = params
        .as_ref()
        .and_then(|p| p.get("kind"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let min_degree = params
        .as_ref()
        .and_then(|p| p.get("min_degree"))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    let min_loop_depth = params
        .as_ref()
        .and_then(|p| p.get("min_loop_depth"))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    let limit = params
        .as_ref()
        .and_then(|p| p.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    let offset = params
        .as_ref()
        .and_then(|p| p.get("offset"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let st = state.read().await;
    let qe = GraphQueryEngine::new(&st.graph);
    let resp = qe.search_graph(&GraphSearchRequest {
        query,
        name_pattern,
        file_pattern,
        kind,
        min_degree,
        min_loop_depth,
        limit,
        offset,
    });

    let output = json!({
        "status": "ok",
        "total": resp.total,
        "offset": resp.offset,
        "limit": resp.limit,
        "has_more": resp.has_more,
        "results": resp.results,
        "_meta": {
            "provenance": {
                "untrusted_content": true,
                "source": "chronos-bm25-search"
            }
        }
    });

    CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
}
