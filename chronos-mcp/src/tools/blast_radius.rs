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

    let st = state.read().await;

    if st.graph.status_report().total_nodes == 0 {
        return CallToolResult::text(
            "⚠️ **4D Graph is empty.**\n\nCall `chronos_analyze_file` first.",
        );
    }

    // Try to find the node — try multiple kinds + file hints
    // NodeKind variants: Function, Class, ApiRoute, TypeDefinition, TestCase
    let node_kinds = [
        NodeKind::Function,
        NodeKind::Class,
        NodeKind::ApiRoute,
        NodeKind::TestCase,
    ];

    // Collect all candidate NodeIds
    let mut candidate_id: Option<NodeId> = None;

    'outer: for kind in &node_kinds {
        // Try with file hint first
        if let Some(ref file) = file_hint {
            let id = NodeId::new(file, &symbol_name, kind);
            if st.graph.get_node(&id).is_some() {
                candidate_id = Some(id);
                break 'outer;
            }
        }

        // Try without file path (search all nodes by name)
        let status = st.graph.status_report();
        debug!("Searching {} nodes for symbol '{}'", status.total_nodes, symbol_name);
    }

    // Fallback: search graph for any node matching the name
    if candidate_id.is_none() {
        // Use the hot_path_nodes query to get a broader list and name-match
        let analyzer_result = {
            // Search by trying common file path patterns
            let possible_files = vec![
                format!("src/{}.ts", symbol_name.to_lowercase()),
                format!("src/services/{}.ts", symbol_name.to_lowercase()),
                format!("src/routes/{}.ts", symbol_name.to_lowercase()),
            ];

            let mut found = None;
            for file in &possible_files {
                for kind in &node_kinds {
                    let id = NodeId::new(file, &symbol_name, kind);
                    if st.graph.get_node(&id).is_some() {
                        found = Some(id);
                        break;
                    }
                }
                if found.is_some() { break; }
            }
            found
        };
        candidate_id = analyzer_result;
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

    // Generate full HTML dashboard for HIGH/CRITICAL risk
    if report.needs_visual_dashboard() {
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
    }

    // Append inline status line
    output.push_str(&format!("\n\n---\n_{}_", generate_inline_status(&report)));

    CallToolResult::text(output)
}
