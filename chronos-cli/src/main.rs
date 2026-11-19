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
    about = "CHRONOS-AGENT CLI — 5D Code Graph for AI coding agents",
    long_about = r#"CHRONOS gives AI coding agents runtime sensation:

  chronos audit <file>   — Full mutation score + blast-radius analysis
  chronos glance <file>  — 30-second impact summary (Markdown)
  chronos graph <file>   — Query 5D graph: hotspots, callers, tests
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
    /// Query the 5D Code Graph for hotspots, callers, complexity
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
    /// BM25 + Regex + Complexity search across the 5D Code Graph
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
    /// Hydrate the 5D Code Graph from an offline profiler artifact (.cpuprofile, Speedscope, Folded Flamegraph, OTLP JSON)
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
    /// Synthesize a live AGENTS.md / CLAUDE.md / GEMINI.md / .mdc specification from the 5D Knowledge Graph
    AgentsMd {
        /// Target AI CLI/IDE ecosystem: codex | antigravity | claude | opencode | cursor | windsurf | copilot | cline
        #[arg(short, long, default_value = "codex")]
        target: String,
        /// Optional directory or files to index (defaults to workspace root)
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Optional project title override
        #[arg(long)]
        name: Option<String>,
        /// Write directly to disk
        #[arg(long)]
        write: bool,
        /// Optional output file path override when --write is enabled
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Auto-scaffold native 5D rules, skills, slash commands, and MCP configs for any AI coding CLI/IDE
    Init {
        /// Target client ecosystem: all | codex | antigravity | claude | opencode | cursor | windsurf | copilot | cline
        #[arg(short, long, default_value = "all")]
        client: String,
        /// Optional files or directories to index before synthesizing
        #[arg(short, long)]
        files: Vec<PathBuf>,
        /// Optional project title override
        #[arg(long)]
        name: Option<String>,
        /// Custom MCP server binary command (default: "chronos-mcp")
        #[arg(long, default_value = "chronos-mcp")]
        binary: String,
        /// Also scaffold zero-latency (<20ms) lifecycle hook configs (.agents/hooks.json, .claude/settings.json)
        #[arg(long)]
        with_hooks: bool,
    },
    /// Export all 14 CHRONOS tool schemas for OpenAI, Anthropic Claude, Google Gemini, or Codex CLI TOML
    Schema {
        /// Output format: openai | responses | anthropic | gemini | codex-toml | mcp
        #[arg(short, long, default_value = "openai")]
        format: String,
    },
    /// 1-Call Subagent Context Engine: BM25 search + Class/* outlines + blast-radius + 4-angle audit
    Explore {
        /// Natural-language question, symbol name, 'Class/*' wildcard, or regex pattern to explore
        query: String,
        /// Optional additional batch queries
        #[arg(long)]
        queries: Vec<String>,
        /// Optional file path filter
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Maximum number of symbol bundles to return (default: 3, max: 20)
        #[arg(short, long, default_value_t = 3)]
        max_symbols: usize,
        /// Omit full source bodies and return signature outlines only (saves tokens)
        #[arg(long)]
        no_body: bool,
        /// Class/module outline expansion depth (default: 1)
        #[arg(long, default_value_t = 1)]
        depth: usize,
        /// Attach inline RiskZone + caller counts per explored symbol
        #[arg(long)]
        blast_radius: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Surgical AST-bounded symbol edit with fresh disk parse + 2-stage syntax & blast-radius gate
    Edit {
        /// Symbol name or 'Class/method' path to edit
        symbol: String,
        /// Target file inside workspace
        #[arg(short, long)]
        file: PathBuf,
        /// Edit operation: replace_body | insert_before | insert_after
        #[arg(long, default_value = "replace_body")]
        op: String,
        /// Inline replacement/inserted source code
        #[arg(long)]
        code: Option<String>,
        /// Path to a file containing replacement/inserted source code
        #[arg(long)]
        code_file: Option<PathBuf>,
        /// Preview diff and contract checks without writing disk
        #[arg(long)]
        dry_run: bool,
        /// Force edit even when signature changes on a symbol with active callers
        #[arg(long)]
        force: bool,
        /// Output structured JSON result
        #[arg(long)]
        json: bool,
    },
    /// Graph-edge-verified multi-file symbol rename with ambiguous/candidate guard and atomic rollback
    Rename {
        /// Existing symbol name or 'Class/method' path
        symbol: String,
        /// New identifier name
        new_name: String,
        /// Optional defining source file hint
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Preview multi-file usages & diff without writing disk
        #[arg(long)]
        dry_run: bool,
        /// Force applying verified edits even when homonyms/unresolved/candidate sites exist
        #[arg(long)]
        force: bool,
        /// Output structured JSON result
        #[arg(long)]
        json: bool,
    },
    /// Hardened AST safe-delete verifying CALLS, TESTS, ApiRoute, entrypoint, visibility, and trait impls
    Delete {
        /// Target symbol name or 'Class/method' path to remove
        symbol: String,
        /// Optional defining source file hint
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Preview deletion diff & blockers without modifying disk
        #[arg(long)]
        dry_run: bool,
        /// Force deletion despite active dependents or visibility/entrypoint guards
        #[arg(long)]
        force: bool,
        /// Output structured JSON result
        #[arg(long)]
        json: bool,
    },
    /// Evaluate a bounded Rhai 5D graph query or chained refactor script
    Eval {
        /// Rhai script source string
        script: String,
        /// Restrict script execution to ReadOnly 5D graph profile
        #[arg(long)]
        readonly: bool,
        /// Output full structured JSON outcome
        #[arg(long)]
        json: bool,
    },
    /// Launch an interactive Rhai 5D Code Intelligence REPL
    Repl {
        /// Run interactive REPL in ReadOnly query mode (disables edit_symbol/refactor_symbol)
        #[arg(long)]
        readonly: bool,
    },
    /// Zero-latency (<20ms), fail-open lifecycle hook handler for Antigravity (.agents/hooks.json) and Claude Code (.claude/settings.json)
    Hook {
        /// Hook event name: session-start | post-edit
        event: String,
        /// Target client hook schema: antigravity | claude | codex
        #[arg(short, long, default_value = "antigravity")]
        client: String,
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
            use chronos_core::session::WorkspaceSession;
            let mut session = WorkspaceSession::open(workspace, false)?;
            session.ensure_indexed_files(&files, 300)?;
            let repo = session
                .repo()
                .ok_or_else(|| anyhow::anyhow!("KuzuDB persistent engine is unavailable"))?;
            let res = repo.execute_readonly_cypher(&cypher, max_rows)?;
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
                session::WorkspaceSession,
                telemetry::{hydration::hydrate_graph, profile_loader::load_profile_from_file},
            };
            let profile_abs = if profile.is_absolute() {
                profile
            } else {
                workspace.join(&profile)
            };
            let telemetry_session = load_profile_from_file(&profile_abs)?;
            let mut session = WorkspaceSession::open(workspace, false)?;
            session.ensure_indexed_files(&files, 300)?;
            let report = hydrate_graph(&mut session.graph, &telemetry_session);

            if let Some(ref repo) = session.repo {
                let _ = repo.sync_from_graph(&session.graph);
            }

            if json {
                let out = serde_json::json!({
                    "backend": telemetry_session.backend,
                    "total_duration_ms": telemetry_session.total_duration_ms,
                    "total_profiled_functions": report.total_profiled,
                    "hydrated_nodes": report.hydrated_count,
                    "cold_nodes": report.cold_count,
                    "bottlenecks_found": report.bottlenecks_found,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{}", telemetry_session.summary());
                println!("{}", report);
            }
            Ok(())
        }
        Commands::AgentsMd { target, files, name, write, output } => {
            use chronos_core::graph::agents_md::{synthesize_target_context, AgentTarget};
            let agent_target = AgentTarget::from_str_loose(&target);
            let graph = build_cli_graph(&workspace, &files)?;
            let md = synthesize_target_context(&graph, &workspace, agent_target, name.as_deref());
            if write {
                let rel_or_abs = output.unwrap_or_else(|| PathBuf::from(agent_target.default_context_path()));
                let out_path = if rel_or_abs.is_absolute() {
                    rel_or_abs
                } else {
                    workspace.join(&rel_or_abs)
                };
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&out_path, &md)?;
                println!(
                    "✅ Synthesized {} ({} bytes, {} graph nodes) -> {}",
                    agent_target.display_name(),
                    md.len(),
                    graph.node_count(),
                    out_path.display()
                );
            } else {
                print!("{md}");
            }
            Ok(())
        }
        Commands::Init { client, files, name, binary, with_hooks } => {
            use chronos_core::graph::agents_md::{scaffold_client_files_with_options, AgentTarget};
            let agent_target = AgentTarget::from_str_loose(&client);
            let graph = build_cli_graph(&workspace, &files)?;
            let bundle = scaffold_client_files_with_options(
                &graph,
                &workspace,
                agent_target,
                &binary,
                name.as_deref(),
                with_hooks,
            );
            println!(
                "🚀 Scaffolding CHRONOS 5D Integration for {} ({} graph nodes indexed)...",
                agent_target.display_name(),
                graph.node_count()
            );
            for item in &bundle {
                let full_path = workspace.join(&item.relative_path);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&full_path, &item.content)?;
                println!(
                    "  ✓ {:<45} ({})",
                    item.relative_path, item.target_client
                );
            }
            println!(
                "\n✅ Generated {} native integration file(s) in {}",
                bundle.len(),
                workspace.display()
            );
            Ok(())
        }
        Commands::Schema { format } => {
            let out = commands::mcp_info::export_schema(&format)?;
            println!("{out}");
            Ok(())
        }
        Commands::Explore {
            query,
            queries,
            file,
            max_symbols,
            no_body,
            depth,
            blast_radius,
            json,
        } => {
            use chronos_core::graph::query::{ExploreOptions, GraphQueryEngine};
            let files = file.iter().cloned().collect::<Vec<_>>();
            let graph = build_cli_graph(&workspace, &files)?;
            let qe = GraphQueryEngine::new(&graph);
            let file_filter = file.as_ref().map(|f| f.to_string_lossy().to_string());
            let resp = qe.explore_graph_with_options(
                &query,
                &ExploreOptions {
                    queries,
                    file_filter,
                    max_symbols,
                    include_body: !no_body,
                    depth,
                    include_blast_radius: blast_radius,
                },
            );
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                println!("🧭 CHRONOS 1-Call Explore: {}", resp.summary);
                for (i, b) in resp.bundles.iter().enumerate() {
                    println!(
                        "\n── [{}/{}] {} ({}:{}-{}, score: {:.1}, PageRank: {:.4}) ──",
                        i + 1,
                        resp.bundles.len(),
                        b.snippet.name,
                        b.snippet.file,
                        b.snippet.start_line,
                        b.snippet.end_line,
                        b.score,
                        b.snippet.pagerank_score
                    );
                    if let Some(ref br) = b.blast_radius {
                        println!(
                            "Blast-Radius: {} (score {:.2}, direct={}, transitive={})",
                            br.risk_zone, br.impact_score, br.direct_callers_count, br.transitive_callers_count
                        );
                    }
                    println!("Callers: {:?} | Callees: {:?} | Tests: {:?}", b.snippet.callers, b.snippet.callees, b.snippet.covering_tests);
                    println!("{}\n", b.snippet.source_code);
                }
                if !resp.verification_surfaces.is_empty() {
                    println!("🎯 Verification Surfaces (End-to-End Entrypoints):");
                    for s in &resp.verification_surfaces {
                        println!("  • [{}] {} ({}:{}) — {}", s.surface_kind, s.name, s.file, s.line, s.suggested_verification);
                    }
                }
                if !resp.four_angle_diagnostics.is_empty() {
                    println!("\n🔍 4-Angle Code Review Diagnostics:");
                    for d in &resp.four_angle_diagnostics {
                        println!("  • [{}] {} ({}:{}) — {} -> {}", d.angle, d.symbol, d.file, d.line, d.summary, d.recommendation);
                    }
                }
            }
            Ok(())
        }
        Commands::Edit {
            symbol,
            file,
            op,
            code,
            code_file,
            dry_run,
            force,
            json,
        } => {
            use chronos_core::ast::symbol_editor::{EditOperation, EditSymbolRequest};
            let new_code = match (code, code_file) {
                (Some(c), _) => c,
                (None, Some(path)) => std::fs::read_to_string(&path)?,
                (None, None) => anyhow::bail!("Provide either --code <str> or --code-file <path>"),
            };
            let operation = EditOperation::parse(&op)?;
            let mut session = chronos_core::session::WorkspaceSession::open(workspace.clone(), false)?;
            session.ensure_indexed(None, 400)?;
            let outcome = session
                .edit_symbol(EditSymbolRequest {
                    symbol,
                    file,
                    operation,
                    new_code,
                    dry_run,
                    force,
                })
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!("[{}] {}", outcome.status, outcome.summary);
                if !outcome.unified_diff.is_empty() {
                    println!("\n{}", outcome.unified_diff);
                }
            }
            if outcome.is_error {
                anyhow::bail!("{}", outcome.summary);
            }
            Ok(())
        }
        Commands::Rename {
            symbol,
            new_name,
            file,
            dry_run,
            force,
            json,
        } => {
            use chronos_core::ast::refactor::{RefactorAction, RefactorSymbolRequest};
            let mut session = chronos_core::session::WorkspaceSession::open(workspace.clone(), false)?;
            session.ensure_indexed(None, 400)?;
            let outcome = session
                .refactor_symbol(RefactorSymbolRequest {
                    symbol,
                    action: RefactorAction::Rename,
                    new_name: Some(new_name),
                    file,
                    dry_run,
                    force,
                })
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!("[{}] {}", outcome.status, outcome.summary);
                if !outcome.candidate_sites.is_empty() {
                    println!("\nCandidate sites requiring review:");
                    for c in &outcome.candidate_sites {
                        println!("  #{}: {}:{} [{}] {}", c.index, c.file, c.line, c.kind, c.reason);
                    }
                }
                if !outcome.unified_diff.is_empty() {
                    println!("\n{}", outcome.unified_diff);
                }
            }
            if outcome.is_error {
                anyhow::bail!("{}", outcome.summary);
            }
            Ok(())
        }
        Commands::Delete {
            symbol,
            file,
            dry_run,
            force,
            json,
        } => {
            use chronos_core::ast::refactor::{RefactorAction, RefactorSymbolRequest};
            let mut session = chronos_core::session::WorkspaceSession::open(workspace.clone(), false)?;
            session.ensure_indexed(None, 400)?;
            let outcome = session
                .refactor_symbol(RefactorSymbolRequest {
                    symbol,
                    action: RefactorAction::SafeDelete,
                    new_name: None,
                    file,
                    dry_run,
                    force,
                })
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!("[{}] {}", outcome.status, outcome.summary);
                if !outcome.unified_diff.is_empty() {
                    println!("\n{}", outcome.unified_diff);
                }
            }
            if outcome.is_error {
                anyhow::bail!("{}", outcome.summary);
            }
            Ok(())
        }
        Commands::Eval { script, readonly, json } => {
            use chronos_core::script::ScriptProfile;
            let mut session = chronos_core::session::WorkspaceSession::open(workspace.clone(), readonly)?;
            session.ensure_indexed(None, 400)?;
            let profile = if readonly {
                ScriptProfile::ReadOnly
            } else {
                ScriptProfile::FullCli
            };
            let outcome = session.eval_script(profile, &script)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                for l in &outcome.printed_logs {
                    println!("{l}");
                }
                println!("{}", serde_json::to_string_pretty(&outcome.result)?);
            }
            Ok(())
        }
        Commands::Repl { readonly } => {
            use chronos_core::script::ScriptProfile;
            use std::io::{BufRead, Write};
            let mut session = chronos_core::session::WorkspaceSession::open(workspace.clone(), readonly)?;
            let stats = session.ensure_indexed(None, 400)?;
            let profile = if readonly {
                ScriptProfile::ReadOnly
            } else {
                ScriptProfile::FullCli
            };
            println!(
                "CHRONOS 5D Rhai REPL v{} ({:?}, {} nodes, {} edges). Type .help or .exit.",
                env!("CARGO_PKG_VERSION"),
                profile,
                stats.total_nodes,
                stats.total_edges
            );
            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            loop {
                print!("chronos> ");
                let _ = stdout.flush();
                let mut line = String::new();
                if stdin.lock().read_line(&mut line)? == 0 {
                    break;
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == ".exit" || trimmed == "quit" || trimmed == "exit" {
                    break;
                }
                if trimmed == ".help" {
                    println!("ReadOnly fns: explore(q[,n]), search(q), search_kind(k,ld), blast_radius(s), hotspots(n), arch([pref]), snippet(s), cypher(q)");
                    if profile == ScriptProfile::FullCli {
                        println!("Mutation fns: edit_symbol(sym, op, code, file[, dry, force]), refactor_symbol(sym, act, new, file[, dry, force])");
                    }
                    continue;
                }
                match session.eval_script(profile, trimmed) {
                    Ok(out) => {
                        for l in &out.printed_logs {
                            println!("{l}");
                        }
                        println!("{}", serde_json::to_string_pretty(&out.result)?);
                    }
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            Ok(())
        }
        Commands::Hook { event, client } => {
            // Zero-latency (<20ms), fail-open lifecycle hook handler
            let ev = event.trim().to_ascii_lowercase();
            let cl = client.trim().to_ascii_lowercase();
            let msg = format!(
                "CHRONOS-AGENT v{} 5D Code Intelligence active (14 MCP tools). Use chronos_explore for 1-call context and chronos_blast_radius before editing shared hubs.",
                env!("CARGO_PKG_VERSION")
            );
            if ev == "session-start" || ev == "sessionstart" {
                if cl.contains("claude") {
                    let payload = serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "SessionStart",
                            "additionalContext": msg
                        }
                    });
                    println!("{}", payload);
                } else {
                    // Antigravity SessionStart injectSteps format
                    let payload = serde_json::json!({
                        "injectSteps": [
                            {
                                "ephemeralMessage": msg
                            }
                        ]
                    });
                    println!("{}", payload);
                }
            } else {
                // PostToolUse / post-edit fail-open empty JSON object
                println!("{{}}");
            }
            Ok(())
        }
    }
}

fn build_cli_graph(workspace: &std::path::Path, explicit_files: &[PathBuf]) -> Result<chronos_core::graph::schema::ChronosGraph> {
    let mut session = chronos_core::session::WorkspaceSession::open(workspace.to_path_buf(), false)?;
    session.ensure_indexed_files(explicit_files, 300)?;
    Ok(session.graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parses_v18_subcommands() {
        let edit = Cli::try_parse_from([
            "chronos",
            "edit",
            "compute",
            "--file",
            "src/lib.rs",
            "--op",
            "replace_body",
            "--code",
            "fn compute() {}",
            "--dry-run",
        ])
        .unwrap();
        assert!(matches!(edit.command, Commands::Edit { dry_run: true, .. }));

        let rename = Cli::try_parse_from(["chronos", "rename", "old_fn", "new_fn", "--force"]).unwrap();
        assert!(matches!(rename.command, Commands::Rename { force: true, .. }));

        let delete = Cli::try_parse_from(["chronos", "delete", "dead_fn", "--dry-run"]).unwrap();
        assert!(matches!(delete.command, Commands::Delete { dry_run: true, .. }));

        let eval = Cli::try_parse_from(["chronos", "eval", "arch().total_nodes", "--readonly", "--json"]).unwrap();
        assert!(matches!(eval.command, Commands::Eval { readonly: true, json: true, .. }));

        let repl = Cli::try_parse_from(["chronos", "repl", "--readonly"]).unwrap();
        assert!(matches!(repl.command, Commands::Repl { readonly: true }));

        let explore = Cli::try_parse_from([
            "chronos",
            "explore",
            "Calc/*",
            "--no-body",
            "--depth",
            "2",
            "--blast-radius",
        ])
        .unwrap();
        assert!(matches!(
            explore.command,
            Commands::Explore {
                no_body: true,
                depth: 2,
                blast_radius: true,
                ..
            }
        ));
    }
}
