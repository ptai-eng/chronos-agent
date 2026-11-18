# CHRONOS-AGENT v1.7.0 — Technical Implementation Plan (Core Module Deepening & Seam Hardening)

> **Release Scope:** `v1.7.0` — Deepen 6 Core Modules, Eliminate 4 Verified Bugs, and Establish Clean Seams for `v1.8.0`  
> **Companion Next-Phase Roadmap:** [`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./ROADMAP-v1.8-SERENA-EXPANSION.md) (`v1.8.0` Serena-Inspired Symbolic Action & Refactoring Engine)  
> **Architecture Audit Reference:** `architecture-review-20261118-204119.html` (`/improve-codebase-architecture`)

---

## 1. Executive Summary & Release Boundary (`v1.7.0` vs. `v1.8.0`)

Following the architectural audit (`/improve-codebase-architecture`) and design interview (`/grill-me` Q1–Q5), the post-`v1.6.0` evolution is cleanly separated into two focused releases:

- **`v1.7.0` (This Plan — Core Architecture Deepening & Bug-Fix Release):**
  - Resolves **4 verified correctness bugs** hiding inside shallow modules.
  - Consolidates duplicated logic across `chronos-core`, `chronos-mcp`, and `chronos-cli` into **6 deep modules** (`WorkspaceSession`, `ToolRegistry`, `GraphRepository`, `CodeIndex`, `ContractEngine`, `MutationArena`).
  - Lays down the exact structural seams (`SourceLocation` byte spans, `TextEdit`, `SemanticResolver`, `ToolRegistry` response budgeting, `WorkspaceSession` transactional file operations) required by `v1.8.0`.
  - Preserves **100% MCP JSON-RPC 2.0 & CLI wire compatibility** across all **12 existing MCP tools** and **8 AI client integrations**.
- **`v1.8.0` ([`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./ROADMAP-v1.8-SERENA-EXPANSION.md) — Serena-Inspired Expansion):**
  - Builds `chronos_edit_symbol` (#13), `chronos_refactor_symbol` (#14), `name_path` (`Class/method`) addressing, multi-query batching on `chronos_explore`, and the opt-in `LspAdapter` directly on top of the `v1.7.0` seams.

---

## 2. Settled Architectural Decisions (`Q1`–`Q5`)

1. **Two-Milestone Separation (`Q1`):** Land all 6 module deepenings and 4 bug fixes in `v1.7.0` first with a 100% green test suite before adding the Serena-inspired symbolic editing/refactoring features in `v1.8.0`.
2. **Wire Compatibility vs. Internal Rust Refactoring (`Q2`):** Lock 100% backward compatibility on external MCP JSON-RPC 2.0 inputs/outputs (`chronos-vscode`, Codex, Antigravity, Claude Code, OpenCode) and CLI flags, while freely reshaping internal `chronos-core` Rust structs and **auto-rebuilding `.chronos/graph.kuzu`** whenever a schema version change is detected.
3. **Write-Safety & Read-Only Guard (`Q3`):** Prepare `WorkspaceSession` and `ToolRegistry` with canonical workspace path validation, in-memory Tree-sitter syntax verification, Blast-Radius `RiskZone` gating, and a `--readonly` server flag ready for `v1.8.0` write tools.
4. **Zero-Dependency Default Resolver (`Q4`):** Keep `<15ms` `AstAdapter` (Tree-sitter) as the default `SemanticResolver` adapter with zero external daemon dependencies, exposing a clean seam for `v1.8.0`'s opt-in `LspAdapter`.
5. **Declarative Budgeting & Multi-Query (`Q5`):** Build `max_answer_chars` truncation (default `24,000` chars) directly into `ToolRegistry` middleware rather than embedding an arbitrary scripting VM.

---

## 3. Four Verified Bugs Fixed in `v1.7.0`

| Bug ID | Location Observed | Root Cause | `v1.7.0` Fix |
| :--- | :--- | :--- | :--- |
| **BUG-01** | `chronos-mcp/src/tools/mutation_gate.rs:35-85` & `chronos-core/src/mutation/arena.rs:177-191` | `handle_mutation_gate` computes `threshold = 0.90` when `strict = true` and prints `Threshold: 90%`, but never passes `strict` or `threshold` into `MutationArena::run`, which hardcodes `MUTATION_SCORE_THRESHOLD` (`0.80`) and skips `run_super_mutant_parallel`. | Pass `threshold` (or `strict: bool`) into `MutationArena`, evaluate `passed_quality_gate = score >= self.threshold`, and route eligible languages through `run_super_mutant_parallel`. |
| **BUG-02** | `chronos-mcp/src/watcher.rs:100-107` | `FileWatcher` calls `st.graph.ingest_parsed_module(&parsed)` on file save without calling `st.graph.remove_nodes_for_file(&path)` or updating `ChronosFileHash` in KuzuDB, leaving ghost symbols from deleted/renamed functions in memory. | Route both `watcher.rs` and `analyze_file.rs` through `WorkspaceSession::index_file(&path)` which atomically evicts stale file nodes, ingests the new `ParsedModule`, updates `xxHash64`, and syncs `GraphRepository` + `ContractEngine`. |
| **BUG-03** | `chronos-core/src/graph/persistent.rs:190-211, 328-430` | `upsert_node`'s `ON MATCH SET` clause updates `end_line` but omits `n.start_line = $start_line`, `n.param_count = $params`, and `n.sloc = $sloc`. Additionally, `load_into_graph` drops `qualified_name`, `contract_signature`, and `unresolved_call_sites`. | Include all mutable columns (`start_line`, `start_byte`, `end_byte`, `param_count`, `sloc`, `qualified_name`, `contract_signature`) in `GraphRepository` schema + `ON MATCH SET` + `load_into_graph`, with automatic `.chronos/graph.kuzu` rebuild on schema version mismatch. |
| **BUG-04** | `chronos-mcp/src/tools/mod.rs:156-295` vs `chronos-cli/src/commands/mcp_info.rs:318-390` | Hand-maintained duplicate tool schemas drifted: `chronos_search_graph` in `mcp_info.rs` is missing `offset`, and `chronos_generate_agents_md` marks `output_path` as required without nullable type in strict mode. | Delete duplicate schema tables; generate both MCP `tools/list` and CLI `chronos schema --format <openai\|responses\|anthropic\|gemini\|mcp>` from one canonical `ToolRegistry`. |

---

## 4. Six Core Module Deepenings (Detailed Design)

### Module 1 — `WorkspaceSession` (`chronos-core/src/session.rs`)
- **Replaces / Absorbs:** Raw `ChronosState` manipulation in `chronos-mcp/src/state.rs`, duplicated directory walkers in `chronos-cli/src/main.rs` (`build_cli_graph`, `collect_cli_files`), `chronos-mcp/src/tools/explore.rs`, `get_architecture.rs`, `generate_agents_md.rs`, and `watcher.rs`.
- **Dependency Category:** `local-substitutable` (tested against real `tempfile::TempDir` workspaces).
- **Interface:**
  ```rust
  pub struct WorkspaceSession {
      workspace_root: PathBuf,
      graph: ChronosGraph,
      repo: Option<GraphRepository>,
      contracts: ContractEngine,
      dispatcher: LanguageDispatcher,
      readonly: bool,
  }

  impl WorkspaceSession {
      pub fn open(workspace_root: PathBuf, readonly: bool) -> Result<Self>;
      pub fn ensure_indexed(&mut self, scope: Option<&Path>, max_files: usize) -> Result<IndexStats>;
      pub fn index_file(&mut self, file_path: &Path) -> Result<FileIndexReport>;
      pub fn validate_workspace_path(&self, candidate: &Path) -> Result<PathBuf>;
      pub fn graph(&self) -> &ChronosGraph;
      pub fn contracts(&self) -> &ContractEngine;
      pub fn repo(&self) -> Option<&GraphRepository>;
  }
  ```
- **Invariants Enforced:**
  - Single canonical ignore list (`.git`, `node_modules`, `target`, `dist`, `build`, `.venv`, `venv`, `__pycache__`, `.chronos`, `out`).
  - Every single-file re-index (`index_file`) always evicts stale nodes for that file, re-parses via `LanguageDispatcher`, resolves pending calls/routes, diffs contracts in `ContractEngine`, updates `xxHash64`, and flushes file delta to `GraphRepository`.

---

### Module 2 — `ToolRegistry` (`chronos-mcp/src/registry.rs`)
- **Replaces / Absorbs:** `chronos-mcp/src/tools/mod.rs` (`all_tools`), `chronos-mcp/src/server.rs` (`tools/call` match table), and `chronos-cli/src/commands/mcp_info.rs` (`raw_tool_specs`).
- **Dependency Category:** `in-process`.
- **Interface:**
  ```rust
  pub struct ToolSpec {
      pub name: &'static str,
      pub description: &'static str,
      pub readonly: bool,
      pub properties: Value,
      pub required: &'static [&'static str],
  }

  pub struct ToolRegistry;

  impl ToolRegistry {
      pub fn specs(include_write_tools: bool) -> Vec<ToolSpec>;
      pub fn export_schema(format: SchemaFormat) -> Result<String>;
      pub async fn dispatch(
          state: SharedState,
          tool_name: &str,
          arguments: Option<Value>,
      ) -> Result<CallToolResult, JsonRpcError>;
  }
  ```
- **Middleware Capabilities:**
  - Automatically drains pending `ContractEngine` alerts and attaches them to `_meta.contract_alerts` when present.
  - Enforces universal `max_answer_chars` (default `24_000`) truncation on text/JSON payloads so no tool call ever overflows an agent's context window.
  - Derives both relaxed MCP `inputSchema` and OpenAI `strict: true` schemas (automatically converting optional fields to `["<type>", "null"]` and listing all keys in `required` for `openai` / `responses` formats).

---

### Module 3 — `GraphRepository` (`chronos-core/src/graph/persistent.rs`)
- **Replaces / Absorbs:** `PersistentGraph` (`chronos-core/src/graph/persistent.rs`) and deletes the unused stub `GraphStore` (`chronos-core/src/graph/store.rs`).
- **Dependency Category:** `local-substitutable`.
- **Interface:**
  ```rust
  pub const GRAPH_SCHEMA_VERSION: u32 = 2; // v1.7.0 lossless schema

  pub struct GraphRepository {
      db: Database, // private — no longer leaked as `pub db`
  }

  impl GraphRepository {
      pub fn open(db_path: &Path) -> Result<Self>;
      pub fn sync_file_delta(&self, graph: &ChronosGraph, file_path: &Path, file_hash: &str) -> Result<()>;
      pub fn sync_full(&self, graph: &ChronosGraph) -> Result<()>;
      pub fn restore_into_graph(&self, graph: &mut ChronosGraph) -> Result<usize>;
      pub fn execute_readonly_cypher(&self, cypher: &str, max_rows: usize) -> Result<CypherQueryResult>;
  }
  ```
- **Fixes & Upgrades:**
  - Stores a `ChronosMeta` table (or version marker file `.chronos/schema_version`); if an existing `.chronos/graph.kuzu` is from `v1.6.0` (schema v1), automatically removes and recreates the KuzuDB directory cleanly on startup.
  - Persists and restores `qualified_name`, `contract_signature`, `start_byte`, `end_byte`, `unresolved_call_sites`, and updates `start_line`, `param_count`, `sloc` inside `ON MATCH SET`.
  - Adds `sync_file_delta` so single-file edits only update nodes/edges touching that file instead of rewriting all $O(N + E)$ rows.

---

### Module 4 — `CodeIndex` & `SemanticResolver` Seam (`chronos-core/src/graph/query.rs` & `schema.rs`)
- **Replaces / Absorbs:** Scattered `all_nodes()` linear scans in `blast_radius.rs`, `glance.rs`, `graph.rs`, repeated `compute_pagerank` calls inside `explore_graph` and `analyze_diff`, and duplicated `CoverageReport` builders.
- **Dependency Category:** `in-process` + `ports & adapters` (`AstAdapter` in `v1.7.0`; `LspAdapter` plugs into the same seam in `v1.8.0`).
- **Schema Upgrade (`SourceLocation`):**
  ```rust
  #[derive(Debug, Clone, Default, Serialize, Deserialize)]
  pub struct SourceLocation {
      pub file: PathBuf,
      pub start_line: usize,
      pub start_col: usize,
      pub end_line: usize,
      pub end_col: usize,
      #[serde(default)]
      pub start_byte: usize,
      #[serde(default)]
      pub end_byte: usize,
  }
  ```
  *(Populated directly from `tree_sitter::Node::start_byte()` and `end_byte()` across all 7 language parsers: `ts_parser.rs`, `py_parser.rs`, `rs_parser.rs`, `polyglot_parser.rs`).*
- **Unified Resolution & Cached Centrality:**
  - `ChronosGraph::resolve_symbol(query_or_name_path: &str, file_filter: Option<&str>) -> Vec<&ChronosNode>`: supports exact name, `qualified_name`, suffix `::name` / `.name`, and `Class/method` (`name_path`) matching in one place.
  - Memoizes PageRank vector on `ChronosGraph` (invalidated automatically when nodes/edges mutate) so `explore_graph` and `analyze_diff` compute PageRank **once** instead of $N$ times.
  - Unifies `CoverageReport::from_graph(graph)` as a single method shared by `ImpactAnalyzer` and `chronos_explore`.

---

### Module 5 — Polyglot `ContractEngine` (`chronos-core/src/contract/engine.rs`)
- **Replaces / Absorbs:** Duplicate TypeScript-only text scanners in `chronos-core/src/ast/diff.rs` and `chronos-core/src/contract/signature.rs`, plus deletes redundant `chronos-core/src/contract/watcher.rs`.
- **Dependency Category:** `in-process` + `local-substitutable`.
- **Interface:**
  ```rust
  pub struct ContractEngine { ... }

  impl ContractEngine {
      pub fn new() -> Self;
      pub fn update_module(&self, parsed: &ParsedModule, graph: &ChronosGraph) -> Vec<BreakingChangeAlert>;
      pub fn drain_alerts(&self) -> Vec<BreakingChangeAlert>;
      pub fn peek_alerts(&self) -> Vec<BreakingChangeAlert>;
  }
  ```
- **Upgrades:**
  - Extracts function/method/class/route signatures directly from `ParsedModule` (`ChronosNode.contract_signature` and `ApiRoute` nodes) across **all 7 supported languages** (Rust, Python, TypeScript/JS, Go, C, C++, Java) instead of regex-scanning `.ts` lines.
  - Cross-references callers via `ChronosGraph::callers_of` to list affected downstream callers in every `BreakingChangeAlert`.

---

### Module 6 — Deep `MutationArena` with `TextEdit` & `TestExecutor` Seam (`chronos-core/src/mutation/`)
- **Replaces / Absorbs:** Leaky coupling across `arena.rs`, `operators.rs`, `runner.rs`, `watchdog.rs`, and `mutation_gate.rs`.
- **Dependency Category:** `ports & adapters`.
- **Shared `TextEdit` Value Type (`chronos-core/src/ast/edit.rs`):**
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct TextEdit {
      pub file: PathBuf,
      pub start_byte: usize,
      pub end_byte: usize,
      pub start_line: usize,
      pub end_line: usize,
      pub replacement: String,
  }
  ```
  *(Used by `MutationArena` in `v1.7.0` and reused directly by `chronos_edit_symbol` / `chronos_refactor_symbol` in `v1.8.0`).*
- **Interface & Fixes:**
  - `MutationArena::with_strict(mut self, strict: bool) -> Self` sets threshold to `0.90` (`MUTATION_SCORE_THRESHOLD_STRICT`) when `strict == true` and `0.80` otherwise (fixing **BUG-01** in both `chronos_mutation_gate` MCP tool and `chronos audit --strict` CLI).
  - Introduces `pub trait TestExecutor: Send + Sync` with `ProcessTestExecutor` (production OS sandbox runner) and `MockTestExecutor` (deterministic in-memory test double for fast unit tests).
  - Routes single-compile eligible languages through `run_super_mutant_parallel` (`CHRONOS_MUTANT_ID`) with automatic fallback to `run_mutants_parallel`.

---

## 5. Step-by-Step TDD Execution Order for `v1.7.0`

1. **Step 1 — `SourceLocation` Byte Spans & `TextEdit` Primitive (`chronos-core/src/graph/schema.rs`, `ast/*.rs`):**
   - Add `start_byte` and `end_byte` to `SourceLocation` across `ts_parser.rs`, `py_parser.rs`, `rs_parser.rs`, and `polyglot_parser.rs`.
   - Add `TextEdit::apply_in_memory(source: &str) -> Result<String>` with UTF-8 boundary checks and unit tests.
2. **Step 2 — Lossless `GraphRepository` & Schema v2 Auto-Rebuild (`chronos-core/src/graph/persistent.rs`):**
   - Fix `ON MATCH SET` (`start_line`, `param_count`, `sloc`, `start_byte`, `end_byte`, `qualified_name`, `contract_signature`), fix `load_into_graph`, add schema version check, add `sync_file_delta`, and delete `chronos-core/src/graph/store.rs`.
   - Write regression test in `persistent_test.rs` asserting shifted `start_line`, `qualified_name`, and `contract_signature` survive KuzuDB restart.
3. **Step 3 — `CodeIndex` Caching & Unified Symbol/Coverage Resolution (`chronos-core/src/graph/query.rs`, `impact_matrix.rs`):**
   - Cache PageRank on `ChronosGraph`, add `ChronosGraph::resolve_symbol` (supporting `name_path` `Struct/method`), unify `CoverageReport::from_graph`, and define `SemanticResolver` trait (`AstAdapter`).
4. **Step 4 — Polyglot `ContractEngine` (`chronos-core/src/contract/`):**
   - Unify `ast/diff.rs` and `contract/signature.rs` into AST-driven 7-language contract diffing; remove duplicate `contract/watcher.rs`.
5. **Step 5 — Deep `MutationArena` (`chronos-core/src/mutation/` & `chronos-mcp/src/tools/mutation_gate.rs`):**
   - Wire `with_strict(strict_mode)` into `MutationArena`, add `TestExecutor` trait + in-memory test adapter, and add unit test proving an 85% score passes default mode (80%) and **fails** `strict: true` mode (90%).
6. **Step 6 — `WorkspaceSession` & `ToolRegistry` (`chronos-core/src/session.rs`, `chronos-mcp/`, `chronos-cli/`):**
   - Replace scattered directory walkers and `watcher.rs` raw ingestion with `WorkspaceSession`.
   - Unify `tools/mod.rs`, `server.rs`, and `mcp_info.rs` into `ToolRegistry` with `max_answer_chars` budgeting and zero schema drift test.
   - Bump workspace version to `1.7.0` and verify all `cargo test --workspace -j 4` tests pass.
