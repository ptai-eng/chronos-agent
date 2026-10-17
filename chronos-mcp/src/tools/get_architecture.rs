//! # `chronos_get_architecture` Tool Handler
//!
//! Returns a high-level architecture overview: community clusters (Label Propagation + Cohesion),
//! language breakdown, entry points, top PageRank hubs, and hidden O(N^2) hotspot candidates.
//! Automatically indexes supported workspace files if the graph is currently empty or `auto_index` is true.

use crate::protocol::CallToolResult;
use crate::state::{ChronosState, SharedState};
use chronos_core::{
    ast::dispatcher::LanguageDispatcher,
    graph::query::GraphQueryEngine,
};
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
            let root = st.workspace_root.clone();
            if let Ok(mut dispatcher) = LanguageDispatcher::new() {
                let mut files = Vec::new();
                collect_supported_files(&root, &mut files, 300);
                // Split borrow of st.db and st.graph
                let ChronosState {
                    ref mut graph,
                    ref db,
                    ..
                } = *st;
                if let Some(ref db_inst) = db {
                    let _ = db_inst.sync_workspace_incremental(graph, &files, &mut dispatcher);
                } else if graph.node_count() == 0 {
                    for f in &files {
                        if let Ok(parsed) = dispatcher.parse_file(f) {
                            graph.ingest_parsed_module(&parsed);
                        }
                    }
                    graph.resolve_pending_calls_and_routes();
                }
                chronos_core::graph::git_coupling::ingest_git_co_changes(graph, &root, 200);
            }
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

fn collect_supported_files(dir: &Path, out: &mut Vec<std::path::PathBuf>, max_files: usize) {
    if out.len() >= max_files {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        if out.len() >= max_files {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | ".venv" | "venv" | ".chronos" | "dist" | "build"
            ) {
                continue;
            }
            collect_supported_files(&path, out, max_files);
        } else if LanguageDispatcher::is_supported(&path) {
            out.push(path);
        }
    }
}
