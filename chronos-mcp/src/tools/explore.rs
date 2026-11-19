//! # `chronos_explore` Tool Handler (12th MCP Tool)
//!
//! 1-Call Subagent Context Engine supporting:
//! - Natural-language BM25 / regex search
//! - Progressive `Class/*` wildcard outlines & `include_body: false` signature-only slices
//! - Batch `queries[]` & class expansion `depth`
//! - Inline `include_blast_radius` risk summaries
//! - Sandboxed `ScriptProfile::ReadOnly` Rhai 5D graph queries (`script` parameter)

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::{ExploreOptions, GraphQueryEngine};
use chronos_core::script::ScriptProfile;
use serde_json::{json, Value};

pub async fn handle_explore(state: SharedState, params: Option<Value>) -> CallToolResult {
    let query = params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let queries: Vec<String> = params
        .as_ref()
        .and_then(|p| p.get("queries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let script = params
        .as_ref()
        .and_then(|p| p.get("script"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if query.is_empty() && queries.is_empty() && script.is_none() {
        return CallToolResult::error(
            "Missing required query: provide at least one of 'query', 'queries', or 'script'",
        );
    }

    let file_filter = params
        .as_ref()
        .and_then(|p| p.get("file_filter").or_else(|| p.get("file_path")))
        .and_then(|v| v.as_str())
        .map(String::from);

    let max_symbols = params
        .as_ref()
        .and_then(|p| p.get("max_symbols"))
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(3)
        .clamp(1, 20);

    let include_body = params
        .as_ref()
        .and_then(|p| p.get("include_body"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let depth = params
        .as_ref()
        .and_then(|p| p.get("depth"))
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(1)
        .clamp(1, 5);

    let include_blast_radius = params
        .as_ref()
        .and_then(|p| p.get("include_blast_radius"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let auto_index = params
        .as_ref()
        .and_then(|p| p.get("auto_index"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Auto-index workspace on cold start so 1-call explore never returns empty on an unindexed repo
    if auto_index {
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

    // Evaluate bounded ReadOnly Rhai script if provided (write functions structurally excluded)
    let script_outcome = if let Some(ref script_src) = script {
        let mut st = state.write().await;
        match st.session.eval_script(ScriptProfile::ReadOnly, script_src) {
            Ok(outcome) => Some(outcome),
            Err(e) => {
                return CallToolResult::error(format!(
                    "chronos_explore ReadOnly script error: {e}"
                ))
            }
        }
    } else {
        None
    };

    let st = state.read().await;
    let status = st.graph.status_report();
    let (cov, _, _) =
        chronos_core::blast_radius::impact_matrix::CoverageReport::from_graph(&st.graph);

    let explore_res = if !query.is_empty() || !queries.is_empty() {
        let qe = GraphQueryEngine::new(&st.graph);
        Some(qe.explore_graph_with_options(
            &query,
            &ExploreOptions {
                queries,
                file_filter,
                max_symbols,
                include_body,
                depth,
                include_blast_radius,
            },
        ))
    } else {
        None
    };

    let mut output = json!({
        "status": "ok",
        "coverage": {
            "completeness": cov.completeness,
            "total_nodes": status.total_nodes,
            "total_edges": status.total_edges,
            "resolved_calls": cov.resolved_calls,
            "unresolved_calls": cov.unresolved_calls,
            "telemetry_hydrated": cov.telemetry_hydrated
        },
        "_meta": {
            "provenance": {
                "untrusted_content": true,
                "source": "chronos-explore-1call"
            }
        }
    });

    if let Some(exp) = explore_res {
        output["explore"] = serde_json::to_value(exp).unwrap_or(Value::Null);
    }
    if let Some(sc) = script_outcome {
        output["script_result"] = sc.result;
        if !sc.printed_logs.is_empty() {
            output["printed_logs"] = json!(sc.printed_logs);
        }
    }

    CallToolResult::text(serde_json::to_string(&output).unwrap_or_default())
}
