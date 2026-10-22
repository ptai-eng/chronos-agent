//! # CHRONOS CLI — Developer & Reviewer Command-Line Interface
//!
//! Three primary commands:
//! - `chronos audit <file>`   → Full mutation + blast-radius report
//! - `chronos glance <file>`  → 30-second Markdown summary
//! - `chronos mcp`            → Start the MCP server (delegates to chronos-mcp binary)
//! - `chronos graph`          → Query the 4D graph

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

// Subcommand modules
mod commands;

#[derive(Parser)]
#[command(
    name = "chronos",
    version = env!("CARGO_PKG_VERSION"),
    about = "CHRONOS-AGENT CLI — 4D Knowledge Graph for AI coding agents",
    long_about = r#"CHRONOS gives AI coding agents runtime sensation:

  chronos audit <file>   — Full mutation score + blast-radius analysis
  chronos glance <file>  — 30-second impact summary (Markdown)
  chronos graph <file>   — Query 4D graph: hotspots, callers, tests
  chronos mcp            — Print MCP server startup command (copy-paste)

Designed for: Antigravity · Cursor · Codex · OpenCode · Claude · Cline · Windsurf · Continue · Zed"#
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Workspace root directory
    #[arg(short, long, global = true, default_value = ".")]
    workspace: PathBuf,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run full mutation gate + blast-radius analysis on a file
    Audit {
        /// TypeScript/Python/Rust file to audit
        file: PathBuf,
        /// Use strict 90% mutation gate (default: 80%)
        #[arg(long)]
        strict: bool,
        /// Test runner command
        #[arg(long, default_value = "npx jest --passWithNoTests")]
        test_cmd: String,
    },
    /// Targeted blast-radius on git-changed functions (no full file scan needed)
    Diff {
        /// Analyze staged changes instead of unstaged
        #[arg(long)]
        staged: bool,
        /// Analyze a specific commit SHA
        #[arg(long)]
        commit: Option<String>,
        /// Analyze a PR range (e.g. "main..feature-branch")
        #[arg(long)]
        range: Option<String>,
        /// Output as JSON (for CI script consumption)
        #[arg(long)]
        json: bool,
        /// Exit with code 1 if any HIGH/CRITICAL risk is detected (CI gate)
        #[arg(long)]
        ci: bool,
    },
    /// Quick 30-second Markdown impact summary for a function
    Glance {
        /// TypeScript file to analyze
        file: PathBuf,
        /// Function/symbol name to focus on
        #[arg(short, long)]
        symbol: Option<String>,
    },
    /// Query the 4D graph for hotspots, callers, complexity
    Graph {
        /// File(s) to analyze and insert into graph
        files: Vec<PathBuf>,
        /// Query type: hotspots | callers | complexity | tests
        #[arg(short, long, default_value = "hotspots")]
        query: String,
        /// Target symbol name for callers/tests queries
        #[arg(short, long)]
        symbol: Option<String>,
    },
    /// Show MCP server startup commands for all supported AI clients
    Mcp {
        /// Show config for a specific client: all | antigravity | cursor | codex | opencode | claude | cline | windsurf | continue | zed
        #[arg(short, long, default_value = "all")]
        client: String,
    },
    /// Check contract broker status — drain any pending breaking-change alerts
    Status {
        /// Directory to watch for contract changes
        #[arg(short, long)]
        watch: Option<PathBuf>,
    },
    /// Execute a Read-Only Cypher query directly against the embedded KuzuDB graph
    Query {
        /// Read-Only Cypher query string (e.g. "MATCH (n:ChronosNode) RETURN n.name, n.loop_depth LIMIT 10")
        cypher: String,
        /// Optional files or directories to index before querying
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Max rows to return (default: 200)
        #[arg(long, default_value_t = 200)]
        max_rows: usize,
    },
    /// BM25 + Regex + Complexity search across the 4D Knowledge Graph
    Search {
        /// Natural-language or keyword query (splits camelCase & snake_case)
        #[arg(short, long)]
        query: Option<String>,
        /// Regex pattern for symbol name
        #[arg(short, long)]
        pattern: Option<String>,
        /// Minimum nested loop depth (transitive_loop_depth)
        #[arg(long)]
        min_loop_depth: Option<u32>,
        /// Optional files or directories to index
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Max results (default: 25)
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
    },
    /// Retrieve verbatim source code snippet + callers, callees, tests, and AST complexity
    Snippet {
        /// Symbol name (function/method/class)
        symbol: String,
        /// Optional file path filter
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    /// Generate workspace architecture report with community clusters and O(N^2) hotspots
    Arch {
        /// Optional directory prefix to scope architecture analysis
        path: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Hydrate the 4D Knowledge Graph from an offline profiler artifact (.cpuprofile, Speedscope, Folded Flamegraph, OTLP JSON)
    Hydrate {
        /// Path to the offline profile file
        profile: PathBuf,
        /// Optional files or directories to index before hydrating
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Synthesize a live AGENTS.md specification from the 5D Knowledge Graph for OpenAI Codex CLI & ChatGPT Codex
    AgentsMd {
        /// Optional directory or files to index (defaults to workspace root)
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Optional project title override
        #[arg(long)]
        name: Option<String>,
        /// Write directly to disk (default output file: AGENTS.md)
        #[arg(long)]
        write: bool,
        /// Output file path when --write is enabled (default: AGENTS.md)
        #[arg(short, long, default_value = "AGENTS.md")]
        output: PathBuf,
    },
    /// Export all 11 CHRONOS tool schemas for OpenAI Function Calling, OpenAI Responses API, or Codex CLI TOML
    Schema {
        /// Output format: openai | responses | codex-toml | mcp
        #[arg(short, long, default_value = "openai")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { Level::DEBUG } else { Level::WARN };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let workspace_raw = cli.workspace.canonicalize()
        .unwrap_or_else(|_| cli.workspace.clone());
    let workspace = PathBuf::from(
        workspace_raw.to_string_lossy().trim_start_matches(r"\\?\")
    );

    match cli.command {
        Commands::Audit { file, strict, test_cmd } => {
            commands::audit::run(workspace, file, strict, test_cmd).await
        }
        Commands::Diff { staged, commit, range, json, ci } => {
            use commands::diff_audit::{get_changed_symbols, DiffMode, DiffBlastRadiusReport, SymbolImpact, format_markdown, format_json};

            let mode = if let Some(sha) = commit {
                DiffMode::Commit(sha)
            } else if let Some(r) = range {
                let parts: Vec<&str> = r.splitn(2, "..").collect();
                if parts.len() == 2 {
                    DiffMode::Range(parts[0].to_string(), parts[1].to_string())
                } else {
                    DiffMode::Unstaged
                }
            } else if staged {
                DiffMode::Staged
            } else {
                DiffMode::Unstaged
            };

            let symbols = match get_changed_symbols(&workspace, mode) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ {}", e);
                    std::process::exit(1);
                }
            };

            // Build a 4D graph across changed files and compute real blast-radius impact
            use chronos_core::{
                ast::dispatcher::LanguageDispatcher,
                blast_radius::impact_matrix::{ImpactAnalyzer, RiskZone},
                graph::schema::{ChronosGraph, NodeKind},
            };
            use std::collections::HashSet;

            let mut graph = ChronosGraph::new();
            if let Ok(mut dispatcher) = LanguageDispatcher::new() {
                let unique_files: HashSet<_> = symbols.iter().map(|s| s.file.clone()).collect();
                for file in unique_files {
                    if file.exists() && LanguageDispatcher::is_supported(&file) {
                        if let Ok(parsed) = dispatcher.parse_file(&file) {
                            graph.ingest_parsed_module(&parsed);
                        }
                    }
                }
                graph.resolve_pending_calls_and_routes();
            }
            chronos_core::graph::git_coupling::ingest_git_co_changes(&mut graph, &workspace, 200);

            let analyzer = ImpactAnalyzer::new(&graph);
            let all_nodes = graph.all_nodes();
            let mut changed_ids = Vec::new();

            let symbol_reports: Vec<SymbolImpact> = symbols.iter().map(|s| {
                let matched_node = all_nodes.iter().find(|n| {
                    n.name == s.symbol_name && n.location.file == s.file
                }).or_else(|| {
                    all_nodes.iter().find(|n| n.name == s.symbol_name)
                });

                if let Some(node) = matched_node {
                    changed_ids.push(node.id.clone());
                    let impact = analyzer.analyze(&node.id, false);
                    let covering_tests = graph.tests_for(&node.id)
                        .into_iter()
                        .map(|t| t.name.clone())
                        .collect();
                    SymbolImpact {
                        symbol: s.symbol_name.clone(),
                        file: s.file.to_string_lossy().to_string(),
                        risk_level: impact.overall_risk.label().to_string(),
                        direct_callers: impact.direct_callers_count,
                        transitive_callers: impact.transitive_callers_count,
                        recommended_test_modules: covering_tests,
                    }
                } else {
                    SymbolImpact {
                        symbol: s.symbol_name.clone(),
                        file: s.file.to_string_lossy().to_string(),
                        risk_level: "SAFE".to_string(),
                        direct_callers: 0,
                        transitive_callers: 0,
                        recommended_test_modules: vec![],
                    }
                }
            }).collect();

            let id_refs: Vec<_> = changed_ids.iter().collect();
            let diff_summary = analyzer.analyze_diff(&id_refs);
            let has_high_risk = matches!(diff_summary.overall_risk, RiskZone::High | RiskZone::Critical);
            let overall_risk_str = if symbols.is_empty() {
                "SAFE (0 changed symbols)".to_string()
            } else {
                format!(
                    "{} ({:.0}% impact, coverage: {})",
                    diff_summary.overall_risk.label(),
                    diff_summary.impact_score * 100.0,
                    diff_summary.coverage.completeness,
                )
            };

            let _ = NodeKind::Function; // keep import clean
            let report = DiffBlastRadiusReport {
                changed_symbols: symbols,
                symbol_reports,
                overall_risk: overall_risk_str,
                exit_code_for_ci: if has_high_risk { 1 } else { 0 },
            };

            if json {
                println!("{}", format_json(&report));
            } else {
                println!("{}", format_markdown(&report));
                println!("\n💡 For full mutation + blast-radius audit, run: chronos audit <file>");
            }

            if ci && has_high_risk {
                std::process::exit(1);
            }
            Ok(())
        }
        Commands::Glance { file, symbol } => {
            commands::glance::run(workspace, file, symbol).await
        }
        Commands::Graph { files, query, symbol } => {
            commands::graph::run(workspace, files, query, symbol).await
        }
        Commands::Mcp { client } => {
            commands::mcp_info::run(client)
        }
        Commands::Status { watch } => {
            commands::status::run(workspace, watch).await
        }
        Commands::Query { cypher, files, max_rows } => {
            use chronos_core::graph::persistent::PersistentGraph;
            let graph = build_cli_graph(&workspace, &files)?;
            let db_path = workspace.join(".chronos").join("graph.kuzu");
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let db = PersistentGraph::open(&db_path)?;
            db.sync_from_graph(&graph)?;
            let res = db.execute_readonly_cypher(&cypher, max_rows)?;
            println!("{}", serde_json::to_string_pretty(&res)?);
            Ok(())
        }
        Commands::Search { query, pattern, min_loop_depth, files, limit } => {
            use chronos_core::graph::query::{GraphQueryEngine, GraphSearchRequest};
            let graph = build_cli_graph(&workspace, &files)?;
            let qe = GraphQueryEngine::new(&graph);
            let resp = qe.search_graph(&GraphSearchRequest {
                query,
                name_pattern: pattern,
                min_loop_depth,
                limit,
                ..Default::default()
            });
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        }
        Commands::Snippet { symbol, file } => {
            use chronos_core::graph::query::GraphQueryEngine;
            let files = file.iter().cloned().collect::<Vec<_>>();
            let graph = build_cli_graph(&workspace, &files)?;
            let qe = GraphQueryEngine::new(&graph);
            let file_filter = file.as_ref().map(|f| f.to_string_lossy().to_string());
            match qe.get_code_snippet(&symbol, file_filter.as_deref()) {
                Some(snip) => println!("{}", serde_json::to_string_pretty(&snip)?),
                None => {
                    eprintln!("Symbol '{}' not found in workspace", symbol);
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Commands::Arch { path, json } => {
            use chronos_core::graph::query::GraphQueryEngine;
            let files = path.iter().cloned().collect::<Vec<_>>();
            let graph = build_cli_graph(&workspace, &files)?;
            let qe = GraphQueryEngine::new(&graph);
            let prefix = path.as_ref().map(|p| p.to_string_lossy().to_string());
            let arch = qe.get_architecture(prefix.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&arch)?);
            } else {
                println!(
                    "CHRONOS Architecture Report ({} nodes, {} edges, {} clusters)",
                    arch.total_nodes,
                    arch.total_edges,
                    arch.clusters.len()
                );
                println!("Languages: {:?}", arch.languages);
                println!("\nTop Community Clusters:");
                for c in arch.clusters.iter().take(10) {
                    println!(
                        "  - {} ({} symbols, cohesion: {:.0}%, top: {})",
                        c.label,
                        c.member_count,
                        c.cohesion_score * 100.0,
                        c.top_nodes.join(", ")
                    );
                }
                if !arch.algorithmic_hotspots.is_empty() {
                    println!("\nAlgorithmic Hotspots (Nested Loops / Linear Scans in Loops / Recursion):");
                    for h in arch.algorithmic_hotspots.iter().take(10) {
                        println!(
                            "  - {} ({}:{}) — transitive_loop_depth={}, linear_scan_in_loop={}, recursive={}",
                            h.name, h.file, h.line, h.transitive_loop_depth, h.linear_scan_in_loop, h.is_recursive
                        );
                    }
                }
            }
            Ok(())
        }
        Commands::Hydrate { profile, files, json } => {
            use chronos_core::{
                graph::persistent::PersistentGraph,
                telemetry::{hydration::hydrate_graph, profile_loader::load_profile_from_file},
            };
            let profile_abs = if profile.is_absolute() {
                profile
            } else {
                workspace.join(&profile)
            };
            let session = load_profile_from_file(&profile_abs)?;
            let mut graph = build_cli_graph(&workspace, &files)?;
            let report = hydrate_graph(&mut graph, &session);

            let db_path = workspace.join(".chronos").join("graph.kuzu");
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(db) = PersistentGraph::open(&db_path) {
                let _ = db.sync_from_graph(&graph);
            }

            if json {
                let out = serde_json::json!({
                    "backend": session.backend,
                    "total_duration_ms": session.total_duration_ms,
                    "total_profiled_functions": report.total_profiled,
                    "hydrated_nodes": report.hydrated_count,
                    "cold_nodes": report.cold_count,
                    "bottlenecks_found": report.bottlenecks_found,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{}", session.summary());
                println!("{}", report);
            }
            Ok(())
        }
        Commands::AgentsMd { files, name, write, output } => {
            use chronos_core::graph::agents_md::synthesize_agents_md;
            let graph = build_cli_graph(&workspace, &files)?;
            let md = synthesize_agents_md(&graph, &workspace, name.as_deref());
            if write {
                let out_path = if output.is_absolute() {
                    output
                } else {
                    workspace.join(&output)
                };
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&out_path, &md)?;
                println!(
                    "✅ Synthesized OpenAI Codex AGENTS.md ({} bytes, {} graph nodes) -> {}",
                    md.len(),
                    graph.node_count(),
                    out_path.display()
                );
            } else {
                print!("{md}");
            }
            Ok(())
        }
        Commands::Schema { format } => {
            let out = commands::mcp_info::export_schema(&format)?;
            println!("{out}");
            Ok(())
        }
    }
}

fn build_cli_graph(workspace: &std::path::Path, explicit_files: &[PathBuf]) -> Result<chronos_core::graph::schema::ChronosGraph> {
    use chronos_core::{
        ast::dispatcher::LanguageDispatcher,
        graph::schema::ChronosGraph,
    };
    let mut graph = ChronosGraph::new();
    let mut dispatcher = LanguageDispatcher::new()?;
    let mut targets = Vec::new();

    if explicit_files.is_empty() {
        collect_cli_files(workspace, &mut targets, 300);
    } else {
        for p in explicit_files {
            let resolved = if p.is_absolute() { p.clone() } else { workspace.join(p) };
            if resolved.is_dir() {
                collect_cli_files(&resolved, &mut targets, 300);
            } else if resolved.exists() {
                targets.push(resolved);
            }
        }
    }

    for file in targets {
        if LanguageDispatcher::is_supported(&file) {
            if let Ok(parsed) = dispatcher.parse_file(&file) {
                graph.ingest_parsed_module(&parsed);
            }
        }
    }
    graph.resolve_pending_calls_and_routes();
    chronos_core::graph::git_coupling::ingest_git_co_changes(&mut graph, workspace, 200);
    Ok(graph)
}

fn collect_cli_files(dir: &std::path::Path, out: &mut Vec<PathBuf>, max_files: usize) {
    use chronos_core::ast::dispatcher::LanguageDispatcher;
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
                ".git" | "node_modules" | "target" | ".venv" | "venv" | ".chronos" | "dist" | "build" | "out"
            ) {
                continue;
            }
            collect_cli_files(&path, out, max_files);
        } else if LanguageDispatcher::is_supported(&path) {
            out.push(path);
        }
    }
}
