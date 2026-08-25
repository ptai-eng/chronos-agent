//! # CHRONOS MCP Server — Entry Point

use anyhow::Result;
use clap::Parser;
use chronos_mcp::{server::run_server, state::new_shared_state};
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(
    name = "chronos-mcp",
    version = env!("CARGO_PKG_VERSION"),
    about = "CHRONOS-AGENT MCP Server — 4D Knowledge Graph for AI coding agents",
    long_about = r#"CHRONOS MCP Server exposes 5 tools over the Model Context Protocol:

  chronos_analyze_file    — Parse TypeScript and insert into 4D graph
  chronos_query_hotspots  — Find performance bottlenecks
  chronos_blast_radius    — Analyze change impact
  chronos_mutation_gate   — Run mutation testing gate (80%/90%)
  chronos_contract_status — Drain Pre-Tool Execution Gate alerts

Compatible with: Antigravity, Cursor, Codex CLI, OpenCode, Claude Desktop,
                 Cline, Windsurf, Continue, Zed"#
)]
struct Args {
    /// Workspace root directory to analyze
    #[arg(short, long, default_value = ".")]
    workspace: PathBuf,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "warn")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Log to STDERR (stdout is reserved for MCP JSON-RPC)
    let log_level = match args.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info"  => Level::INFO,
        "warn"  => Level::WARN,
        _       => Level::ERROR,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_writer(std::io::stderr) // ← CRITICAL: logs to stderr, not stdout
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Resolve workspace root
    let workspace_root = args.workspace.canonicalize()
        .unwrap_or_else(|_| args.workspace.clone());

    // Initialize shared state
    let state = new_shared_state(workspace_root.clone());

    tracing::info!("CHRONOS MCP Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Workspace: {}", workspace_root.display());

    // Start the MCP server loop
    run_server(state).await
}
