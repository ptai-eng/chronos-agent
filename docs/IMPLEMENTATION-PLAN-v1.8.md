# CHRONOS-AGENT v1.8.0 — Technical Implementation Plan (Hardened Symbolic Editing, Refactoring & Rhai 5D Script Engine)

> **Target Release:** `v1.8.0` (Builds on the 6 deepened modules shipped in `v1.7.0`)  
> **Follow-Up Milestone (`v1.8.1`):** On-Demand Hybrid `LspSemanticAdapter` & `chronos lsp-hydrate` (deferred from `v1.8.0` to keep `v1.8.0` zero-daemon, deterministic, and fast)  
> **Companion Vision Doc:** [`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./ROADMAP-v1.8-SERENA-EXPANSION.md)

---

## 1. Finalized Architectural Decisions & Security Hardening (`Q1`–`Q5` + Audit Fixes)

### 1.1 `Q1` — 14 Canonical MCP Tools + Dedicated CLI Action Subcommands
- Extend `ToolRegistry` (`chronos-core::registry` & `chronos-mcp::registry`) from **12 to 14 MCP tools**:
  - **Tool #13: `chronos_edit_symbol`** (`operation: "replace_body" | "insert_before" | "insert_after"`, dispatch alias `chronos_replace_symbol_body`). Marked `readonly: false` (hidden from `tools/list` and blocked at dispatch when `--readonly` is active).
  - **Tool #14: `chronos_refactor_symbol`** (`action: "rename" | "safe_delete" | "preview_usages"`, dispatch aliases `chronos_rename_symbol`, `chronos_safe_delete`). Marked `readonly: false` (except `action: "preview_usages"` or `dry_run: true`, which are allowed in `--readonly` mode).
- Add dedicated CLI subcommands in `chronos-cli`:
  - `chronos edit <symbol> --file <path> --op <replace_body|insert_before|insert_after> (--code <str> | --code-file <path>) [--dry-run] [--force]`
  - `chronos rename <symbol> <new_name> [--file <path>] [--dry-run] [--force]`
  - `chronos delete <symbol> [--file <path>] [--dry-run] [--force]`
  - `chronos repl` & `chronos eval "<rhai_script>" [--json]`

### 1.2 `Q2` — Fresh-Parse Symbol Editing, Leading Trivia Span & 2-Stage Gate (`chronos_edit_symbol`)
- **Fix #1 (Never Write Using Cached Graph Byte Offsets):**
  - Before any edit or delete, `WorkspaceSession` **always reads the current file bytes from disk, re-parses the fresh Tree-sitter `Tree` + `ParsedModule` on the spot**, and resolves the target symbol from that fresh parse. Cached graph byte offsets are used only for read-only search/ranking, never for file mutation.
- **Fix #2 (Explicit Leading Trivia + Declaration Span & Indentation Normalization):**
  - `locate_symbol_with_trivia(tree, source, target_node)` walks preceding sibling AST nodes (`attribute_item`, `decorator`, `comment` / doc-comments immediately attached to the declaration with no blank-line gap) to compute two exact byte spans:
    - `declaration_span` (`start_byte..end_byte` of the function/class/method node itself)
    - `full_symbol_span` (`trivia_start_byte..end_byte` including attached doc-comments & attributes/decorators)
  - In `replace_body`: if `new_code` starts with leading doc-comments or attributes/decorators (`#[...]`, `@...`, `///`, `/** */`), replace `full_symbol_span` (preventing duplicate attributes and allowing decorator edits/removal); otherwise replace `declaration_span` while preserving existing leading doc-comments/attributes.
  - Auto-normalize indentation of `new_code` to match the base column indent of the target symbol (critical for Python block indentation) and preserve original line endings (`CRLF` vs `LF`) and UTF-8 BOM.
- **2-Stage Safety Gate & Explicit Block Semantics:**
  1. **Stage 1 (In-Memory Tree-sitter Syntax Gate):** Apply `TextEdit` in memory (`apply_edits_in_memory`), verify `WorkspaceSession::validate_syntax` returns `None` (`0` syntax errors). If invalid, return `isError: Some(true)` with `status: "syntax_error"` and exact `1-indexed (line, col)` without touching disk.
  2. **Stage 2 (Contract & Caller Blast-Radius Gate):** Compare old vs. new `contract_signature` via `ContractEngine`.
     - If signature is unchanged (internal body edit) and `dry_run == false`: write atomically via `AtomicFileWriter` and flush `GraphRepository::sync_file_delta`.
     - If signature changes AND the symbol has downstream callers (`fan_in > 0`) while `force == false` and `dry_run == false`: **do not write to disk**; return `isError: Some(true)` with `status: "blocked_requires_force"`, unified diff preview, downstream callers, and `verification_surfaces` so the LLM never mistakes a blocked edit for a completed write.

### 1.3 `Q3` — Graph-Edge-Verified Multi-File Refactoring & Transactional Rollback (`chronos_refactor_symbol`)
- **Fix #3 (Edge-Verified `rename` + Candidate Sites & Homonym/Unresolved Guard):**
  - Rather than blindly replacing every matching identifier token in connected files (which would corrupt local variables `let foo`, struct fields `self.foo`, or unrelated methods `other.foo()`), `rename` classifies occurrences in the fresh AST of the defining file and files linked by `CALLS`, `IMPORTS`, and `TESTS` into:
    1. **Verified Sites (`verified_edits`):**
       - The symbol's own declaration identifier at `target_node.location`.
       - Call-site identifiers on lines recorded in verified inbound `CALLS` / `TESTS` edges (`ChronosEdge.call_site_line` or inside verified caller function spans where the call expression target is a bare identifier matching `old_name` or `Parent::old_name`).
       - Import/export specifiers (`IMPORTS` edges / `use` / `from ... import` clauses referencing the defining module).
    2. **Ambiguous / Unverified Candidate Sites (`candidate_sites`):**
       - Member-access identifiers (`obj.old_name`) without static receiver type verification, shadowed local bindings, or occurrences when `homonyms > 1` (`resolve_symbols(old_name, None).len() > 1`) or `unresolved_calls > 0` in the affected files.
  - **Gate Rule:** If `candidate_sites` is non-empty, `homonyms > 1`, or `unresolved_calls > 0` in affected files, AND `force == false` (and `dry_run == false`), `rename` refuses to auto-write and returns `isError: Some(true)` with `status: "blocked_ambiguous_rename"`, listing both `verified_edits` and numbered `candidate_sites`.
  - **All-or-Nothing Multi-File Transaction (`AtomicFileWriter`):**
    - Validates Tree-sitter syntax in memory for **100% of touched files** first.
    - Writes via `AtomicFileWriter::commit_transaction` (backups in memory, write temp file in same directory + atomic rename, automatic full rollback of all previously written files if any disk I/O fails, preserving `CRLF`/`LF` and UTF-8 BOM).
- **Hardened `safe_delete`:**
  - Checks active inbound `CALLS`, covering `TESTS`, cross-language `ApiRoute` endpoints, **and** entrypoint/visibility markers (`RootEntrypoint`, `CliCommand`, `main`, `#[test]`, `#[tokio::main]`, exported/`pub` symbols when `coverage.completeness != "complete"`, or Rust trait implementation methods).
  - Blocks deletion with `isError: Some(true)` (`status: "blocked_active_dependents"`) if any dependent or public/entrypoint guard triggers (unless `force: true`).
  - When allowed, deletes `full_symbol_span` (including attached leading doc-comments and attributes/decorators) and normalizes surrounding blank lines.

### 1.4 `Q4` — Defer On-Demand Hybrid LSP (`LspSemanticAdapter`) to `v1.8.1`
- Keep `v1.8.0` 100% zero-daemon and deterministic on top of `AstSemanticAdapter` (`<15ms`).
- Document `LspSemanticAdapter` (`chronos lsp-hydrate`, Windows `.cmd` wrapper resolution, `$/progress` indexing wait, and 60s CLI / 15s MCP timeouts) in [`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./ROADMAP-v1.8-SERENA-EXPANSION.md) as the dedicated **`v1.8.1`** follow-up milestone.

### 1.5 `Q5` — Embedded `rhai` 5D Script Engine & Read-Only MCP `chronos_explore.script`
- **Fix #4 (Strict Read-Only Boundary for MCP `chronos_explore.script`):**
  - Embed the battle-tested Rust [`rhai`](https://crates.io/crates/rhai) scripting engine (`Engine::new_raw()` — zero filesystem, zero shell, zero network, bounded with `set_max_operations(50_000)`, `set_max_call_levels(32)`, `set_max_expr_depths(32, 32)`, `set_max_string_size(256 * 1024)`, `set_max_array_size(4096)`).
  - **Two Capability Profiles:**
    1. **`ScriptProfile::ReadOnly` (Used by MCP `chronos_explore` `script` parameter and `--readonly` CLI):**
       - Registers ONLY read-only 5D graph functions:
         - `explore(query)`, `explore(query, max_symbols)`
         - `search(query)`, `search_kind(kind, min_loop_depth)`
         - `blast_radius(symbol)`
         - `hotspots(top_n)`
         - `arch()`, `arch(path_prefix)`
         - `snippet(symbol)`
         - `cypher(query)`
       - **Never registers `edit_symbol` or `refactor_symbol`**, making prompt-injection write escalation through `chronos_explore` structurally impossible.
    2. **`ScriptProfile::FullCli` (Used by CLI `chronos eval` and `chronos repl` when not `--readonly`):**
       - Registers all read-only functions above PLUS `edit_symbol(symbol, op, new_code, file)` and `refactor_symbol(symbol, action, new_name_or_empty, file)` (both still subject to the 2-stage syntax + blast-radius gate).
- **Upgraded `chronos_explore` (#12 — Keeps Total MCP Tools at 14):**
  - Adds optional parameters:
    - `queries: Option<Vec<String>>` (batch multiple symbol / `Class/*` queries in one round-trip)
    - `include_body: bool` (default `true`; when `false`, returns signature + line/byte span + complexity metrics without source code body)
    - `depth: usize` (default `1`; when querying `ClassName/*` or `ClassName`, expands child methods contained in that class/struct/impl)
    - `include_blast_radius: bool` (default `false`; attaches compact `RiskZone` + direct/transitive caller counts per symbol bundle)
    - `script: Option<String>` (evaluates a `ScriptProfile::ReadOnly` Rhai script and returns its JSON result under `"script_result"`).

---

## 2. Step-by-Step TDD Implementation Order for `v1.8.0`

### Step 0 — Fresh Re-Parse Resolver, `AtomicFileWriter` & `xxHash64` Watcher Deduplication (`chronos-core/src/ast/atomic_writer.rs`, `session.rs`)
- **Implement `AtomicFileWriter`:**
  - Detects and preserves original file line endings (`\r\n` vs `\n`) and UTF-8 BOM.
  - Supports multi-file atomic transactions (`commit_transaction(&[(PathBuf, String)])`): writes temp file in target directory, renames over target, and rolls back all prior files from in-memory snapshot if any step fails.
- **Implement `WorkspaceSession::parse_fresh_file(&self, file_path: &Path)`:**
  - Validates workspace boundary, reads current disk content, parses fresh Tree-sitter `Tree` + `ParsedModule`, and locates target symbols with attached leading trivia (`full_symbol_span` vs `declaration_span`).
- **Implement `xxHash64` short-circuit in `WorkspaceSession::index_file`:**
  - Store `last_indexed_hashes: HashMap<PathBuf, String>` on `WorkspaceSession` so when `FileWatcher` fires 300ms after `chronos_edit_symbol` / `chronos_refactor_symbol` already indexed the file, `index_file` skips redundant re-parsing in `<0.2ms`.
- **Unit Tests:**
  - `test_atomic_writer_preserves_crlf_and_bom_and_rolls_back_on_error`
  - `test_fresh_parse_locates_leading_attributes_and_doc_comments_across_rust_python_ts`
  - `test_index_file_skips_unchanged_xxhash64`

### Step 1 — Symbol-Boundary AST Editing Engine (`chronos-core/src/ast/symbol_editor.rs`)
- Implement `WorkspaceSession::edit_symbol(req: EditSymbolRequest) -> Result<EditSymbolOutcome>`:
  - Operations: `ReplaceBody`, `InsertBefore`, `InsertAfter`.
  - Leading trivia deduplication + target column indentation alignment.
  - Stage 1 In-Memory Tree-sitter syntax verification (`validate_syntax`).
  - Stage 2 `ContractEngine` signature comparison + `fan_in > 0` Blast-Radius Gate (`status: "blocked_requires_force"` with unified diff when signature changes on a symbol with active callers and `force == false`).
  - Atomic write via `AtomicFileWriter` + immediate `WorkspaceSession::index_file` delta sync.
- **Unit Tests:**
  - `test_replace_body_internal_change_writes_and_syncs_graph`
  - `test_replace_body_deduplicates_rust_attributes_and_python_decorators`
  - `test_replace_body_rejects_syntax_error_without_touching_disk`
  - `test_replace_body_blocks_breaking_signature_when_callers_exist_until_forced`
  - `test_insert_after_and_insert_before_preserve_indentation`

### Step 2 — Graph-Edge-Verified Multi-File Refactoring Engine (`chronos-core/src/ast/refactor.rs`)
- Implement `WorkspaceSession::refactor_symbol(req: RefactorSymbolRequest) -> Result<RefactorSymbolOutcome>`:
  - Actions: `PreviewUsages`, `Rename`, `SafeDelete`.
  - AST identifier collector that skips comments and string literals and distinguishes `verified_edits` (declaration + verified `CALLS`/`TESTS` call sites + `IMPORTS` specifiers) from `candidate_sites` (unverified member accesses / shadowed locals).
  - Homonym (`resolve_symbols(name).len() > 1`), `unresolved_calls > 0`, and `candidate_sites` gate (`status: "blocked_ambiguous_rename"` unless `force: true`).
  - Multi-file in-memory Tree-sitter syntax verification across all touched files before `AtomicFileWriter::commit_transaction`.
  - Hardened `safe_delete` checking `CALLS`, `TESTS`, `ApiRoute`, and public/entrypoint/test/trait-impl guards, removing `full_symbol_span`.
- **Unit Tests:**
  - `test_rename_updates_declaration_and_verified_callers_across_files_ignoring_comments_and_strings`
  - `test_rename_blocks_on_homonym_or_ambiguous_member_access_without_force`
  - `test_rename_rolls_back_all_files_if_second_file_fails_syntax_check`
  - `test_safe_delete_blocks_when_callers_or_tests_or_entrypoint_exist`
  - `test_safe_delete_removes_symbol_and_leading_attributes_when_unreferenced`

### Step 3 — Progressive `name_path` (`Class/*`, `include_body: false`) & Multi-Query `chronos_explore` (`chronos-core/src/graph/query.rs`)
- Upgrade `ChronosGraph::resolve_symbols` to support wildcard `Parent/*` (returning child methods/functions connected via `CONTAINS` or matching `Parent::*` / `Parent.*` qualified names).
- Upgrade `GraphQueryEngine::explore_graph_with_options`:
  - Supports `queries: &[String]`, `include_body: bool`, `depth: usize`, and `include_blast_radius: bool`.
- **Unit Tests:**
  - `test_wildcard_name_path_outline_without_body`
  - `test_multi_query_explore_with_inline_blast_radius`

### Step 4 — Embedded `rhai` Sandboxed 5D Script Engine (`chronos-core/src/script/mod.rs`)
- Add `rhai = { version = "1", features = ["serde"] }` to workspace dependencies.
- Implement `ScriptEngine` with `ScriptProfile::ReadOnly` (for MCP `chronos_explore.script` and `--readonly` CLI) and `ScriptProfile::FullCli` (for CLI `chronos eval` / `chronos repl`):
  - Enforces strict execution budgets (`max_operations = 50_000`, `max_call_levels = 32`, `max_expr_depths = (32, 32)`).
  - Exposes ergonomic functions (`explore`, `search`, `blast_radius`, `hotspots`, `arch`, `snippet`, `cypher`, plus CLI-only `edit_symbol`, `refactor_symbol`).
- **Unit Tests:**
  - `test_rhai_readonly_profile_executes_5d_queries_and_blocks_edit_functions`
  - `test_rhai_operation_limit_aborts_infinite_loops_cleanly`
  - `test_rhai_cli_profile_chains_search_and_edit_with_gate`

### Step 5 — 14-Tool Registry Expansion, CLI Subcommands, Hardcoded-Count Audit & `v1.8.0` Verification
- Update `chronos-core/src/registry.rs`:
  - Update `chronos_explore` parameters (`queries`, `include_body`, `depth`, `include_blast_radius`, `script`).
  - Add `chronos_edit_symbol` (#13, `readonly: false`) and `chronos_refactor_symbol` (#14, `readonly: false`).
  - Update schema drift test assertions from `12` → `14` tools (and verify `--readonly` `McpToolRegistry::list_tools(true)` filters out write tools cleanly).
- Update `chronos-mcp/src/registry.rs` & `chronos-mcp/src/tools/`:
  - Add `tools/edit_symbol.rs` and `tools/refactor_symbol.rs` + dispatch aliases (`chronos_replace_symbol_body`, `chronos_rename_symbol`, `chronos_safe_delete`).
  - Wire `chronos_explore` new options (`queries`, `include_body`, `depth`, `include_blast_radius`, `script` with `ScriptProfile::ReadOnly`).
- Update `chronos-cli`:
  - Add `Edit`, `Rename`, `Delete`, `Eval`, and `Repl` subcommands in `chronos-cli/src/main.rs`.
  - Update all hardcoded `"12"` tool counts in `chronos-core/src/graph/agents_md.rs`, `chronos-cli/src/main.rs`, `chronos-cli/src/commands/mcp_info.rs`, `Cargo.toml`, and `chronos-vscode/package.json` to **`14`**.
- Bump workspace version to `1.8.0`, refresh client context files (`chronos init --client all --with-hooks`), and run full `cargo test --workspace -j 4`.
