# AGENTS.md — CHRONOS-AGENT

> Auto-synthesized by **CHRONOS-AGENT v1.5.0** (5D Living Code Intelligence Engine) for **OpenAI Codex CLI**, **ChatGPT Codex**, and MCP-compliant autonomous coding agents.

## 1. Build, Test & Verification Commands

- **Rust Build:** `cargo build --workspace -j 4`
- **Rust Test Suite:** `cargo test --workspace -j 4`
- **5D Git Diff Blast-Radius Gate (CI / Pre-PR):** `chronos diff --staged --json --ci`
- **Refresh `AGENTS.md` from 5D Graph:** `chronos agents-md --write`

## 2. OpenAI Codex & MCP Tool Protocol (Mandatory Before Committing)

When connected to `chronos-mcp` (`codex mcp add chronos-agent -- chronos-mcp --workspace .`), follow this 4-step verification loop:

1. **Locate & Read Without Context Bloat:** Call `chronos_get_architecture`, `chronos_search_graph` (BM25 + regex + loop depth), or `chronos_get_snippet` instead of dumping entire files.
2. **Pre-Edit Blast-Radius Check:** Before modifying any function listed in **High-Centrality Hubs** below, call `chronos_blast_radius` (`symbol_name`, `contract_breaking`) to inspect transitive callers, 95% confidence bounds, and Git co-change partners.
3. **Contract Broker Gate:** After editing public signatures or routes, call `chronos_contract_status` to drain breaking-change alerts across callers.
4. **Super-Mutant Verification Gate:** Before finalizing a PR on critical logic, run `chronos_mutation_gate` (`file_path`) to verify unit tests kill synthetic code mutants.

## 3. 5D Knowledge Graph Snapshot

- **Indexed Symbols:** `510` nodes (`319` functions, `99` tests) | **Directed Edges:** `1164` (`1450` resolved calls, `2914` unresolved dynamic calls)
- **Language Distribution:** Rust (510)
- **Runtime Telemetry Hydrated:** `Cold-Start Static + PageRank Mode` (`0` hot paths, `0` bottlenecks)

## 4. Architectural Modules (Label-Propagation Communities)

| Module Cluster | Symbols | Internal Cohesion | Primary Symbols |
| :--- | :---: | :---: | :--- |
| `graph/schema` | 65 | 53% | `new`, `default`, `add_node`, `status_report` |
| `ast/ts_parser` | 30 | 87% | `new`, `node_text`, `parse_source`, `extract_symbols` |
| `graph/query` | 29 | 49% | `new`, `from_node`, `default`, `tokenize_identifier` |
| `graph/persistent` | 27 | 34% | `new`, `load_into_graph`, `validate_readonly_cypher`, `ensure_schema` |
| `blast_radius/impact_matrix` | 25 | 40% | `new`, `new`, `analyze`, `build_coverage_and_unknowns` |
| `ast/polyglot_parser` | 22 | 86% | `node_text`, `new`, `extract_routes_and_http_calls_from_source`, `populate_ast_complexity_metrics` |
| `mutation/operators` | 21 | 65% | `new`, `generate_mutants`, `description`, `replace_first_in_line` |
| `contract/signature` | 19 | 85% | `from_source`, `extract_exported_functions`, `extract_api_routes`, `diff` |
| `tests/integration_test` | 19 | 31% | `new`, `nodes_by_kind`, `parse_file`, `ingest_parsed_module` |
| `telemetry/cdp_client` | 18 | 62% | `default`, `rand_factor`, `is_available`, `backend_name` |
| `ast/diff` | 17 | 93% | `compute_module_diff`, `is_contract_breaking`, `new`, `add_change` |
| `ast/py_parser` | 17 | 65% | `ok`, `new`, `parse_str`, `make_node` |

## 5. High-Centrality Hub Symbols (Blast-Radius Sensitive)

Modifying these structural hubs affects the largest transitive caller trees. **Always run `chronos_blast_radius` before changing their signatures or invariants:**

| Symbol | Kind | File | PageRank | Fan-In | Fan-Out | Transitive Loop Depth |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: |
| `new` | `Function` | `chronos-core/src/graph/schema.rs:295` | `0.0223` | 16 | 2 | 0 |
| `new` | `Function` | `chronos-core/src/contract/broker.rs:49` | `0.0180` | 20 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/telemetry/adapter.rs:116` | `0.0172` | 32 | 0 | 0 |
| `NodeId` | `Class` | `chronos-core/src/graph/schema.rs:24` | `0.0144` | 6 | 0 | 0 |
| `default` | `Function` | `chronos-core/src/graph/schema.rs:1104` | `0.0125` | 3 | 1 | 0 |
| `ok` | `Function` | `chronos-mcp/src/protocol.rs:40` | `0.0123` | 16 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/mutation/operators.rs:86` | `0.0097` | 10 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/ast/ts_parser.rs:86` | `0.0083` | 14 | 0 | 0 |
| `node_text` | `Function` | `chronos-core/src/ast/ts_parser.rs:612` | `0.0074` | 8 | 0 | 0 |
| `node_text` | `Function` | `chronos-core/src/ast/polyglot_parser.rs:686` | `0.0073` | 7 | 0 | 0 |

## 6. Algorithmic Complexity & Loop Hotspots

Functions with interprocedural nested loops (`transitive_loop_depth >= 2`), linear scans inside loops ($O(N^2)$ risk), or recursion:

- **`extract_decorated`** (`chronos-core/src/ast/py_parser.rs:234`) — `transitive_loop_depth=10`, `linear_scan_in_loop=11`, `alloc_in_loop=4`, `recursive=true`
- **`sync_workspace_incremental`** (`chronos-core/src/graph/persistent.rs:482`) — `transitive_loop_depth=10`, `linear_scan_in_loop=1`, `alloc_in_loop=1`, `recursive=false`
- **`main`** (`chronos-cli/src/main.rs:187`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`run`** (`chronos-cli/src/commands/graph.rs:10`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=1`, `recursive=false`
- **`handle_request`** (`chronos-mcp/src/server.rs:63`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`start`** (`chronos-mcp/src/watcher.rs:33`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=3`, `recursive=false`
- **`parse_unified_diff`** (`chronos-cli/src/commands/diff_audit.rs:106`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=12`, `recursive=false`
- **`extract_symbols`** (`chronos-core/src/ast/ts_parser.rs:137`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=true`
- **`handle_analyze_file`** (`chronos-mcp/src/tools/analyze_file.rs:13`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`run`** (`chronos-cli/src/commands/audit.rs:18`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=true`

## 7. Cross-Boundary API Routes (Frontend <-> Backend Stitched)

- **`ROUTE /users/:id`** (`chronos-core/src/ast/ts_parser.rs:670`) — called by `is_test_module`
- **`ROUTE /users`** (`chronos-core/src/ast/ts_parser.rs:675`) — no stitched callers
- **`ROUTE /users/:id`** (`chronos-core/src/contract/signature.rs:338`) — no stitched callers
- **`ROUTE /users`** (`chronos-core/src/contract/signature.rs:339`) — no stitched callers

## 8. Git Temporal Co-Change Coupling (`CO_CHANGES_WITH`)

Files that historically change together in Git commits. When modifying the left file, verify whether the right partner also requires updates:

- `Cargo.lock` $\rightarrow$ `Cargo.toml` (**100%** co-change confidence)
- `Cargo.toml` $\rightarrow$ `Cargo.lock` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.lock` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.toml` (**100%** co-change confidence)
- `chronos-core/src/ast/dispatcher.rs` $\rightarrow$ `chronos-core/src/ast/rs_parser.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/dispatcher.rs` $\rightarrow$ `chronos-core/src/ast/py_parser.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/dispatcher.rs` $\rightarrow$ `chronos-core/src/ast/mod.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/mod.rs` $\rightarrow$ `chronos-core/src/ast/py_parser.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/mod.rs` $\rightarrow$ `chronos-core/src/ast/rs_parser.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/mod.rs` $\rightarrow$ `chronos-core/src/ast/dispatcher.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/py_parser.rs` $\rightarrow$ `chronos-core/src/ast/mod.rs` (**100%** co-change confidence)
- `chronos-core/src/ast/py_parser.rs` $\rightarrow$ `chronos-core/src/ast/rs_parser.rs` (**100%** co-change confidence)

