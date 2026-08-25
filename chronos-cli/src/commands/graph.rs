//! `chronos graph <files...>` — Query the 4D knowledge graph

use anyhow::Result;
use chronos_core::{
    ast::ts_parser::TypeScriptParser,
    graph::{query::GraphQueryEngine, schema::ChronosGraph},
};
use std::path::PathBuf;

pub async fn run(
    workspace: PathBuf,
    files: Vec<PathBuf>,
    query: String,
    symbol: Option<String>,
) -> Result<()> {
    if files.is_empty() {
        eprintln!("❌ Provide at least one TypeScript file to analyze.");
        eprintln!("   Example: chronos graph src/services/orderService.ts --query hotspots");
        std::process::exit(1);
    }

    // Build graph from all provided files
    let mut parser = TypeScriptParser::new()?;
    let mut graph = ChronosGraph::new();

    for file in &files {
        let abs_file = if file.is_absolute() { file.clone() } else { workspace.join(file) };
        match parser.parse_file(&abs_file) {
            Ok(parsed) => {
                let count = parsed.all_nodes().len();
                for node in parsed.all_nodes() {
                    graph.add_node(node.clone());
                }
                println!("✓ Parsed {} ({} symbols)", abs_file.display(), count);
            }
            Err(e) => eprintln!("⚠️  Skipping {}: {}", file.display(), e),
        }
    }

    let report = graph.status_report();
    println!("\n{}", report);

    let engine = GraphQueryEngine::new(&graph);

    println!("\n{}", "─".repeat(60));
    match query.as_str() {
        "hotspots" | "bottlenecks" => {
            let result = engine.find_bottlenecks();
            println!("## {} Bottleneck Results\n{}", result.nodes_found, result.summary);
            for (i, node) in result.details.iter().enumerate() {
                println!(
                    "  {}. `{}` in {} (line {})",
                    i + 1, node.name, shorten(&node.file), node.line
                );
                if let Some(t) = &node.telemetry_summary {
                    println!(
                        "     p50={:.1}ms p99={:.1}ms cpu={:.1}% calls={}",
                        t.exec_time_p50_ms, t.exec_time_p99_ms, t.cpu_percent, t.invocation_count
                    );
                } else {
                    println!("     cognitive load: {:.0}", node.cognitive_load);
                }
            }

            // Also show high-complexity
            let complex = engine.high_complexity_functions(40.0);
            if !complex.details.is_empty() {
                println!("\n## {} High Complexity Functions (Static)\n{}", complex.nodes_found, complex.summary);
                for (i, node) in complex.details.iter().enumerate() {
                    println!(
                        "  {}. `{}` — cognitive load: {:.0} ({} lines, {} params)",
                        i + 1, node.name, node.cognitive_load,
                        0, // sloc not in NodeSummary
                        0,
                    );
                }
            }
        }

        "callers" => {
            let sym = symbol.as_deref().unwrap_or_else(|| {
                eprintln!("❌ --symbol required for 'callers' query");
                std::process::exit(1);
            });

            let target = graph
                .nodes_by_kind(&chronos_core::graph::schema::NodeKind::Function)
                .into_iter()
                .chain(graph.nodes_by_kind(&chronos_core::graph::schema::NodeKind::Class).into_iter())
                .find(|n| n.name == sym)
                .map(|n| n.id.clone());

            match target {
                Some(id) => {
                    let result = engine.callers_chain(&id, 10);
                    println!("## Callers of `{}`\n{}", sym, result.summary);
                    for (i, node) in result.details.iter().enumerate() {
                        println!("  {}. `{}` in {}", i + 1, node.name, shorten(&node.file));
                    }
                }
                None => eprintln!("Symbol '{}' not found in graph", sym),
            }
        }

        "tests" => {
            let sym = symbol.as_deref().unwrap_or_else(|| {
                eprintln!("❌ --symbol required for 'tests' query");
                std::process::exit(1);
            });

            let target = graph
                .nodes_by_kind(&chronos_core::graph::schema::NodeKind::Function)
                .into_iter()
                .find(|n| n.name == sym)
                .map(|n| n.id.clone());

            match target {
                Some(id) => {
                    let result = engine.covering_tests(&id);
                    println!("## Tests covering `{}`\n{}", sym, result.summary);
                    for (i, node) in result.details.iter().enumerate() {
                        println!("  {}. `{}` in {}", i + 1, node.name, shorten(&node.file));
                    }
                }
                None => eprintln!("Symbol '{}' not found in graph", sym),
            }
        }

        "complexity" => {
            let result = engine.high_complexity_functions(20.0);
            println!("## {} High Complexity Functions\n{}", result.nodes_found, result.summary);
            for (i, node) in result.details.iter().enumerate() {
                println!(
                    "  {}. `{}` in {} — cognitive load: {:.0}",
                    i + 1, node.name, shorten(&node.file), node.cognitive_load
                );
            }
        }

        unknown => {
            eprintln!("❌ Unknown query type: '{}'. Use: hotspots | callers | tests | complexity", unknown);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn shorten(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    if parts.len() > 2 {
        format!("…/{}", parts[parts.len() - 2..].join("/"))
    } else {
        path.to_string()
    }
}
