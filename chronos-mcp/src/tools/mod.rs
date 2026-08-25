//! # Tools module — all 5 CHRONOS MCP tools

pub mod analyze_file;
pub mod blast_radius;
pub mod contract_status;
pub mod hotspots;
pub mod mutation_gate;

use crate::protocol::McpTool;
use serde_json::json;

/// Return all tool definitions for tools/list
pub fn all_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "chronos_analyze_file",
            description: "Parse a TypeScript/JavaScript file with CHRONOS's AST engine \
                          and insert all symbols (functions, classes, routes, tests) into \
                          the 4D Knowledge Graph. Call this first before querying hotspots \
                          or blast-radius for any file.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path to the .ts/.js file to analyze"
                    }
                },
                "required": ["file_path"]
            }),
        },
        McpTool {
            name: "chronos_query_hotspots",
            description: "Query the 4D Knowledge Graph for the top N performance bottlenecks \
                          and hot-path functions detected during runtime profiling. \
                          Returns ranked list with p95 latency, CPU%, and complexity scores.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "top_n": {
                        "type": "integer",
                        "description": "Number of hotspots to return (default: 10)",
                        "default": 10
                    },
                    "filter": {
                        "type": "string",
                        "enum": ["bottlenecks", "hot_paths", "high_complexity", "all"],
                        "description": "Filter type (default: all)",
                        "default": "all"
                    }
                },
                "required": []
            }),
        },
        McpTool {
            name: "chronos_blast_radius",
            description: "Analyze the blast-radius (change impact) of modifying a specific \
                          function or symbol. Returns a risk zone (SAFE/MEDIUM/HIGH/CRITICAL), \
                          affected callers count, hot-path exposure, and a 30-second Markdown \
                          summary. Generates full HTML dashboard for HIGH/CRITICAL risk.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "symbol_name": {
                        "type": "string",
                        "description": "The function/class name to analyze (e.g. 'getUser', 'OrderService')"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "File containing the symbol (helps disambiguate same-name symbols)"
                    },
                    "contract_breaking": {
                        "type": "boolean",
                        "description": "Set true if this change modifies the public API signature",
                        "default": false
                    }
                },
                "required": ["symbol_name"]
            }),
        },
        McpTool {
            name: "chronos_mutation_gate",
            description: "Run targeted mutation testing on a specific file using CHRONOS's \
                          Mutation Arena. Generates mutants, selects covering tests via 4D graph, \
                          runs them with timeout isolation, and returns the mutation score. \
                          Gate: 80% dev mode, 90% with strict=true (release gate).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the TypeScript file to mutation-test"
                    },
                    "test_command": {
                        "type": "string",
                        "description": "Test runner command (default: 'npx jest --passWithNoTests')",
                        "default": "npx jest --passWithNoTests"
                    },
                    "strict": {
                        "type": "boolean",
                        "description": "Use 90% threshold (release gate). Default false = 80% dev gate.",
                        "default": false
                    }
                },
                "required": ["file_path"]
            }),
        },
        McpTool {
            name: "chronos_contract_status",
            description: "Drain the Semantic Contract Broker's Pre-Tool Execution Gate. \
                          Returns any pending contract violation alerts detected since the \
                          last call. ALWAYS call this at the start of every coding session \
                          to receive any queued breaking-change warnings.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "watch_dir": {
                        "type": "string",
                        "description": "Directory to start watching for contract changes (optional)"
                    }
                },
                "required": []
            }),
        },
    ]
}

pub use analyze_file::handle_analyze_file;
pub use blast_radius::handle_blast_radius;
pub use contract_status::handle_contract_status;
pub use hotspots::handle_query_hotspots;
pub use mutation_gate::handle_mutation_gate;
