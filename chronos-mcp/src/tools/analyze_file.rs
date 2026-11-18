//! # Tool: chronos_analyze_file
//!
//! Parse a TypeScript file and bulk-insert all symbols into the 4D graph.
//! The parser returns `ChronosNode` objects directly — no remapping needed.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::GraphQueryEngine;
use serde_json::Value;
use std::path::PathBuf;

pub async fn handle_analyze_file(state: SharedState, params: Option<Value>) -> CallToolResult {
    let file_path = match params.as_ref()
        .and_then(|p| p.get("file_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        Some(p) => p,
        None => return CallToolResult::error("Missing required parameter: file_path"),
    };

    let (idx_report, report, four_angles) = {
        let mut st = state.write().await;
        let idx_report = match st.session.index_file(&PathBuf::from(&file_path)).await {
            Ok(r) => r,
            Err(e) => return CallToolResult::error(e.to_string()),
        };
        let report = st.graph.status_report();
        let four_angles = GraphQueryEngine::new(&st.graph).audit_file_four_angles(&file_path);
        (idx_report, report, four_angles)
    };

    let fn_count = idx_report.functions;
    let class_count = idx_report.classes;
    let route_count = idx_report.api_routes;
    let test_count = idx_report.test_cases;
    let call_ref_count = idx_report.call_refs;
    let import_count = idx_report.import_refs;
    let unresolved_count = idx_report.unresolved_call_sites;
    let inserted = idx_report.nodes_inserted;
    let edges_added = idx_report.edges_added;

    let four_angle_section = if four_angles.is_empty() {
        "  - No Reuse/Efficiency/Simplification/Altitude warnings detected.".to_string()
    } else {
        four_angles
            .iter()
            .take(8)
            .map(|d| {
                format!(
                    "  - **[{}]** `{}` (`{}:{}`) — {} → *{}*",
                    d.angle, d.symbol, d.file, d.line, d.summary, d.recommendation
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    CallToolResult::text(format!(
        r#"✅ **CHRONOS: File Analyzed**

**File:** `{file_path}`
**Symbols extracted:**
  - Functions: {fn_count}
  - Classes: {class_count}
  - API routes: {route_count}
  - Test cases: {test_count}
  - Call references: {call_ref_count} (unresolved dynamic sites: {unresolved_count})
  - Import references: {import_count}

**Graph state:**
  - Total nodes: {} (functions: {}, tests: {})
  - Total edges: {} (resolved calls: {}, unresolved: {})
  - New nodes inserted: {inserted}
  - New edges linked: {edges_added}
  - Hydrated: {}/{} nodes

**4-Angle Code Review Audit (`Reuse | Efficiency | Simplification | Altitude`):**
{four_angle_section}

**Provenance & Coverage (`_meta`):**
  - `_meta.provenance.untrusted_content`: `true` (parsed from external source AST)
  - `coverage.call_edges_available`: `{}`

Next steps:
- `chronos_explore` {{ query: "functionName" }} → 1-call verbatim snippet + verification surfaces + 4-angle audit
- `chronos_query_hotspots` → performance & PageRank bottlenecks
- `chronos_blast_radius` {{ symbol_name: "functionName", detail: "summary" }} → change impact + structured findings
- `chronos_contract_status` → check for breaking change alerts"#,
        report.total_nodes,
        report.function_nodes,
        report.test_nodes,
        report.total_edges,
        report.resolved_calls,
        report.unresolved_calls,
        report.hydrated_nodes,
        report.total_nodes,
        report.total_edges > 0,
    ))
}
