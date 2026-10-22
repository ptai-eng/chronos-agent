//! # OpenAI Codex `AGENTS.md` Synthesizer
//!
//! Compiles the 5D Knowledge Graph (Community Clusters, PageRank Hubs, Algorithmic Hotspots,
//! Cross-Boundary HTTP Routes, and Git Temporal Co-Change Coupling) into a structured
//! `AGENTS.md` blueprint tailored for OpenAI Codex CLI (`openai/codex`), ChatGPT Codex Cloud,
//! and autonomous coding agents.

use crate::graph::{
    query::GraphQueryEngine,
    schema::{ChronosGraph, EdgeKind, NodeKind},
};
use std::path::Path;

/// Synthesize a complete, data-driven `AGENTS.md` specification from a populated `ChronosGraph`.
pub fn synthesize_agents_md(
    graph: &ChronosGraph,
    workspace: &Path,
    project_name_override: Option<&str>,
) -> String {
    let project_name = project_name_override
        .map(|s| s.to_string())
        .or_else(|| {
            workspace
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "workspace".to_string());

    let qe = GraphQueryEngine::new(graph);
    let arch = qe.get_architecture(None);
    let status = graph.status_report();

    let mut out = String::with_capacity(8192);
    out.push_str(&format!("# AGENTS.md — {project_name}\n\n"));
    out.push_str(
        "> Auto-synthesized by **CHRONOS-AGENT v1.5.0** (5D Living Code Intelligence Engine) for **OpenAI Codex CLI**, **ChatGPT Codex**, and MCP-compliant autonomous coding agents.\n\n",
    );

    // 1. Build & Verification Commands
    out.push_str("## 1. Build, Test & Verification Commands\n\n");
    let detected_cmds = detect_workspace_commands(workspace);
    for (label, cmd) in &detected_cmds {
        out.push_str(&format!("- **{label}:** `{cmd}`\n"));
    }
    out.push_str("- **5D Git Diff Blast-Radius Gate (CI / Pre-PR):** `chronos diff --staged --json --ci`\n");
    out.push_str("- **Refresh `AGENTS.md` from 5D Graph:** `chronos agents-md --write`\n\n");

    // 2. Mandatory OpenAI Codex & MCP Guardrail Workflow
    out.push_str("## 2. OpenAI Codex & MCP Tool Protocol (Mandatory Before Committing)\n\n");
    out.push_str(
        "When connected to `chronos-mcp` (`codex mcp add chronos-agent -- chronos-mcp --workspace .`), follow this 4-step verification loop:\n\n",
    );
    out.push_str(
        "1. **Locate & Read Without Context Bloat:** Call `chronos_get_architecture`, `chronos_search_graph` (BM25 + regex + loop depth), or `chronos_get_snippet` instead of dumping entire files.\n",
    );
    out.push_str(
        "2. **Pre-Edit Blast-Radius Check:** Before modifying any function listed in **High-Centrality Hubs** below, call `chronos_blast_radius` (`symbol_name`, `contract_breaking`) to inspect transitive callers, 95% confidence bounds, and Git co-change partners.\n",
    );
    out.push_str(
        "3. **Contract Broker Gate:** After editing public signatures or routes, call `chronos_contract_status` to drain breaking-change alerts across callers.\n",
    );
    out.push_str(
        "4. **Super-Mutant Verification Gate:** Before finalizing a PR on critical logic, run `chronos_mutation_gate` (`file_path`) to verify unit tests kill synthetic code mutants.\n\n",
    );

    // 3. 5D Graph Telemetry & Topology Summary
    out.push_str("## 3. 5D Knowledge Graph Snapshot\n\n");
    let mut langs: Vec<(&String, &usize)> = arch.languages.iter().collect();
    langs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let lang_summary = if langs.is_empty() {
        "None indexed".to_string()
    } else {
        langs
            .iter()
            .map(|(k, v)| format!("{k} ({v})"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    out.push_str(&format!(
        "- **Indexed Symbols:** `{}` nodes (`{}` functions, `{}` tests) | **Directed Edges:** `{}` (`{}` resolved calls, `{}` unresolved dynamic calls)\n",
        status.total_nodes,
        status.function_nodes,
        status.test_nodes,
        status.total_edges,
        status.resolved_calls,
        status.unresolved_calls
    ));
    out.push_str(&format!("- **Language Distribution:** {lang_summary}\n"));
    out.push_str(&format!(
        "- **Runtime Telemetry Hydrated:** `{}` (`{}` hot paths, `{}` bottlenecks)\n\n",
        if status.hydrated_nodes > 0 { "Yes" } else { "Cold-Start Static + PageRank Mode" },
        status.hot_paths,
        status.bottlenecks
    ));

    // 4. Community Architecture Clusters
    if !arch.clusters.is_empty() {
        out.push_str("## 4. Architectural Modules (Label-Propagation Communities)\n\n");
        out.push_str("| Module Cluster | Symbols | Internal Cohesion | Primary Symbols |\n");
        out.push_str("| :--- | :---: | :---: | :--- |\n");
        for cluster in arch.clusters.iter().take(12) {
            let top = if cluster.top_nodes.is_empty() {
                "—".to_string()
            } else {
                cluster
                    .top_nodes
                    .iter()
                    .take(4)
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "| `{}` | {} | {:.0}% | {} |\n",
                cluster.label,
                cluster.member_count,
                cluster.cohesion_score * 100.0,
                top
            ));
        }
        out.push('\n');
    }

    // 5. High-Centrality Hub Symbols (PageRank)
    if !arch.top_pagerank_hubs.is_empty() {
        out.push_str("## 5. High-Centrality Hub Symbols (Blast-Radius Sensitive)\n\n");
        out.push_str(
            "Modifying these structural hubs affects the largest transitive caller trees. **Always run `chronos_blast_radius` before changing their signatures or invariants:**\n\n",
        );
        out.push_str("| Symbol | Kind | File | PageRank | Fan-In | Fan-Out | Transitive Loop Depth |\n");
        out.push_str("| :--- | :---: | :--- | :---: | :---: | :---: | :---: |\n");
        for hub in arch.top_pagerank_hubs.iter().take(10) {
            let rel_file = relativize_path(&hub.node.file, workspace);
            out.push_str(&format!(
                "| `{}` | `{}` | `{}:{}` | `{:.4}` | {} | {} | {} |\n",
                hub.node.name,
                hub.node.kind,
                rel_file,
                hub.node.line,
                hub.pagerank,
                hub.node.fan_in,
                hub.node.fan_out,
                hub.node.transitive_loop_depth
            ));
        }
        out.push('\n');
    }

    // 6. Algorithmic Complexity Hotspots
    if !arch.algorithmic_hotspots.is_empty() {
        out.push_str("## 6. Algorithmic Complexity & Loop Hotspots\n\n");
        out.push_str(
            "Functions with interprocedural nested loops (`transitive_loop_depth >= 2`), linear scans inside loops ($O(N^2)$ risk), or recursion:\n\n",
        );
        for h in arch.algorithmic_hotspots.iter().take(10) {
            let rel_file = relativize_path(&h.file, workspace);
            out.push_str(&format!(
                "- **`{}`** (`{}:{}`) — `transitive_loop_depth={}`, `linear_scan_in_loop={}`, `alloc_in_loop={}`, `recursive={}`\n",
                h.name,
                rel_file,
                h.line,
                h.transitive_loop_depth,
                h.linear_scan_in_loop,
                h.alloc_in_loop,
                h.is_recursive
            ));
        }
        out.push('\n');
    }

    // 7. Cross-Boundary HTTP API Routes
    let routes = graph.nodes_by_kind(&NodeKind::ApiRoute);
    if !routes.is_empty() {
        out.push_str("## 7. Cross-Boundary API Routes (Frontend <-> Backend Stitched)\n\n");
        for route in routes.iter().take(15) {
            let callers: Vec<String> = graph
                .callers_of(&route.id)
                .into_iter()
                .map(|c| format!("`{}`", c.name))
                .collect();
            let caller_str = if callers.is_empty() {
                "no stitched callers".to_string()
            } else {
                format!("called by {}", callers.join(", "))
            };
            let rel_file = relativize_path(&route.location.file.to_string_lossy(), workspace);
            out.push_str(&format!(
                "- **`{}`** (`{}:{}`) — {}\n",
                route.name,
                rel_file,
                route.location.start_line,
                caller_str
            ));
        }
        out.push('\n');
    }

    // 8. 5th Dimension: Git Temporal Co-Change Coupling
    let mut co_change_pairs: Vec<(String, String, usize)> = Vec::new();
    for (src_id, dst_id, edge) in graph.all_edges() {
        if matches!(edge.kind, EdgeKind::CoChangesWith) {
            if let (Some(s), Some(d)) = (graph.get_node(&src_id), graph.get_node(&dst_id)) {
                let conf = edge.call_site_line.unwrap_or(50);
                co_change_pairs.push((
                    relativize_path(&s.name, workspace),
                    relativize_path(&d.name, workspace),
                    conf,
                ));
            }
        }
    }
    if co_change_pairs.is_empty() {
        let git_couplings = crate::graph::git_coupling::mine_git_co_changes(workspace, 200);
        for c in git_couplings {
            co_change_pairs.push((
                c.file_a,
                c.file_b,
                c.confidence_pct,
            ));
        }
    }
    co_change_pairs.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    co_change_pairs.dedup();

    if !co_change_pairs.is_empty() {
        out.push_str("## 8. Git Temporal Co-Change Coupling (`CO_CHANGES_WITH`)\n\n");
        out.push_str(
            "Files that historically change together in Git commits. When modifying the left file, verify whether the right partner also requires updates:\n\n",
        );
        for (a, b, conf) in co_change_pairs.iter().take(12) {
            out.push_str(&format!("- `{a}` $\\rightarrow$ `{b}` (**{conf}%** co-change confidence)\n"));
        }
        out.push('\n');
    }

    out
}

fn detect_workspace_commands(workspace: &Path) -> Vec<(&'static str, &'static str)> {
    let mut cmds = Vec::new();
    if workspace.join("Cargo.toml").exists() {
        cmds.push(("Rust Build", "cargo build --workspace -j 4"));
        cmds.push(("Rust Test Suite", "cargo test --workspace -j 4"));
    }
    if workspace.join("package.json").exists() {
        cmds.push(("Node / TypeScript Tests", "npm test"));
    }
    if workspace.join("pyproject.toml").exists() || workspace.join("pytest.ini").exists() {
        cmds.push(("Python Test Suite", "pytest"));
    }
    if workspace.join("go.mod").exists() {
        cmds.push(("Go Test Suite", "go test ./..."));
    }
    if cmds.is_empty() {
        cmds.push(("CHRONOS Audit", "chronos arch"));
    }
    cmds
}

fn relativize_path(raw: &str, workspace: &Path) -> String {
    let norm_raw = raw.replace('\\', "/");
    let norm_ws = workspace
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if !norm_ws.is_empty() && norm_ws != "." {
        if let Some(stripped) = norm_raw.strip_prefix(&norm_ws) {
            let clean = stripped.trim_start_matches('/');
            if !clean.is_empty() {
                return clean.to_string();
            }
        }
    }
    norm_raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::{ChronosEdge, ChronosNode, EdgeKind, NodeId, NodeKind, SourceLocation};
    use std::path::PathBuf;

    #[test]
    fn test_synthesize_agents_md_contains_clusters_hubs_and_co_changes() {
        let mut graph = ChronosGraph::new();
        let fn_a = NodeId::new("src/auth.rs", "verify_token", &NodeKind::Function);
        let fn_b = NodeId::new("src/api.rs", "login_handler", &NodeKind::Function);

        let mut node_a = ChronosNode::new(
            fn_a.clone(),
            NodeKind::Function,
            "verify_token",
            "src/auth.rs::verify_token",
            SourceLocation {
                file: PathBuf::from("src/auth.rs"),
                start_line: 10,
                start_col: 0,
                end_line: 40,
                end_col: 0,
            },
        );
        node_a.heuristics.loop_depth = 2;
        node_a.heuristics.transitive_loop_depth = 2;
        node_a.heuristics.linear_scan_in_loop = 1;

        let node_b = ChronosNode::new(
            fn_b.clone(),
            NodeKind::Function,
            "login_handler",
            "src/api.rs::login_handler",
            SourceLocation {
                file: PathBuf::from("src/api.rs"),
                start_line: 5,
                start_col: 0,
                end_line: 25,
                end_col: 0,
            },
        );

        graph.add_node(node_a);
        graph.add_node(node_b);
        graph
            .add_edge(&fn_b, &fn_a, ChronosEdge::new(EdgeKind::Calls))
            .unwrap();

        let mut co_edge = ChronosEdge::new(EdgeKind::CoChangesWith);
        co_edge.call_site_line = Some(88);
        graph.add_edge(&fn_b, &fn_a, co_edge).unwrap();

        let md = synthesize_agents_md(&graph, Path::new("."), Some("demo-repo"));
        assert!(md.contains("# AGENTS.md — demo-repo"));
        assert!(md.contains("verify_token"));
        assert!(md.contains("High-Centrality Hub Symbols"));
        assert!(md.contains("Algorithmic Complexity & Loop Hotspots"));
        assert!(md.contains("88%"));
    }
}
