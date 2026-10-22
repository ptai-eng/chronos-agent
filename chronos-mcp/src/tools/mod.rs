//! # Tools module — all 11 CHRONOS MCP tools

pub mod analyze_file;
pub mod blast_radius;
pub mod contract_status;
pub mod generate_agents_md;
pub mod get_architecture;
pub mod get_snippet;
pub mod hotspots;
pub mod hydrate_telemetry;
pub mod mutation_gate;
pub mod query_graph;
pub mod search_graph;

use crate::protocol::McpTool;
use serde_json::json;

/// Return all tool definitions for tools/list
pub fn all_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "chronos_analyze_file",
            description: "Parse a source file (.ts, .tsx, .js, .py, .rs, .go, .c, .cpp, .java) \
                          with CHRONOS's polyglot AST engine and insert all symbols, call edges, \
                          cross-language HTTP routes, test edges, and loop/complexity metrics into the 4D Knowledge Graph.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute or workspace-relative path to the source file to analyze"
                    }
                },
                "required": ["file_path"]
            }),
        },
        McpTool {
            name: "chronos_query_hotspots",
            description: "Query the 4D Knowledge Graph for the top N performance bottlenecks \
                          and hot-path functions (using live telemetry or PageRank + O(N^2) loop fallback).",
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
                          PageRank centrality, Git temporal co-change partners, confidence interval, \
                          coverage completeness, and residual unknowns.",
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
                    },
                    "detail": {
                        "type": "string",
                        "enum": ["summary", "complete"],
                        "description": "Output verbosity: 'summary' (compact 30s glance) or 'complete' (writes full HTML dashboard)",
                        "default": "summary"
                    }
                },
                "required": ["symbol_name"]
            }),
        },
        McpTool {
            name: "chronos_mutation_gate",
            description: "Run targeted mutation testing on a specific file using CHRONOS's \
                          Mutation Arena. Generates mutants, selects covering tests via 4D graph, \
                          runs them in parallel sandboxes, and returns the mutation score.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the source file to mutation-test"
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
                          Returns any pending contract violation alerts detected since the last call.",
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
        McpTool {
            name: "chronos_query_graph",
            description: "Execute a Read-Only Cypher query directly against the embedded KuzuDB 4D Knowledge Graph. \
                          Every ChronosNode has properties: id, name, kind, file, start_line, end_line, cognitive_load, \
                          cyclomatic, fan_in, fan_out, param_count, sloc, loop_count, loop_depth, transitive_loop_depth, \
                          linear_scan_in_loop, alloc_in_loop, is_recursive, is_hydrated, exec_time_p50, exec_time_p99, \
                          invocation_count, cpu_percent, is_hot_path, is_bottleneck. Relationship tables: CALLS, CONTAINS, TESTS, IMPORTS, CO_CHANGES_WITH.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Read-only Cypher query (e.g. 'MATCH (f:ChronosNode) WHERE f.transitive_loop_depth >= 2 RETURN f.name, f.file, f.transitive_loop_depth')"
                    },
                    "max_rows": {
                        "type": "integer",
                        "description": "Maximum rows to return (default: 500, ceiling: 100000)",
                        "default": 500
                    }
                },
                "required": ["query"]
            }),
        },
        McpTool {
            name: "chronos_search_graph",
            description: "Search the 4D Knowledge Graph using BM25 ranked identifier tokenization (splits camelCase/snake_case), \
                          regex pattern matching, degree filtering, and nested-loop complexity filtering.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural-language or keyword query ranked by BM25 + structural boost + PageRank"
                    },
                    "name_pattern": {
                        "type": "string",
                        "description": "Regex pattern to match symbol name or qualified_name"
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Substring or regex filter on file path"
                    },
                    "kind": {
                        "type": "string",
                        "description": "Filter by node kind: Function | Class | ApiRoute | TestCase"
                    },
                    "min_degree": {
                        "type": "integer",
                        "description": "Minimum total degree (fan_in + fan_out)"
                    },
                    "min_loop_depth": {
                        "type": "integer",
                        "description": "Minimum interprocedural nested loop depth (transitive_loop_depth)"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 50
                    },
                    "offset": {
                        "type": "integer",
                        "default": 0
                    }
                },
                "required": []
            }),
        },
        McpTool {
            name: "chronos_get_snippet",
            description: "Read the verbatim source code (start_line..=end_line) of a function/class from disk \
                          along with its callers, callees, covering tests, PageRank, and AST complexity metrics.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "symbol_name": {
                        "type": "string",
                        "description": "Function, method, or class name to retrieve"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path filter to disambiguate same-name symbols"
                    }
                },
                "required": ["symbol_name"]
            }),
        },
        McpTool {
            name: "chronos_get_architecture",
            description: "Get a high-level architecture overview: community clusters (Label Propagation + Cohesion score), \
                          language breakdown, entry points, top PageRank hubs, and hidden O(N^2) algorithmic hotspots.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional directory prefix to scope architecture analysis"
                    },
                    "auto_index": {
                        "type": "boolean",
                        "description": "Automatically scan and incrementally index workspace files (default: true)",
                        "default": true
                    }
                },
                "required": []
            }),
        },
        McpTool {
            name: "chronos_hydrate_telemetry",
            description: "Hydrate the 4th Dimension (runtime telemetry) of the ChronosGraph from an offline profiler artifact \
                          (V8 .cpuprofile JSON, Speedscope/py-spy JSON, Folded Stack Flamegraph text, or OpenTelemetry OTLP trace JSON).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "profile_path": {
                        "type": "string",
                        "description": "Path to the offline profile file (.cpuprofile, .speedscope.json, .folded, .otlp.json)"
                    },
                    "profile_content": {
                        "type": "string",
                        "description": "Optional inline profile content string (alternative to profile_path)"
                    }
                },
                "required": []
            }),
        },
        McpTool {
            name: "chronos_generate_agents_md",
            description: "Synthesize a live, data-driven AGENTS.md specification from the 5D Knowledge Graph \
                          (Community Clusters, PageRank Hubs, O(N^2) Loop Hotspots, Cross-Boundary Routes, and Git Co-Changes) \
                          tailored for OpenAI Codex CLI, ChatGPT Codex Cloud, and autonomous coding agents.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_name": {
                        "type": "string",
                        "description": "Optional project title override for the AGENTS.md header"
                    },
                    "write_to_disk": {
                        "type": "boolean",
                        "description": "If true, writes the synthesized AGENTS.md directly into the workspace (default: false)",
                        "default": false
                    },
                    "output_path": {
                        "type": "string",
                        "description": "Relative workspace path when write_to_disk is true (default: 'AGENTS.md')",
                        "default": "AGENTS.md"
                    },
                    "auto_index": {
                        "type": "boolean",
                        "description": "Automatically scan and incrementally index workspace files first (default: true)",
                        "default": true
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
pub use generate_agents_md::handle_generate_agents_md;
pub use get_architecture::handle_get_architecture;
pub use get_snippet::handle_get_snippet;
pub use hotspots::handle_query_hotspots;
pub use hydrate_telemetry::handle_hydrate_telemetry;
pub use mutation_gate::handle_mutation_gate;
pub use query_graph::handle_query_graph;
pub use search_graph::handle_search_graph;
