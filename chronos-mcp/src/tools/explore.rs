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
use crate::state::{ChronosState, SharedState};
use chronos_core::{ast::dispatcher::LanguageDispatcher, graph::query::GraphQueryEngine};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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
                let root = st.workspace_root.clone();
                if let Ok(mut dispatcher) = LanguageDispatcher::new() {
                    let mut files = Vec::new();
                    collect_supported_files(&root, &mut files, 300);
                    let ChronosState {
                        ref mut graph,
                        ref db,
                        ..
                    } = *st;
                    if let Some(ref db_inst) = db {
                        let _ = db_inst.sync_workspace_incremental(graph, &files, &mut dispatcher);
                    } else {
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
    }

    let st = state.read().await;
    let status = st.graph.status_report();
    let qe = GraphQueryEngine::new(&st.graph);
    let explore_res = qe.explore_graph(&query, file_filter.as_deref(), max_symbols);

    let output = json!({
        "status": "ok",
        "explore": explore_res,
        "coverage": {
            "completeness": if status.total_edges == 0 {
                "unknown"
            } else if status.unresolved_calls == 0 && status.hydrated_nodes > 0 {
                "complete"
            } else {
                "partial"
            },
            "total_nodes": status.total_nodes,
            "total_edges": status.total_edges,
            "resolved_calls": status.resolved_calls,
            "unresolved_calls": status.unresolved_calls,
            "telemetry_hydrated": status.hydrated_nodes > 0
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

fn collect_supported_files(dir: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
    if out.len() >= max_files {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= max_files {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name == "build"
            || name == "__pycache__"
            || name == "venv"
        {
            continue;
        }
        if path.is_dir() {
            collect_supported_files(&path, out, max_files);
        } else if LanguageDispatcher::is_supported(&path) {
            out.push(path);
        }
    }
}
