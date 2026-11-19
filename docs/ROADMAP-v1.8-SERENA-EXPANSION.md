# CHRONOS-AGENT v1.8.0 & v1.8.1 Roadmap — Serena-Inspired Symbolic Action, Refactoring & Hybrid LSP Engine

> **Target Releases:**
> - **`v1.8.0` (Core Symbolic Action & Rhai Script Release):** [`docs/IMPLEMENTATION-PLAN-v1.8.md`](./IMPLEMENTATION-PLAN-v1.8.md) — `chronos_edit_symbol` (#13), `chronos_refactor_symbol` (#14), Progressive `name_path` (`Class/*`, `include_body: false`), Multi-Query `chronos_explore`, and Embedded Sandboxed `rhai` 5D Script Engine (`chronos eval` / `chronos repl`).
> - **`v1.8.1` (Follow-Up Hybrid LSP Release):** On-Demand `LspSemanticAdapter` & `chronos lsp-hydrate`.
> **Primary Sources Inspected:**
> - Installed `serena` (`oraios/serena`) MCP Server schemas & runtime (`initial_instructions`, `activate_project`, `serena_repl` facades)
> - Agent Memory record: `mem_mvpcp6mh_f2cd008c8e98`

---

## 1. Strategic Vision (`v1.8.0` → `v1.8.1`)

With `v1.7.0`'s 6 core module deepenings (`WorkspaceSession`, `ToolRegistry`, `GraphRepository`, `CodeIndex` + `SourceLocation` byte spans, `ContractEngine`, and `MutationArena` + `TextEdit`) complete, the `v1.8.x` series transforms CHRONOS from a **5D Code Intelligence Sensor** into a **Blast-Radius-Gated Symbolic Action Engine**:

- **`v1.8.0`** ships **2 new consolidated write tools (14 MCP tools total)**, fresh-parse symbol editing, graph-edge-verified multi-file refactoring, progressive `Class/*` outlines, and an embedded **`rhai` 5D Script Engine** — with **zero background daemon overhead** (`<15ms` Tree-sitter default).
- **`v1.8.1`** layers the opt-in **On-Demand Hybrid `LspSemanticAdapter` (`chronos lsp-hydrate`)** onto the `SemanticResolver` seam to resolve polymorphic cross-module call sites and compiler diagnostics.

---

## 2. Four Core Pillars in `v1.8.0` (Detailed in `IMPLEMENTATION-PLAN-v1.8.md`)

### Pillar 1 — Fresh-Parse Symbol-Boundary AST Editing (`chronos_edit_symbol` — 13th MCP Tool)
- **Operations:** `"replace_body"` | `"insert_before"` | `"insert_after"` (alias: `chronos_replace_symbol_body`).
- **Defense-in-Depth:**
  1. **Fresh Disk Re-Parse:** Always re-parses the target file from disk right before editing (never mutates using cached graph byte offsets) and resolves attached leading trivia (`full_symbol_span` including doc-comments/attributes vs `declaration_span`).
  2. **In-Memory Tree-sitter Syntax Gate:** Rejects any edit that introduces a syntax error before touching disk.
  3. **Contract & Caller Blast-Radius Gate:** Internal body edits write immediately via `AtomicFileWriter` (preserving `CRLF`/`LF` and UTF-8 BOM); signature changes on symbols with active callers (`fan_in > 0`) return `status: "blocked_requires_force"` + `isError: true` with a unified diff preview unless `force: true`.

### Pillar 2 — Graph-Edge-Verified Multi-File Refactoring (`chronos_refactor_symbol` — 14th MCP Tool)
- **Actions:** `"rename"` | `"safe_delete"` | `"preview_usages"` (aliases: `chronos_rename_symbol`, `chronos_safe_delete`).
- **Precision & Safety:**
  - **`rename`:** Distinguishes graph-verified occurrences (declaration + verified `CALLS`/`TESTS` call sites + `IMPORTS` specifiers) from unverified candidate sites (member accesses `obj.foo` without receiver type info, homonyms `> 1`, or files with `unresolved_calls > 0`). Blocks with `status: "blocked_ambiguous_rename"` if any ambiguity exists unless `force: true`. Commits across multiple files transactionally via `AtomicFileWriter` with full rollback on any syntax or I/O failure.
  - **`safe_delete`:** Blocks deletion if active `CALLS`, `TESTS`, `ApiRoute`, or entrypoint/public/trait-impl guards apply (unless `force: true`), and cleanly removes `full_symbol_span` (including leading doc-comments and decorators).

### Pillar 3 — Hierarchical `name_path` (`Class/*`) & Multi-Query `chronos_explore`
- Supports `StructName/method_name` and wildcard `ClassName/*` with `include_body: false`, `depth`, `queries: Vec<String>`, and `include_blast_radius: bool` in `chronos_explore` (#12).
- Lets agents inspect all method signatures, line ranges, and complexity scores of a class/module or batch 3–5 symbol lookups in a single MCP call.

### Pillar 4 — Sandboxed `rhai` 5D Script Engine (`chronos repl` / `chronos eval` & Read-Only MCP `script`)
- Embeds `rhai` (`Engine::new_raw()`) with strict operation, call-depth, and memory limits (`max_operations = 50_000`).
- **Strict Privilege Separation:**
  - Inside MCP `chronos_explore.script` (and `--readonly` CLI), runs `ScriptProfile::ReadOnly` (exposing only `explore`, `search`, `blast_radius`, `hotspots`, `arch`, `snippet`, `cypher` — zero write functions, preventing prompt-injection write escalation).
  - Inside CLI `chronos eval` and `chronos repl`, runs `ScriptProfile::FullCli` (adding `edit_symbol` and `refactor_symbol` behind the 2-stage gate).

---

## 3. Follow-Up Milestone `v1.8.1` — Opt-In Hybrid `LspSemanticAdapter` (`chronos lsp-hydrate`)

- **Built on `v1.7.0` Seam:** `SemanticResolver` trait (`AstSemanticAdapter` default).
- **Hardened Cross-Platform Design for `v1.8.1`:**
  - Resolves Windows npm `.cmd` wrappers (`typescript-language-server.cmd`, `pyright-langserver.cmd`) alongside native binaries (`rust-analyzer`, `gopls`, `clangd`).
  - Speaks `Content-Length` framed stdio JSON-RPC (`initialize` → `initialized` → waits for `$/progress` indexing completion → `textDocument/didOpen` → `references` / `rename` → `shutdown`).
  - Uses tiered timeouts (`60s` for CLI `chronos lsp-hydrate` cold indexing; `15s` for on-demand queries) and gracefully falls back to `AstSemanticAdapter` with explicit `coverage.residual_unknowns` when a language server is absent or times out.
