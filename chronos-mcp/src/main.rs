//! # CHRONOS MCP Server — Entry Point

use anyhow::Result;
use clap::Parser;
use chronos_core::session::normalize_workspace_root;
use chronos_mcp::{
    server::run_server,
    state::new_shared_state_with_options,
    watcher::FileWatcher,
};
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(
    name = "chronos-mcp",
    version = env!("CARGO_PKG_VERSION"),
    about = "CHRONOS-AGENT MCP Server — 5D Code Graph, AST Symbol Editing & Refactoring",
    long_about = r#"CHRONOS MCP Server exposes 14 tools over the Model Context Protocol:

  Exploration & Search : chronos_explore, chronos_search_graph, chronos_get_snippet,
                         chronos_get_architecture, chronos_query_graph
  Analysis & Risk      : chronos_analyze_file, chronos_blast_radius, chronos_query_hotspots,
                         chronos_mutation_gate, chronos_contract_status, chronos_hydrate_telemetry
  Scaffolding & Edits  : chronos_generate_agents_md, chronos_edit_symbol, chronos_refactor_symbol

Changed source files are re-parsed automatically (disable with --no-watch).
Pass --readonly to hide write tools and block source mutations.

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

    /// Disable real-time file watcher (useful for CI environments)
    #[arg(long)]
    no_watch: bool,

    /// Run server in strict read-only mode (disables disk/workspace write operations)
    #[arg(long)]
    readonly: bool,
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
    let workspace_root = normalize_workspace_root(&args.workspace);

    // Initialize shared state backed by WorkspaceSession
    let state = new_shared_state_with_options(workspace_root.clone(), args.readonly);

    tracing::info!("CHRONOS MCP Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Workspace: {}", workspace_root.display());

    // Start real-time file watcher (unless disabled)
    let _watcher = if !args.no_watch {
        match FileWatcher::start(workspace_root.clone(), state.clone()) {
            Ok(w) => {
                tracing::info!("📡 Real-time file watcher active on {}", workspace_root.display());
                Some(w)
            }
            Err(e) => {
                tracing::warn!("File watcher failed to start: {} (continuing without auto-sync)", e);
                None
            }
        }
    } else {
        tracing::info!("File watcher disabled (--no-watch)");
        None
    };

    // Start the MCP server loop
    run_server(state).await
}
