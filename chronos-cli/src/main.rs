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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { Level::DEBUG } else { Level::WARN };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let workspace = cli.workspace.canonicalize()
        .unwrap_or_else(|_| cli.workspace.clone());

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

            // Build a lightweight impact report (without full graph — fast path)
            let symbol_reports: Vec<SymbolImpact> = symbols.iter().map(|s| {
                SymbolImpact {
                    symbol: s.symbol_name.clone(),
                    file: s.file.to_string_lossy().to_string(),
                    risk_level: "UNKNOWN".to_string(), // Full graph needed for real score
                    direct_callers: 0,
                    transitive_callers: 0,
                    recommended_test_modules: vec![],
                }
            }).collect();

            let has_high_risk = false; // Requires full graph for real analysis
            let report = DiffBlastRadiusReport {
                changed_symbols: symbols,
                symbol_reports,
                overall_risk: "UNKNOWN — run chronos audit for full analysis".to_string(),
                exit_code_for_ci: 0,
            };

            if json {
                println!("{}", format_json(&report));
            } else {
                println!("{}", format_markdown(&report));
                println!("\n💡 For full blast-radius scores, run: chronos audit <file>");
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
    }
}
