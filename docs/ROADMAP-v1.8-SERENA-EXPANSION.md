# CHRONOS-AGENT v1.8.0 Roadmap — Serena-Inspired Symbolic Action & Refactoring Engine

> **Target Release:** `v1.8.0` (Builds on top of the deepened seams in [`docs/IMPLEMENTATION-PLAN-v1.7.md`](./IMPLEMENTATION-PLAN-v1.7.md))  
> **Primary Sources Inspected:**
> - Installed `serena` (`oraios/serena`) MCP Server schemas & runtime (`initial_instructions`, `activate_project`, `serena_repl` facades: `s.lsp`, `s.edit`, `s.fs`, `s.mem`, `s.shell`, `s.cfg`)
> - Agent Memory record: `mem_mvpcp6mh_f2cd008c8e98`

---

## 1. Strategic Vision for `v1.8.0`

Once `v1.7.0` completes the 6 Core Module Deepenings (`WorkspaceSession`, `ToolRegistry`, `GraphRepository`, `CodeIndex` + `SourceLocation` byte spans, `ContractEngine`, and `MutationArena` + `TextEdit`), `CHRONOS-AGENT v1.8.0` will evolve from a **5D Code Intelligence & Verification Sensor** into a **Blast-Radius-Gated Symbolic Action Engine**.

By refining the lessons from `Serena` (`oraios/serena`) with strict architectural guardrails (avoiding tool bloat, unsafe homonym renames, and heavy mandatory LSP daemons), `v1.8.0` adds **2 new write tools (bringing the total to 14 MCP tools)** plus progressive symbol navigation and optional LSP hydration.

---

## 2. Five Refined Expansion Pillars for `v1.8.0`

### Pillar 1 — Symbol-Boundary AST Editing (`chronos_edit_symbol` — 13th MCP Tool)
- **Built on `v1.7.0` Seams:** `SourceLocation` (`start_byte..end_byte`), `TextEdit`, `WorkspaceSession::index_file`, and `ContractEngine`.
- **Operations:** `"replace_body"` | `"insert_before"` | `"insert_after"`.
- **3-Layer Defense-in-Depth (`Q3`):**
  1. **In-Memory Tree-sitter Syntax Gate:** Applies `TextEdit` in memory and re-parses with `LanguageDispatcher` **before touching disk**. If `tree.root_node().has_error()` is true, rejects the edit immediately with exact line/column syntax diagnostics.
  2. **Pre-Edit 5D Blast-Radius & Contract Gate:** If the target symbol sits in `RiskZone::Critical` or changes its public signature across callers without `force: true`, returns a `dry_run` diff preview + `structured_findings` + `verification_surfaces`.
  3. **Atomic KuzuDB Delta Sync:** Updates `GraphRepository::sync_file_delta` in `<5ms` after writing.

### Pillar 2 — Homonym-Safe Atomic Refactoring (`chronos_refactor_symbol` — 14th MCP Tool)
- **Built on `v1.7.0` Seams:** `CodeIndex::resolve_symbol`, `WorkspaceSession`, and `TextEdit`.
- **Actions:** `"safe_delete"` | `"rename"` | `"preview_usages"`.
- **Why It Beats Naive AST Refactoring:**
  - **`safe_delete`:** Checks static `CALLS`, `TESTS`, cross-language `ApiRoute` bridges (`/{*}`), and Git `CO_CHANGES_WITH` partners. Refuses deletion if any active caller exists unless `force: true`.
  - **`rename` (Homonym Guard):** Checks whether the symbol name has homonyms (`ambiguous_homonyms > 1`) or `unresolved_calls > 0`. When unambiguous, applies atomic multi-file `TextEdit`s with full rollback if any file fails Tree-sitter syntax verification; when ambiguous, automatically enforces `dry_run: true` with numbered occurrence previews so the agent can select exact occurrence IDs.

### Pillar 3 — Hierarchical `name_path` & Progressive Overview (`include_body: false`, `depth`)
- **Built on `v1.7.0` Seams:** `CodeIndex::resolve_symbol`.
- **Capabilities:**
  - Supports `StructName/method_name`, `ClassName/__init__`, and wildcard `ClassName/*` (`depth: 1`, `include_body: false`) in `chronos_explore`, `chronos_get_snippet`, and `chronos_search_graph`.
  - Lets agents inspect all method signatures, line ranges, and complexity scores of a class/module at near-zero token cost before fetching specific bodies.

### Pillar 4 — Multi-Query `chronos_explore` & Universal Token Budgeting (`max_answer_chars`)
- **Built on `v1.7.0` Seams:** `ToolRegistry` response-budget middleware.
- **No Tool Bloat (Stays at 14 MCP Tools):**
  - Instead of adding a 15th `chronos_batch` tool, upgrades `chronos_explore` (#12) to accept `queries: Vec<String>` + `include_blast_radius: bool` + `include_body: bool` + `max_answer_chars: usize` (default `24,000`).
  - Allows subagents to explore 3–5 symbols, their blast radius, verification surfaces, and 4-angle review diagnostics in a **single MCP round-trip**.

### Pillar 5 — Opt-In `LspAdapter` (`chronos lsp-hydrate` & `use_lsp: true`)
- **Built on `v1.7.0` Seams:** `SemanticResolver` trait (`AstAdapter` default).
- **Lightweight One-Shot Design:**
  - Keeps `chronos-mcp` zero-dependency and `<15ms` by default.
  - Provides `chronos lsp-hydrate` (and optional `use_lsp: true` with bounded timeout) speaking stdio JSON-RPC to installed language servers (e.g. `rust-analyzer`, `typescript-language-server`, `pyright-langserver`, `gopls`) to resolve polymorphic `unresolved_calls` and ingest compiler diagnostics.
