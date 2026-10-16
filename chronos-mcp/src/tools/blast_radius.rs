//! # Tool: chronos_blast_radius
//!
//! Analyze the blast-radius of changing a specific function/symbol.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::{
    blast_radius::{
        impact_matrix::ImpactAnalyzer,
        markdown_glance::{generate_glance_markdown, generate_inline_status},
        visualizer::generate_html_dashboard,
    },
    graph::schema::{NodeId, NodeKind},
};
use serde_json::Value;
use tracing::debug;

pub async fn handle_blast_radius(state: SharedState, params: Option<Value>) -> CallToolResult {
    let symbol_name = match params.as_ref()
        .and_then(|p| p.get("symbol_name"))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return CallToolResult::error("Missing required parameter: symbol_name"),
    };

    let file_hint = params.as_ref()
        .and_then(|p| p.get("file_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let contract_breaking = params.as_ref()
        .and_then(|p| p.get("contract_breaking"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let detail = params.as_ref()
        .and_then(|p| p.get("detail"))
        .and_then(|v| v.as_str())
        .unwrap_or("summary");

    let st = state.read().await;

    if st.graph.status_report().total_nodes == 0 {
        return CallToolResult::text(
            "⚠️ **4D Graph is empty.**\n\nCall `chronos_analyze_file` first.",
        );
    }

    // Try to find the node — try multiple kinds + file hints, then search all nodes in graph
    let node_kinds = [
        NodeKind::Function,
        NodeKind::Class,
        NodeKind::ApiRoute,
        NodeKind::TestCase,
    ];

    let mut candidate_id: Option<NodeId> = None;

    if let Some(ref file) = file_hint {
        for kind in &node_kinds {
            let id = NodeId::new(file, &symbol_name, kind);
            if st.graph.get_node(&id).is_some() {
                candidate_id = Some(id);
                break;
            }
        }
    }

    // Fallback: search all nodes in the graph by exact name or qualified_name
    if candidate_id.is_none() {
        debug!("Searching all nodes for symbol '{}'", symbol_name);
        let all_nodes = st.graph.all_nodes();
        if let Some(matched) = all_nodes.iter().find(|n| {
            let file_matches = file_hint.as_ref().map_or(true, |hint| {
                n.location.file.to_string_lossy().ends_with(hint.as_str())
            });
            file_matches && (n.name == symbol_name || n.qualified_name == symbol_name)
        }) {
            candidate_id = Some(matched.id.clone());
        } else if let Some(matched) = all_nodes.iter().find(|n| {
            n.name == symbol_name || n.qualified_name == symbol_name
        }) {
            candidate_id = Some(matched.id.clone());
        }
    }

    let node_id = match candidate_id {
        Some(id) => id,
        None => {
            return CallToolResult::text(format!(
                "⚠️ **Symbol `{}` not found in 4D graph.**\n\n\
                 Make sure to call `chronos_analyze_file` on the file containing this symbol first.\n\
                 If you provided a file_path, verify it matches the analyzed file.",
                symbol_name
            ));
        }
    };

    // Run blast-radius analysis
    let analyzer = ImpactAnalyzer::new(&st.graph);
    let report = analyzer.analyze(&node_id, contract_breaking);

    // Always generate the 30-second Markdown glance
    let mut output = generate_glance_markdown(&report);

    // Generate full HTML dashboard only when detail == "complete" (or explicitly requested)
    if detail == "complete" && report.needs_visual_dashboard() {
        let html = generate_html_dashboard(&report);

        // Write HTML to temp file
        let html_path = std::env::temp_dir()
            .join(format!("chronos_blast_radius_{}.html", symbol_name));

        match std::fs::write(&html_path, &html) {
            Ok(_) => {
                output.push_str(&format!(
                    "\n\n📊 **Full HTML Dashboard:** `{}`\n\
                     Open in browser for interactive blast-radius visualization.",
                    html_path.display()
                ));
            }
            Err(e) => {
                output.push_str(&format!("\n\n⚠️ Could not write HTML dashboard: {}", e));
            }
        }
    } else if detail == "summary" && report.needs_visual_dashboard() {
        output.push_str(
            "\n\n> 💡 Pass `detail: \"complete\"` to generate the standalone HTML blast-radius dashboard.",
        );
    }

    // Append provenance & inline status line
    output.push_str(&format!(
        "\n\n---\n_Provenance: `_meta.provenance.untrusted_content = true` | {}_",
        generate_inline_status(&report)
    ));

    CallToolResult::text(output)
}
