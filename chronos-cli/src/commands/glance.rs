//! `chronos glance <file>` — 30-second Markdown impact summary

use anyhow::Result;
use chronos_core::{
    ast::ts_parser::TypeScriptParser,
    blast_radius::{
        impact_matrix::ImpactAnalyzer,
        markdown_glance::{generate_glance_markdown, generate_inline_status},
    },
    graph::schema::{ChronosGraph, NodeId, NodeKind},
};
use std::path::PathBuf;

pub async fn run(workspace: PathBuf, file: PathBuf, symbol: Option<String>) -> Result<()> {
    let abs_file = if file.is_absolute() { file } else { workspace.join(file) };

    let mut parser = TypeScriptParser::new()?;
    let parsed = parser.parse_file(&abs_file)?;

    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let analyzer = ImpactAnalyzer::new(&graph);

    if let Some(sym) = symbol {
        // Find specific symbol
        let candidate = graph
            .nodes_by_kind(&NodeKind::Function)
            .into_iter()
            .chain(graph.nodes_by_kind(&NodeKind::Class).into_iter())
            .find(|n| n.name == sym)
            .map(|n| n.id.clone());

        match candidate {
            Some(id) => {
                let report = analyzer.analyze(&id, false);
                println!("{}", generate_glance_markdown(&report));
                println!("---\n_{}_", generate_inline_status(&report));
            }
            None => {
                eprintln!("⚠️  Symbol '{}' not found in {}", sym, abs_file.display());
                eprintln!("Available: {}",
                    graph.nodes_by_kind(&NodeKind::Function)
                        .iter()
                        .map(|n| n.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                std::process::exit(1);
            }
        }
    } else {
        // Analyze all symbols in the file
        let fn_nodes = graph.nodes_by_kind(&NodeKind::Function);
        let class_nodes = graph.nodes_by_kind(&NodeKind::Class);
        let all_ids: Vec<&NodeId> = fn_nodes.iter()
            .chain(class_nodes.iter())
            .map(|n| &n.id)
            .collect();

        if all_ids.is_empty() {
            println!("ℹ️  No symbols found in {}", abs_file.display());
            return Ok(());
        }

        let report = analyzer.analyze_diff(&all_ids);
        println!("{}", generate_glance_markdown(&report));
        println!("---\n_{}_", generate_inline_status(&report));
    }

    Ok(())
}
