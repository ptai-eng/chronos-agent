//! # `chronos_explore` Tool Handler (12th MCP Tool)
//!
//! 1-Call Subagent Context Engine inspired by Anthropic Claude Code `Explore.md` and
//! OpenAI Codex `codex-auto-review.md`.
//!
//! In a single `<15ms` call, returns:
//! - Top `max_symbols` BM25/regex matching symbols
//! - Verbatim source code (`start_line..=end_line`) with line numbers
//! - Inbound callers, outbound callees, and covering tests
//! - Real user-facing `verification_surfaces` (`ApiRoute`, `CliCommand`, `RootEntrypoint`, `TestCase`)
//! - 4-Angle Code Review diagnostics (`Reuse`, `Efficiency`, `Simplification`, `Altitude`)

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::GraphQueryEngine;
use serde_json::{json, Value};

pub async fn handle_explore(state: SharedState, params: Option<Value>) -> CallToolResult {
    let query = match params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|v| v.as_str())
    {
        Some(q) if !q.trim().is_empty() => q.to_string(),
        _ => return CallToolResult::error("Missing required non-empty parameter 'query'"),
    };

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
        .clamp(1, 10);

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

    let st = state.read().await;
    let status = st.graph.status_report();
    let (cov, _, _) = chronos_core::blast_radius::impact_matrix::CoverageReport::from_graph(&st.graph);
    let qe = GraphQueryEngine::new(&st.graph);
    let explore_res = qe.explore_graph(&query, file_filter.as_deref(), max_symbols);

    let output = json!({
        "status": "ok",
        "explore": explore_res,
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

    CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
}
