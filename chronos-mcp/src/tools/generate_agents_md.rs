//! # `chronos_generate_agents_md` Tool Handler
//!
//! Synthesizes data-driven `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.agents/skills/chronos-agent/SKILL.md`,
//! `.cursor/rules/chronos-5d.mdc`, and MCP client configurations from the 5D Knowledge Graph for:
//! OpenAI Codex CLI, Google Antigravity, Anthropic Claude Code, OpenCode, Cursor, Windsurf, Copilot, and Cline.

use crate::protocol::CallToolResult;
use crate::state::{ChronosState, SharedState};
use chronos_core::{
    ast::dispatcher::LanguageDispatcher,
    graph::agents_md::{scaffold_client_files, synthesize_target_context, AgentTarget},
};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub async fn handle_generate_agents_md(
    state: SharedState,
    params: Option<Value>,
) -> CallToolResult {
    let project_name = params
        .as_ref()
        .and_then(|p| p.get("project_name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let target_str = params
        .as_ref()
        .and_then(|p| p.get("target"))
        .and_then(|v| v.as_str())
        .unwrap_or("codex");
    let target = AgentTarget::from_str_loose(target_str);

    let write_to_disk = params
        .as_ref()
        .and_then(|p| p.get("write_to_disk"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let scaffold_configs = params
        .as_ref()
        .and_then(|p| p.get("scaffold_configs"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let output_rel = params
        .as_ref()
        .and_then(|p| p.get("output_path"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| target.default_context_path());

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
    let md_content = synthesize_target_context(
        &st.graph,
        &st.workspace_root,
        target,
        project_name.as_deref(),
    );

    let mut written_path: Option<String> = None;
    let mut written_files: Vec<String> = Vec::new();

    if write_to_disk {
        if scaffold_configs || target == AgentTarget::All {
            let bundle = scaffold_client_files(
                &st.graph,
                &st.workspace_root,
                target,
                "chronos-mcp",
                project_name.as_deref(),
            );
            for item in bundle {
                let rel_path = Path::new(&item.relative_path);
                if rel_path.is_absolute()
                    || rel_path
                        .components()
                        .any(|c| matches!(c, Component::ParentDir))
                {
                    continue;
                }
                let full_path = st.workspace_root.join(rel_path);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&full_path, &item.content).is_ok() {
                    written_files.push(item.relative_path);
                }
            }
            written_path = written_files.first().cloned();
        } else {
            let rel_path = Path::new(output_rel);
            if rel_path.is_absolute()
                || rel_path
                    .components()
                    .any(|c| matches!(c, Component::ParentDir))
            {
                return CallToolResult::error(
                    "Security Error: output_path must be a relative path inside the workspace without '..' traversal.",
                );
            }
            let target_path = st.workspace_root.join(rel_path);
            if let Some(parent) = target_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&target_path, &md_content) {
                return CallToolResult::error(format!("Failed to write context file: {e}"));
            }
            written_path = Some(target_path.to_string_lossy().to_string());
            written_files.push(output_rel.to_string());
        }
    }

    let output = json!({
        "status": "ok",
        "target": target.display_name(),
        "written_path": written_path,
        "written_files": written_files,
        "agents_md": md_content,
        "_meta": {
            "provenance": {
                "untrusted_content": true,
                "source": "chronos-agents-md-synthesizer"
            }
        }
    });

    CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn collect_supported_files(dir: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
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
