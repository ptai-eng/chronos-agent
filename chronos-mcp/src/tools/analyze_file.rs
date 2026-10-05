//! # Tool: chronos_analyze_file
//!
//! Parse a TypeScript file and bulk-insert all symbols into the 4D graph.
//! The parser returns `ChronosNode` objects directly — no remapping needed.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::{
    ast::dispatcher::LanguageDispatcher,
    graph::schema::{ChronosEdge, EdgeKind},
};
use serde_json::Value;
use std::path::PathBuf;
use tracing::warn;

pub async fn handle_analyze_file(state: SharedState, params: Option<Value>) -> CallToolResult {
    let file_path = match params.as_ref()
        .and_then(|p| p.get("file_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        Some(p) => p,
        None => return CallToolResult::error("Missing required parameter: file_path"),
    };

    let abs_path = {
        let st = state.read().await;
        let p = PathBuf::from(&file_path);
        let joined = if p.is_absolute() { p } else { st.workspace_root.join(&p) };
        // Security: canonicalize and ensure path stays within workspace
        match joined.canonicalize() {
            Ok(canonical) => {
                if !canonical.starts_with(&st.workspace_root) {
                    return CallToolResult::error(
                        "Security: file path is outside the workspace root"
                    );
                }
                canonical
            }
            Err(_) => joined, // file not yet created — will fail at exists() check below
        }
    };

    if !abs_path.exists() {
        return CallToolResult::error(format!("File not found: {}", abs_path.display()));
    }

    // Reject unsupported file types early
    if !LanguageDispatcher::is_supported(&abs_path) {
        return CallToolResult::error(format!(
            "Unsupported file type: '{}'. Supported: .ts .tsx .js .py .rs",
            abs_path.extension().and_then(|e| e.to_str()).unwrap_or("?")
        ));
    }

    let mut dispatcher = match LanguageDispatcher::new() {
        Ok(d) => d,
        Err(e) => return CallToolResult::error(format!("Parser init: {}", e)),
    };

    let parsed = match dispatcher.parse_file(&abs_path) {
        Ok(m) => m,
        Err(e) => return CallToolResult::error(format!("Parse failed: {}", e)),
    };

    let fn_count = parsed.functions.len();
    let class_count = parsed.classes.len();
    let route_count = parsed.api_routes.len();
    let test_count = parsed.test_cases.len();
    let call_ref_count = parsed.call_refs.len();
    let import_count = parsed.import_refs.len();
    // Collect all nodes upfront to avoid double-borrow of `parsed`
    let all_nodes: Vec<chronos_core::graph::schema::ChronosNode> =
        parsed.all_nodes().into_iter().cloned().collect();
    let _total_symbols = all_nodes.len();

    let mut st = state.write().await;
    let mut inserted = 0usize;
    let mut edges_added = 0usize;

    // Bulk insert all nodes (functions, classes, routes, test cases)
    for node in &all_nodes {
        st.graph.add_node(node.clone());
        inserted += 1;
    }

    // Performance fix: pre-build name→id lookup map BEFORE the loop (O(N) instead of O(N×M))
    use chronos_core::graph::schema::NodeKind;
    use std::collections::HashMap;
    let name_to_id: HashMap<String, chronos_core::graph::schema::NodeId> = st.graph
        .nodes_by_kind(&NodeKind::Function)
        .into_iter()
        .chain(st.graph.nodes_by_kind(&NodeKind::Class).into_iter())
        .map(|n| (n.name.clone(), n.id.clone()))
        .collect();

    // Insert call reference edges (A calls B) — now O(N) with HashMap lookup
    for call_ref in &parsed.call_refs {
        let caller_id = name_to_id.get(&call_ref.caller_name).cloned();
        let callee_id = name_to_id.get(&call_ref.callee_name).cloned();

        if let (Some(caller), Some(callee)) = (caller_id, callee_id) {
            if st.graph.add_edge(&caller, &callee, ChronosEdge::new(EdgeKind::Calls)).is_ok() {
                edges_added += 1;
            }
        }
    }

    if let Some(ref db) = st.db {
        if let Err(e) = db.sync_from_graph(&st.graph) {
            warn!("KuzuDB sync failed: {}", e);
        }
    }

    let broker = st.broker.clone();
    
    let report = st.graph.status_report();

    // Drop lock before await
    drop(st);

    // Index in contract broker for real-time contract watching
    if let Err(e) = broker.index_file(&abs_path).await {
        warn!("Contract broker indexing failed for {}: {}", abs_path.display(), e);
    }

    CallToolResult::text(format!(
        r#"✅ **CHRONOS: File Analyzed**

**File:** `{file_path}`
**Symbols extracted:**
  - Functions: {fn_count}
  - Classes: {class_count}
  - API routes: {route_count}
  - Test cases: {test_count}
  - Call references: {call_ref_count}
  - Import references: {import_count}

**Graph state:**
  - Total nodes: {} (functions: {}, tests: {})
  - New nodes inserted: {inserted}
  - New call edges added: {edges_added}
  - Hydrated: {}/{} nodes

Next steps:
- `chronos_query_hotspots` → performance bottlenecks
- `chronos_blast_radius` {{ symbol_name: "functionName" }} → change impact
- `chronos_contract_status` → check for breaking change alerts"#,
        report.total_nodes, report.function_nodes, report.test_nodes,
        report.hydrated_nodes, report.total_nodes,
    ))
}
