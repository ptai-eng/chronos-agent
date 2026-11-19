//! # Tools module — all 14 CHRONOS MCP tools
//!
//! Tool schemas are defined once in `chronos_core::registry::ToolRegistry`
//! and surfaced here via `crate::registry::McpToolRegistry`.

pub mod analyze_file;
pub mod blast_radius;
pub mod contract_status;
pub mod edit_symbol;
pub mod explore;
pub mod generate_agents_md;
pub mod get_architecture;
pub mod get_snippet;
pub mod hotspots;
pub mod hydrate_telemetry;
pub mod mutation_gate;
pub mod query_graph;
pub mod refactor_symbol;
pub mod search_graph;

use crate::protocol::McpTool;

/// Return all canonical tool definitions for `tools/list`.
pub fn all_tools() -> Vec<McpTool> {
    crate::registry::McpToolRegistry::list_tools(false)
}

pub use analyze_file::handle_analyze_file;
pub use blast_radius::handle_blast_radius;
pub use contract_status::handle_contract_status;
pub use edit_symbol::handle_edit_symbol;
pub use explore::handle_explore;
pub use generate_agents_md::handle_generate_agents_md;
pub use get_architecture::handle_get_architecture;
pub use get_snippet::handle_get_snippet;
pub use hotspots::handle_query_hotspots;
pub use hydrate_telemetry::handle_hydrate_telemetry;
pub use mutation_gate::handle_mutation_gate;
pub use query_graph::handle_query_graph;
pub use refactor_symbol::handle_refactor_symbol;
pub use search_graph::handle_search_graph;
