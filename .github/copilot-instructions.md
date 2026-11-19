# GitHub Copilot Coding Agent Instructions — chronos-agent

> Auto-synthesized by **CHRONOS-AGENT** for **GitHub Copilot Coding Agent** and **VS Code MCP (`.vscode/mcp.json`)**.

## 1. Build, Test & Verification Commands

- **Rust Build:** `cargo build --workspace -j 4`
- **Rust Test Suite:** `cargo test --workspace -j 4`
- **1-Call Subagent Context (`chronos explore`):** `chronos explore "<query>" --workspace .`
- **5D Git Diff Blast-Radius Gate (CI / Pre-PR):** `chronos diff --staged --json --ci`
- **Refresh Multi-Agent Context Files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`):** `chronos init --client all --with-hooks`

## 2. Copilot Agent & VS Code MCP Protocol

1. **Locate & Read Without Context Bloat:** Call `chronos_explore` (1-call BM25 + verbatim snippets + verification surfaces + 4-angle audit), `chronos_get_architecture`, `chronos_search_graph`, or `chronos_get_snippet` instead of dumping entire files.
2. **Pre-Edit Blast-Radius Check:** Before modifying any function listed in **High-Centrality Hubs** below, call `chronos_blast_radius` (`symbol_name`, `contract_breaking`) to inspect transitive callers, `verification_surfaces`, `structured_findings` (`failure_scenario`), 95% confidence bounds, and Git co-change partners.
3. **Contract Broker & 4-Angle Review Gate:** After editing public signatures or routes, call `chronos_contract_status` and `chronos_analyze_file` (`detail: "complete"`) to drain breaking-change alerts and 4-angle review diagnostics (`Reuse`, `Efficiency`, `Simplification`, `Altitude`).
4. **Super-Mutant Verification Gate:** Before finalizing a PR on critical logic, run `chronos_mutation_gate` (`file_path`) to verify unit tests kill synthetic code mutants.

## 3. 5D Knowledge Graph Snapshot

- **Indexed Symbols:** `770` nodes (`505` functions, `131` tests) | **Directed Edges:** `1928` (`2474` resolved calls, `4525` unresolved dynamic calls)
- **Language Distribution:** Rust (742), TypeScript (25), C (3)
- **Runtime Telemetry Hydrated:** `Cold-Start Static + PageRank Mode` (`0` hot paths, `0` bottlenecks)

## 4. Architectural Modules (Label-Propagation Communities)

| Module Cluster | Symbols | Internal Cohesion | Primary Symbols |
| :--- | :---: | :---: | :--- |
| `session` | 65 | 53% | `new`, `compute_line_text_edit`, `strip_windows_verbatim`, `open_in_memory` |
| `graph/schema` | 64 | 45% | `lines`, `new`, `default`, `invalidate_caches` |
| `ast/ts_parser` | 52 | 54% | `as_str`, `parse_source`, `new`, `ContentItem` |
| `graph/query` | 50 | 44% | `new`, `from_node`, `all_nodes`, `tokenize_identifier` |
| `registry` | 29 | 64% | `new`, `specs`, `to_mcp_schema`, `list_tools` |
| `ast/edit` | 28 | 64% | `from_ts_node`, `new`, `apply_edits_in_memory`, `byte_to_line_col` |
| `ast/refactor` | 28 | 39% | `as_str`, `ROUTE /users/:id`, `parse`, `is_function_scope_kind` |
| `contract/signature` | 28 | 68% | `from_source`, `is_contract_breaking`, `from_parsed_module`, `diff` |
| `tests/persistent_test` | 25 | 51% | `NodeId`, `open`, `load_into_graph`, `open_repository_if_allowed` |
| `ast/polyglot_parser` | 22 | 72% | `node_text`, `new`, `extract_routes_and_http_calls_from_source`, `parse_source` |
| `mutation/arena` | 22 | 56% | `new`, `generate_ai_feedback_prompt`, `format_for_agent`, `suggest_assertion_for_mutant` |
| `blast_radius/impact_matrix` | 21 | 49% | `new`, `from_graph`, `analyze`, `build_coverage_and_unknowns` |

## 5. High-Centrality Hub Symbols (Blast-Radius Sensitive)

Modifying these structural hubs affects the largest transitive caller trees. **Always run `chronos_blast_radius` before changing their signatures or invariants:**

| Symbol | Kind | File | PageRank | Fan-In | Fan-Out | Transitive Loop Depth |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: |
| `new` | `Function` | `chronos-core/src/mutation/operators.rs:90` | `0.0298` | 34 | 1 | 10 |
| `lines` | `Function` | `chronos-core/src/graph/schema.rs:112` | `0.0272` | 35 | 0 | 0 |
| `compute_line_text_edit` | `Function` | `chronos-core/src/mutation/operators.rs:133` | `0.0260` | 1 | 2 | 10 |
| `new` | `Function` | `chronos-core/src/graph/schema.rs:347` | `0.0178` | 17 | 2 | 0 |
| `as_str` | `Function` | `chronos-core/src/ast/symbol_editor.rs:33` | `0.0139` | 48 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/contract/signature.rs:124` | `0.0106` | 23 | 0 | 0 |
| `NodeId` | `Class` | `chronos-core/src/graph/schema.rs:24` | `0.0106` | 6 | 0 | 0 |
| `default` | `Function` | `chronos-core/src/graph/schema.rs:1305` | `0.0100` | 3 | 1 | 0 |
| `find` | `Function` | `chronos-core/src/registry.rs:901` | `0.0090` | 26 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/telemetry/adapter.rs:116` | `0.0090` | 36 | 0 | 0 |

## 6. Algorithmic Complexity & Loop Hotspots

Functions with interprocedural nested loops (`transitive_loop_depth >= 2`), linear scans inside loops ($O(N^2)$ risk), or recursion:

- **`extract_decorated`** (`chronos-core/src/ast/py_parser.rs:228`) — `transitive_loop_depth=10`, `linear_scan_in_loop=11`, `alloc_in_loop=4`, `recursive=true`
- **`generate_mutants`** (`chronos-core/src/mutation/operators.rs:163`) — `transitive_loop_depth=10`, `linear_scan_in_loop=5`, `alloc_in_loop=5`, `recursive=false`
- **`collect_usage_classification`** (`chronos-core/src/ast/refactor.rs:587`) — `transitive_loop_depth=10`, `linear_scan_in_loop=3`, `alloc_in_loop=12`, `recursive=false`
- **`sync_workspace_incremental`** (`chronos-core/src/graph/persistent.rs:619`) — `transitive_loop_depth=10`, `linear_scan_in_loop=1`, `alloc_in_loop=1`, `recursive=false`
- **`main`** (`chronos-cli/src/main.rs:321`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`execute_rename`** (`chronos-core/src/ast/refactor.rs:167`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=5`, `recursive=false`
- **`dispatch`** (`chronos-mcp/src/registry.rs:38`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`execute_safe_delete`** (`chronos-core/src/ast/refactor.rs:386`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=2`, `recursive=false`
- **`run`** (`chronos-core/src/mutation/arena.rs:170`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=3`, `recursive=false`
- **`register_readonly_functions`** (`chronos-core/src/script/mod.rs:190`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`

## 7. Cross-Boundary API Routes (Frontend <-> Backend Stitched)

- **`ROUTE /users/:id`** (`chronos-core/src/ast/ts_parser.rs:662`) — called by `is_test_module`, `is_call_callee_position`, `is_member_access_target`, `is_other_symbol_declaration_name`, `locate_symbol_with_trivia`
- **`ROUTE /users`** (`chronos-core/src/ast/ts_parser.rs:667`) — no stitched callers
- **`ROUTE /users/:id`** (`chronos-core/src/contract/signature.rs:478`) — no stitched callers
- **`ROUTE /users`** (`chronos-core/src/contract/signature.rs:479`) — no stitched callers
- **`GET mcpBinaryPath`** (`chronos-vscode/src/extension.ts:15`) — no stitched callers
- **`GET /orders`** (`fixtures/ts-express-api/src/orderService.ts:183`) — no stitched callers
- **`GET /orders/:id`** (`fixtures/ts-express-api/src/orderService.ts:192`) — no stitched callers
- **`POST /orders/:id/discount`** (`fixtures/ts-express-api/src/orderService.ts:202`) — no stitched callers
- **`ROUTE /orders`** (`fixtures/ts-express-api/src/orderService.ts:183`) — no stitched callers
- **`ROUTE /orders/:id`** (`fixtures/ts-express-api/src/orderService.ts:192`) — no stitched callers
- **`ROUTE /orders/:id/discount`** (`fixtures/ts-express-api/src/orderService.ts:202`) — no stitched callers

## 8. Git Temporal Co-Change Coupling (`CO_CHANGES_WITH`)

Files that historically change together in Git commits. When modifying the left file, verify whether the right partner also requires updates:

- `Cargo.lock` $\rightarrow$ `Cargo.toml` (**100%** co-change confidence)
- `Cargo.toml` $\rightarrow$ `Cargo.lock` (**100%** co-change confidence)
- `assets/chronos_architecture.svg` $\rightarrow$ `README.md` (**100%** co-change confidence)
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `README.md` (**100%** co-change confidence)
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `assets/chronos_radar_dashboard.svg` (**100%** co-change confidence)
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `assets/chronos_architecture.svg` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `assets/chronos_architecture.svg` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `README.md` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `assets/chronos_benchmark_radar.svg` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.toml` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.lock` (**100%** co-change confidence)
- `chronos-core/src/ast/dispatcher.rs` $\rightarrow$ `chronos-core/src/ast/py_parser.rs` (**100%** co-change confidence)

