//! # CHRONOS MCP Server — Universal AI Agent Interface
//!
//! Implements the Model Context Protocol (MCP) spec v2024-11-05.
//! Transport: JSON-RPC 2.0 over stdin/stdout.
//!
//! Compatible with: Antigravity, Cursor, Codex CLI, OpenCode,
//! Claude Desktop, Cline, Windsurf, Continue, Zed.
//!
//! ## Tools exposed
//! - `chronos_analyze_file`    → Parse + AST graph insert
//! - `chronos_query_hotspots`  → Top bottlenecks from 4D graph
//! - `chronos_blast_radius`    → Impact analysis for a function
//! - `chronos_mutation_gate`   → Mutation score check (80%/90% gate)
//! - `chronos_contract_status` → Drain Pre-Tool Execution Gate alerts

pub mod protocol;
pub mod server;
pub mod state;
pub mod tools;
pub mod watcher;
