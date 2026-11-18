# AGENTS.md — chronos-agent

> Auto-synthesized by **CHRONOS-AGENT** (5D Living Code Intelligence Engine) for **OpenAI Codex CLI**, **ChatGPT Codex**, **Google Antigravity**, **OpenCode**, and MCP-compliant autonomous coding agents.

## 1. Build, Test & Verification Commands

- **Rust Build:** `cargo build --workspace -j 4`
- **Rust Test Suite:** `cargo test --workspace -j 4`
- **1-Call Subagent Context (`chronos explore`):** `chronos explore "<query>" --workspace .`
- **5D Git Diff Blast-Radius Gate (CI / Pre-PR):** `chronos diff --staged --json --ci`
- **Refresh Multi-Agent Context Files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`):** `chronos init --client all --with-hooks`

## 2. OpenAI Codex & MCP Tool Protocol (Mandatory Before Committing)

When connected to `chronos-mcp` (`codex mcp add chronos-agent -- chronos-mcp --workspace .` or `claude mcp add chronos-agent -- chronos-mcp --workspace .`), follow this 4-step verification loop:

1. **Locate & Read Without Context Bloat:** Call `chronos_explore` (1-call BM25 + verbatim snippets + verification surfaces + 4-angle audit), `chronos_get_architecture`, `chronos_search_graph`, or `chronos_get_snippet` instead of dumping entire files.
2. **Pre-Edit Blast-Radius Check:** Before modifying any function listed in **High-Centrality Hubs** below, call `chronos_blast_radius` (`symbol_name`, `contract_breaking`) to inspect transitive callers, `verification_surfaces`, `structured_findings` (`failure_scenario`), 95% confidence bounds, and Git co-change partners.
3. **Contract Broker & 4-Angle Review Gate:** After editing public signatures or routes, call `chronos_contract_status` and `chronos_analyze_file` (`detail: "complete"`) to drain breaking-change alerts and 4-angle review diagnostics (`Reuse`, `Efficiency`, `Simplification`, `Altitude`).
4. **Super-Mutant Verification Gate:** Before finalizing a PR on critical logic, run `chronos_mutation_gate` (`file_path`) to verify unit tests kill synthetic code mutants.

## 3. 5D Knowledge Graph Snapshot

- **Indexed Symbols:** `667` nodes (`429` functions, `117` tests) | **Directed Edges:** `1337` (`2526` resolved calls, `3435` unresolved dynamic calls)
- **Language Distribution:** Rust (639), TypeScript (25), C (3)
- **Runtime Telemetry Hydrated:** `Cold-Start Static + PageRank Mode` (`0` hot paths, `0` bottlenecks)

## 4. Architectural Modules (Label-Propagation Communities)

| Module Cluster | Symbols | Internal Cohesion | Primary Symbols |
| :--- | :---: | :---: | :--- |
| `graph/schema` | 60 | 58% | `lines`, `new`, `default`, `invalidate_caches` |
| `graph/query` | 47 | 62% | `new`, `from_node`, `new_with_options`, `new` |
| `ast/ts_parser` | 31 | 82% | `new`, `parse_source`, `node_text`, `extract_symbols` |
| `graph/agents_md` | 28 | 57% | `new`, `synthesize_target_context`, `resolve_project_name`, `from_str_loose` |
| `session` | 28 | 43% | `strip_windows_verbatim`, `normalize_workspace_root`, `is_supported`, `open_in_memory` |
| `contract/signature` | 27 | 80% | `from_source`, `is_contract_breaking`, `diff`, `from_parsed_module` |
| `registry` | 27 | 71% | `specs`, `to_mcp_schema`, `new_shared_state_with_options`, `list_tools` |
| `tests/persistent_test` | 24 | 52% | `NodeId`, `open`, `load_into_graph`, `open_repository_if_allowed` |
| `ast/polyglot_parser` | 23 | 75% | `node_text`, `from_ts_node`, `new`, `extract_routes_and_http_calls_from_source` |
| `tests/integration_test` | 23 | 32% | `new`, `compute_line_text_edit`, `all_nodes`, `nodes_by_kind` |
| `blast_radius/impact_matrix` | 21 | 69% | `new`, `from_graph`, `analyze`, `build_coverage_and_unknowns` |
| `mutation/arena` | 21 | 76% | `new`, `with_strict`, `suggest_assertion_for_mutant`, `generate_ai_feedback_prompt` |

## 5. High-Centrality Hub Symbols (Blast-Radius Sensitive)

Modifying these structural hubs affects the largest transitive caller trees. **Always run `chronos_blast_radius` before changing their signatures or invariants:**

| Symbol | Kind | File | PageRank | Fan-In | Fan-Out | Transitive Loop Depth |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: |
| `new` | `Function` | `chronos-core/src/mutation/operators.rs:90` | `0.0412` | 59 | 1 | 10 |
| `compute_line_text_edit` | `Function` | `chronos-core/src/mutation/operators.rs:133` | `0.0358` | 1 | 2 | 10 |
| `lines` | `Function` | `chronos-core/src/graph/schema.rs:112` | `0.0334` | 31 | 0 | 0 |
| `new` | `Function` | `chronos-core/src/graph/schema.rs:347` | `0.0193` | 17 | 2 | 0 |
| `new` | `Function` | `chronos-core/src/contract/signature.rs:124` | `0.0127` | 23 | 0 | 0 |
| `NodeId` | `Class` | `chronos-core/src/graph/schema.rs:24` | `0.0113` | 5 | 0 | 0 |
| `default` | `Function` | `chronos-core/src/graph/schema.rs:1252` | `0.0112` | 3 | 1 | 0 |
| `find` | `Function` | `chronos-core/src/registry.rs:714` | `0.0109` | 24 | 0 | 0 |
| `open` | `Function` | `chronos-core/src/session.rs:76` | `0.0077` | 16 | 4 | 10 |
| `new` | `Function` | `chronos-core/src/graph/query.rs:1251` | `0.0069` | 14 | 0 | 0 |

## 6. Algorithmic Complexity & Loop Hotspots

Functions with interprocedural nested loops (`transitive_loop_depth >= 2`), linear scans inside loops ($O(N^2)$ risk), or recursion:

- **`extract_decorated`** (`chronos-core/src/ast/py_parser.rs:228`) — `transitive_loop_depth=10`, `linear_scan_in_loop=11`, `alloc_in_loop=4`, `recursive=true`
- **`generate_mutants`** (`chronos-core/src/mutation/operators.rs:163`) — `transitive_loop_depth=10`, `linear_scan_in_loop=5`, `alloc_in_loop=5`, `recursive=false`
- **`sync_workspace_incremental`** (`chronos-core/src/graph/persistent.rs:611`) — `transitive_loop_depth=10`, `linear_scan_in_loop=1`, `alloc_in_loop=1`, `recursive=false`
- **`run_profiling_session`** (`chronos-core/src/telemetry/cdp_client.rs:222`) — `transitive_loop_depth=10`, `linear_scan_in_loop=1`, `alloc_in_loop=0`, `recursive=false`
- **`main`** (`chronos-cli/src/main.rs:230`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`run`** (`chronos-core/src/mutation/arena.rs:170`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=3`, `recursive=false`
- **`dispatch`** (`chronos-mcp/src/registry.rs:38`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=false`
- **`run`** (`chronos-cli/src/commands/graph.rs:10`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=1`, `recursive=false`
- **`parse_unified_diff`** (`chronos-cli/src/commands/diff_audit.rs:106`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=12`, `recursive=false`
- **`extract_symbols`** (`chronos-core/src/ast/ts_parser.rs:137`) — `transitive_loop_depth=10`, `linear_scan_in_loop=0`, `alloc_in_loop=0`, `recursive=true`

## 7. Cross-Boundary API Routes (Frontend <-> Backend Stitched)

- **`ROUTE /users`** (`chronos-core/src/contract/signature.rs:479`) — no stitched callers
- **`ROUTE /users/:id`** (`chronos-core/src/ast/ts_parser.rs:662`) — no stitched callers
- **`ROUTE /users`** (`chronos-core/src/ast/ts_parser.rs:667`) — no stitched callers
- **`GET mcpBinaryPath`** (`chronos-vscode/src/extension.ts:15`) — no stitched callers
- **`ROUTE /users/:id`** (`chronos-core/src/contract/signature.rs:478`) — no stitched callers
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
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `assets/chronos_radar_dashboard.svg` (**100%** co-change confidence)
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `assets/chronos_architecture.svg` (**100%** co-change confidence)
- `assets/chronos_benchmark_radar.svg` $\rightarrow$ `README.md` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `assets/chronos_benchmark_radar.svg` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `assets/chronos_architecture.svg` (**100%** co-change confidence)
- `assets/chronos_radar_dashboard.svg` $\rightarrow$ `README.md` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.lock` (**100%** co-change confidence)
- `chronos-core/Cargo.toml` $\rightarrow$ `Cargo.toml` (**100%** co-change confidence)
- `chronos-core/src/ast/dispatcher.rs` $\rightarrow$ `chronos-core/src/ast/mod.rs` (**100%** co-change confidence)

