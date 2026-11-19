<p align="center">
  <img src="assets/chronos_hero_banner.jpg" alt="CHRONOS-AGENT Tactile Hardware Banner" width="100%" style="border-radius: 12px;"/>
</p>

<h1 align="center">CHRONOS-AGENT</h1>

<p align="center">
  <strong>5D Living Code Graph, 1-Call Explore (<code>chronos_explore</code>), Fresh-Parse AST Symbol Editing &amp; Edge-Verified Refactoring, Sandboxed Rhai 5D Scripting, 4-Angle Code Review Audit, Universal Multi-Agent Suite (20 Native Files) &amp; Super-Mutant Gate</strong>
</p>

<p align="center">
  <a href="https://github.com/ptai-eng/chronos-agent/releases/tag/v1.8.0"><img src="https://img.shields.io/badge/Version-1.8.0-111827?style=flat-square" alt="Version 1.8.0"></a>
  <a href="https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent"><img src="https://img.shields.io/badge/VS%20Code%20Marketplace-CHRONOS%20Agent-007ACC?style=flat-square&logo=visualstudiocode&logoColor=white" alt="VS Code Marketplace"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2021%20Edition-EB6C36?style=flat-square&logo=rust&logoColor=white" alt="Rust Edition"></a>
  <a href="./chronos-core/tests"><img src="https://img.shields.io/badge/Tests-147%2F147%20Passing-1A7F5A?style=flat-square" alt="Tests 147/147 Passing"></a>
  <a href="https://spec.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP-14%20Tools%20%28v2024--11--05%29-2E5AA8?style=flat-square" alt="MCP Protocol 14 Tools"></a>
  <a href="./AGENTS.md"><img src="https://img.shields.io/badge/Multi--Agent-Codex%20%7C%20Antigravity%20%7C%20Claude%20%7C%20OpenCode-10A37F?style=flat-square" alt="Multi-Agent Ready"></a>
  <a href="./SECURITY.md"><img src="https://img.shields.io/badge/Security-4--Layer%20Hardened-B42318?style=flat-square" alt="Security Notice"></a>
</p>

---

## Table of Contents

- [What's New in v1.8.0](#whats-new-in-v180)
- [Why CHRONOS Exists](#why-chronos-exists)
- [Core Architecture: The 4 Pillars](#core-architecture-the-4-pillars)
- [Capability Matrix](#capability-matrix)
- [Developer HUD &amp; VS Code Extension](#developer-hud--vs-code-extension)
- [Universal AI Integrations (14 MCP Tools &amp; 20 Native Files)](#universal-ai-integrations-14-mcp-tools--20-native-files)
- [Quickstart &amp; Installation](#quickstart--installation)
- [CLI Command Suite](#cli-command-suite)
- [Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)](#security--privacy-notice-nvidia-skillspector-alert)

---

## What's New in v1.8.0

`v1.8.0` turns `CHRONOS-AGENT` from a read-only 5D analysis graph into a surgical **read + edit + refactor + script** engineering harness on top of fresh Tree-sitter parsing and blast-radius gates, while keeping execution zero-daemon (`<15ms`):

| Subsystem | v1.8.0 Capability |
| :--- | :--- |
| **1. Fresh-Parse AST Editing (`chronos_edit_symbol` #13 &amp; `chronos edit`)** | Never mutates disk using stale graph byte offsets. Always reads and re-parses the target file with Tree-sitter immediately before writing, separating `declaration_span` from `full_symbol_span` so Rust `#[attributes]`, Python `@decorators`, doc-comments, and `export` wrappers never duplicate. Preserves original indentation, UTF-8 BOM, and `CRLF`/`LF` via `AtomicFileWriter`, and enforces a **2-Stage Gate**: Stage 1 Tree-sitter syntax verification + Stage 2 signature contract &amp; `fan_in > 0` guard (returning `isError: true`, `status: "blocked_requires_force"`, and unified diff unless `force: true`). |
| **2. Edge-Verified Refactoring (`chronos_refactor_symbol` #14, `chronos rename`, `chronos delete`)** | Separates `verified_edits` (declaration, `CALLS`/`TESTS`-verified calls, non-module-path `import`/`use` specifiers) from `candidate_sites` (unverified member accesses `obj.sym`, shadowed local bindings, callback values) while skipping comments and string literals. Blocks ambiguous renames (`blocked_ambiguous_rename`) whenever candidates, homonyms, or unresolved calls exist without `force: true`, syntax-checks 100% of touched files in memory before multi-file atomic commit/rollback, and hardens `safe_delete` against active callers, covering tests, `ApiRoute`s, entrypoints, `pub`/export coverage gaps, and Rust trait impls. |
| **3. Progressive `Class/*` &amp; Batch `chronos_explore` (#12)** | Supports `Class/*` wildcard outlines (with per-`impl` `NodeId` disambiguation in Rust), `include_body: false` signature-only slices, `depth` child expansion, batch `queries[]`, and inline `include_blast_radius` risk summaries in one round-trip. |
| **4. Sandboxed `rhai` 5D Script Engine (`chronos_explore.script`, `chronos eval`, `chronos repl`)** | Embeds `rhai` (`Engine::new_raw()`, zero filesystem/shell/network I/O, bounded at `50_000` operations) with two strict capability profiles: `ScriptProfile::ReadOnly` for MCP `chronos_explore.script` and `--readonly` CLI (write functions are never registered), and `ScriptProfile::FullCli` for gated CLI scripts and interactive REPL sessions. |
| **5. `AtomicFileWriter` &amp; `xxHash64` Deduplication** | Single-file and transactional multi-file commits preserve encoding and roll back every prior file on failure; `WorkspaceSession::index_file` tracks `last_indexed_hashes` to skip redundant `FileWatcher` re-parses in `<0.2ms`. See [`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./docs/ROADMAP-v1.8-SERENA-EXPANSION.md) for the `v1.8.1` On-Demand Hybrid LSP plan. |

<details>
<summary><strong>Previous Release Highlights (v1.7.0, v1.6.0, v1.5.0, v1.4.0, v1.3.0 &amp; v1.2.0)</strong></summary>

- **v1.7.0 — Deepened Core Architecture, `KuzuDB 0.11.3` &amp; 4 Bug Fixes:** Introduced `WorkspaceSession` (`chronos-core/src/session.rs`), canonical `ToolRegistry` / `McpToolRegistry` eliminating schema drift (`BUG-04`), Schema v2 `GraphRepository` (`BUG-01`..`BUG-03`), `_meta.contract_alerts` middleware, universal 24k-char UTF-8 truncation (`max_answer_chars`), and upgraded embedded `KuzuDB` to single-file `0.11.3` (`cxx-build = 1.0.138`) resolving GCC 15 CI aborts.
- **v1.6.0 — 1-Call Context, 4-Angle Review Audit, Surface Tracer &amp; Hooks:** Added `chronos_explore`, Reverse-BFS `verification_surfaces`, `structured_findings` (`failure_scenario`), 4-Angle Code Review Audit (`Reuse`, `Efficiency`, `Simplification`, `Altitude`), `<20ms` lifecycle hooks (`chronos hook`), and the 20-file multi-agent scaffolder (`chronos init --client all --with-hooks`).
- **v1.5.0 — Universal Multi-Agent Synthesizer &amp; AI SDK Schema Exporter:** Added `chronos init`, `chronos agents-md`, `chronos schema --format <openai|responses|anthropic|gemini|codex-toml|mcp>`, and `SECURITY.md`.
- **v1.4.0 — 5D Graph, Merkle Cache, Route Stitching, Super-Mutant &amp; Offline Hydrator:** Added `xxHash64` KuzuDB delta sync, 2-pass Frontend/Backend HTTP route stitching (`/{*}`), Git temporal co-changes (`CO_CHANGES_WITH`), single-compile `CHRONOS_MUTANT_ID` Super-Mutant evaluation, and offline profiler ingestion (`.cpuprofile`, Speedscope, Folded Flamegraph, OTLP JSON).
- **v1.3.0 — Polyglot 7-Language AST &amp; Cypher Engine:** Expanded Tree-sitter parsing across TS/JS, Python, Rust, Go, C, C++, and Java; added interprocedural `transitive_loop_depth`, `linear_scan_in_loop` $O(N^2)$ detection, OpenCypher (`chronos_query_graph`), BM25 search, and Label Propagation community clustering.
- **v1.2.0 — KuzuDB Persistence &amp; PageRank Uncertainty:** Full graph persistence across restarts, damped PageRank ($\alpha = 0.85$) with `confidence_interval` and `residual_unknowns`, `chronos diff --ci`, and zero-copy `node_modules`/`.venv` sandboxes.

</details>

---

## Why CHRONOS Exists

Coding agents inspect source files as disconnected text buffers. When an agent edits a helper in `Module A`, flat text search hides whether that function sits on a high-PageRank call path, serves a frontend `fetch()` caller across language boundaries, co-changes with another module in Git history, nests inside a quadratic loop, or breaks downstream callers when renamed.

**CHRONOS-AGENT** anchors agent reads and writes inside a verified 5D model:

1. **Polyglot Structural Topology &amp; Fresh-Parse Mutations:** Builds a 7-language AST graph (`Petgraph` in-memory + incremental `xxHash64` `KuzuDB` on disk), stitches cross-language HTTP routes, and validates every symbol edit or multi-file rename against a fresh disk CST parse before touching files.
2. **5th-Dimension Git Temporal Co-Change Coupling:** Mines Git commit history (`CO_CHANGES_WITH`, $P(B \mid A) \ge 40\%$) to expose co-evolving modules even without static AST imports.
3. **Live &amp; Offline Runtime Telemetry Overlay:** Maps live CDP V8 / Linux eBPF probes or offline `.cpuprofile`, Speedscope, Folded Flamegraph, and OTLP traces onto AST function nodes.
4. **Super-Mutant &amp; Graph-Guided Mutation Arena:** Queries `TESTS` edges to select covering tests and toggles mutations in a single build via `CHRONOS_MUTANT_ID`.
5. **Explicit Epistemic Contracts:** Discloses call-graph observation completeness (`complete`, `partial`, or `unknown`), `[low, high]` confidence intervals, and unresolved call sites (`residual_unknowns`).

---

## Core Architecture: The 4 Pillars

<p align="center">
  <img src="assets/chronos_architecture.svg" alt="CHRONOS-AGENT Architecture Schematic" width="100%"/>
</p>

| Pillar | Responsibility | Implementation | Crate Path |
| :--- | :--- | :--- | :--- |
| **1. 5D Knowledge Graph, Fresh AST Editor &amp; Rhai Engine** | 7-language AST topology, cross-boundary HTTP routes, Git co-changes, `xxHash64` Merkle cache, OpenCypher, BM25 search, 1-call `chronos_explore`, gated `chronos_edit_symbol` &amp; `chronos_refactor_symbol`, and sandboxed `rhai` 5D scripts | Tree-sitter (`TS`/`JS`/`Py`/`Rs`/`Go`/`C`/`C++`/`Java`) &bull; `Petgraph` &bull; Single-file `KuzuDB 0.11.3` &bull; `AtomicFileWriter` &bull; `rhai` | `chronos-core/src/{graph,ast,script,session}.rs` |
| **2. Super-Mutant Arena** | Semantic test verification before code acceptance | 5 deterministic operators &bull; `CHRONOS_MUTANT_ID` single-compile engine &bull; Parallel temp-dir sandboxes with zero-copy `node_modules`/`.venv` junctions | `chronos-core/src/mutation/` |
| **3. Semantic Contract Broker** | Real-time signature lock &amp; Pre-Tool Execution Gate | 300ms debounced `notify` watcher &bull; Polyglot `ModuleSignature` diffing &bull; `_meta.contract_alerts` injection &bull; Stage 2 edit gating | `chronos-core/src/contract/` |
| **4. Blast-Radius &amp; Review Engine** | Impact scoring, uncertainty intervals, surface tracer, 4-angle audit &amp; structured findings | 4-tier classification (`SAFE` / `MEDIUM` / `HIGH` / `CRITICAL`) &bull; `verification_surfaces` &bull; `structured_findings` &bull; `CoverageReport` &amp; `confidence_interval` | `chronos-core/src/blast_radius/` |

---

## Capability Matrix

<p align="center">
  <img src="assets/chronos_benchmark_radar.svg" alt="CHRONOS-AGENT Capability Radar Benchmark" width="100%"/>
</p>

### Comparison Across Tool Classes

| Capability | Static Linters (`ESLint`, `SonarQube`) | Vanilla Coding Agents (`Copilot`, `Cursor` alone) | Static Graph MCPs (`GitNexus`, `codebase-memory`) | **CHRONOS-AGENT v1.8** |
| :--- | :---: | :---: | :---: | :---: |
| **1-Call Context &amp; Wildcards (`chronos_explore`)** | No | 5–8 grep/read calls | Search or snippet separately | **`Class/*`, `include_body`, Batch `queries[]`, Surfaces, 4-Angle Audit &amp; ReadOnly Rhai** |
| **Gated AST Symbol Editing (`chronos_edit_symbol`)** | No | Raw string line/diff overwrite | Read-only | **Fresh CST Re-Parse, Trivia Dedup, Syntax + Blast-Radius Gate, Atomic BOM/CRLF Write** |
| **Edge-Verified Multi-File Refactor (`chronos_refactor_symbol`)** | No | Text search/replace | Read-only | **`verified_edits` vs. `candidate_sites`, Homonym Guard, Multi-File Rollback &amp; Hardened `safe_delete`** |
| **Polyglot Call &amp; Test Graph** | Single-file rules | Token window only | Static AST edges | **7 Languages + `xxHash64` Merkle Cache + Single-File KuzuDB 0.11** |
| **Cross-Language HTTP Route Stitching** | No | No | File-local only | **2-Pass Normalized Route Bridge (`/{*}`)** |
| **Git Temporal Co-Change Coupling** | No | No | No | **`CO_CHANGES_WITH` Conditional Probability ($P(B\mid A)$)** |
| **4-Angle Review &amp; Verification Surfaces** | Syntax/lint rules | Prompt-only | Caller list only | **Reverse-BFS Entrypoints + `Reuse`/`Efficiency`/`Simplification`/`Altitude` Findings** |
| **Multi-Agent Suite &amp; Lifecycle Hooks** | No | Manual rules | No | **20 Native Files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `SKILL.md`, `<20ms` Hooks)** |
| **Hidden $O(N^2)$ Loop &amp; Runtime Telemetry** | Basic lint rules | No | Intra-function only | **Interprocedural `transitive_loop_depth` + V8 `.cpuprofile` / Speedscope / OTLP / eBPF** |
| **Mutation Gate &amp; Uncertainty Bounds** | No | No | No | **Super-Mutant Sandboxes + `confidence_interval` + `residual_unknowns`** |

---

## Developer HUD &amp; VS Code Extension

<p align="center">
  <img src="assets/chronos_radar_dashboard.svg" alt="CHRONOS-AGENT Developer HUD &amp; Telemetry View" width="100%"/>
</p>

> [!TIP]
> **Available on Visual Studio Marketplace:**  
> Search **"CHRONOS Agent"** in the VS Code Extensions view (`Ctrl+Shift+X`), or install from the [VS Code Marketplace Listing](https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent).

The companion extension (`chronos-vscode/`) integrates `chronos-mcp` directly into VS Code:

- **Incremental Polyglot Watcher:** Re-indexes saved `.ts`, `.tsx`, `.js`, `.py`, `.rs`, `.go`, `.c`, `.cpp`, and `.java` files, evicts deleted symbols, rebuilds cross-boundary call edges, and syncs to `KuzuDB`.
- **Blast-Radius &amp; Uncertainty HUD:** Shows composite impact scores, `[low, high]` confidence intervals, PageRank hubs, Git co-change partners, transitive loop depths, and surviving mutants inline.
- **Zero-Config Binary Discovery:** Connects to local workspace binaries or pulls release archives automatically.

---

## Universal AI Integrations (14 MCP Tools &amp; 20 Native Files)

`chronos-mcp` implements **Model Context Protocol (MCP v2024-11-05)** over stdio JSON-RPC 2.0 and exposes **14 canonical tools** (plus compatibility aliases `chronos_replace_symbol_body`, `chronos_rename_symbol`, `chronos_safe_delete`, `--readonly` enforcement, `_meta.contract_alerts` injection, 24k `max_answer_chars` truncation, and native SDK exports via `chronos schema`):

| # | Tool Name | Mode | Category | Description |
| :---: | :--- | :---: | :--- | :--- |
| 1 | `chronos_analyze_file` | Read | Ingestion &amp; Audit | Incrementally parse source file (7 language families), update KuzuDB delta, and run the 4-Angle Review Audit. |
| 2 | `chronos_query_hotspots` | Read | Telemetry | Rank runtime bottlenecks, PageRank hubs, and quadratic loop hotspots. |
| 3 | `chronos_blast_radius` | Read | Risk &amp; Findings | Compute PageRank blast-radius, `verification_surfaces`, line-anchored `structured_findings`, Git co-changes, and uncertainty bounds. |
| 4 | `chronos_mutation_gate` | Read | Verification | Execute graph-selected covering tests against Super-Mutant builds before accepting edits. |
| 5 | `chronos_contract_status` | Read | Contracts | Drain queued Semantic Contract Broker violation alerts from the Pre-Tool Execution Gate. |
| 6 | `chronos_query_graph` | Read | Cypher | Run read-only OpenCypher queries over KuzuDB (`CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`, `CO_CHANGES_WITH`). |
| 7 | `chronos_search_graph` | Read | Discovery | Search symbols with BM25 identifier tokenization (`camelCase`/`snake_case`), regex, degree, and loop filters. |
| 8 | `chronos_get_snippet` | Read | Context | Read exact `start_line..=end_line` source snippets with callers, callees, covering tests, and AST metrics. |
| 9 | `chronos_get_architecture` | Read | Topology | Index workspace incrementally, mine Git co-changes, and compute Label Propagation clusters + hotspots. |
| 10 | `chronos_hydrate_telemetry` | Read | Telemetry | Hydrate Dimension 4 runtime metrics from `.cpuprofile`, Speedscope/`py-spy`, Folded Flamegraph, or OTLP JSON. |
| 11 | `chronos_generate_agents_md` | Read/Preview | Multi-Agent | Synthesize live [`AGENTS.md`](./AGENTS.md), [`CLAUDE.md`](./CLAUDE.md), [`GEMINI.md`](./GEMINI.md), Antigravity `SKILL.md`, Cursor `.mdc` rules, and lifecycle hooks (`write_to_disk: true` blocked in `--readonly`). |
| 12 | `chronos_explore` | Read | **1-Call Context** | 1-call context engine supporting `Class/*` wildcards, `include_body: false`, `depth`, batch `queries[]`, inline `include_blast_radius`, and sandboxed `ScriptProfile::ReadOnly` Rhai `script` execution. |
| 13 | `chronos_edit_symbol` | **Write** | **AST Editing** | **New in v1.8:** Fresh-parse `replace_body`, `insert_before`, `insert_after` with trivia deduplication, BOM/CRLF preservation, and 2-Stage syntax + caller gate (alias `chronos_replace_symbol_body`; hidden/blocked under `--readonly`). |
| 14 | `chronos_refactor_symbol` | **Write / Preview** | **Refactoring** | **New in v1.8:** Edge-verified `rename`, hardened `safe_delete`, and `preview_usages` with `verified_edits` vs. `candidate_sites` separation and multi-file atomic rollback (aliases `chronos_rename_symbol`, `chronos_safe_delete`; `--readonly` permits `preview_usages` and `dry_run: true`). |

### One-Command Auto-Scaffold (`chronos init --client all --with-hooks`)

| AI Coding CLI / IDE | One-Command Bootstrap | Synthesized Context / Skill / Rule Files | Native MCP &amp; Hook Configs |
| :--- | :--- | :--- | :--- |
| **All Ecosystems (20 files)** | `chronos init --client all --with-hooks` | All rules, skills, subagents &amp; slash commands below | All MCP &amp; `<20ms` hook configs below |
| **OpenAI Codex CLI &amp; ChatGPT Codex** | `chronos init --client codex` | [`AGENTS.md`](./AGENTS.md) | [`.codex/config.toml`](./.codex/config.toml) (`codex mcp add`) |
| **Google Antigravity (`agy` CLI &amp; IDE)** | `chronos init --client antigravity --with-hooks` | [`GEMINI.md`](./GEMINI.md) + [`.agents/skills/chronos-agent/SKILL.md`](./.agents/skills/chronos-agent/SKILL.md) | [`.agents/plugins/chronos-agent/mcp_config.json`](./.agents/plugins/chronos-agent/mcp_config.json) + [`.agents/hooks.json`](./.agents/hooks.json) |
| **Anthropic Claude Code &amp; Desktop** | `chronos init --client claude --with-hooks` | [`CLAUDE.md`](./CLAUDE.md) + [`.claude/commands/chronos-audit.md`](./.claude/commands/chronos-audit.md) | [`.mcp.json`](./.mcp.json) + [`.claude/settings.json`](./.claude/settings.json) |
| **OpenCode CLI (`opencode`)** | `chronos init --client opencode` | [`.opencode/agent/chronos-auditor.md`](./.opencode/agent/chronos-auditor.md) | [`opencode.json`](./opencode.json) |
| **Cursor IDE &amp; Agent CLI** | `chronos init --client cursor` | [`.cursor/rules/chronos-5d.mdc`](./.cursor/rules/chronos-5d.mdc) (`alwaysApply: true`) | [`.cursor/mcp.json`](./.cursor/mcp.json) |
| **Windsurf Cascade** | `chronos init --client windsurf` | [`.windsurfrules`](./.windsurfrules) | [`.windsurf/mcp.json`](./.windsurf/mcp.json) |
| **GitHub Copilot Agent &amp; VS Code** | `chronos init --client copilot` | [`.github/copilot-instructions.md`](./.github/copilot-instructions.md) | [`.vscode/mcp.json`](./.vscode/mcp.json) |
| **Cline &amp; Roo Code** | `chronos init --client cline` | [`.clinerules`](./.clinerules) | `cline.mcpServers` / `.roo/mcp.json` |

Generate native client scaffolds or export all 14 AI SDK tool schemas:

```bash
chronos init --client all --with-hooks           # Scaffold all 20 native rule, skill, MCP & <20ms hook files
chronos explore "Calculator/*" --no-body         # Wildcard Class/* signature outlines without full body
chronos schema --format responses > openai.json  # OpenAI Responses API (14 tools, strict: true)
chronos schema --format anthropic > claude.json  # Anthropic Claude Messages API (14 tools, input_schema)
chronos schema --format gemini > gemini.json     # Google Gemini / Vertex AI (14 tools, functionDeclarations)
```

---

## Quickstart &amp; Installation

### Option 1: Pre-Compiled Binaries

Download `v1.8.0` archives from [GitHub Releases](https://github.com/ptai-eng/chronos-agent/releases/tag/v1.8.0):
- **Windows (x64):** `chronos-windows-x64.zip` (`chronos.exe` + `chronos-mcp.exe`)
- **Linux (x64):** `chronos-linux-x64.tar.gz` (`chronos` + `chronos-mcp`)
- **macOS (Apple Silicon):** `chronos-macos-arm64.tar.gz` (`chronos` + `chronos-mcp`)

### Option 2: Build from Source

```powershell
# Prerequisites: Rust 1.85+ (Cargo workspace), CMake / C++ toolchain (for KuzuDB 0.11 FFI)
git clone https://github.com/ptai-eng/chronos-agent.git
cd chronos-agent
cargo build --release -j 4

# Compiled binaries:
# ./target/release/chronos-mcp.exe  (MCP Server — 14 Tools)
# ./target/release/chronos.exe      (CLI Harness)
```

---

## CLI Command Suite

```powershell
# 1. Surgical AST Symbol Editing (2-stage syntax + blast-radius gate)
chronos edit multiply --file src/calc.rs --op replace_body --code "pub fn multiply(a: i32, b: i32) -> i32 { a * b }" --dry-run
chronos edit multiply --file src/calc.rs --op replace_body --code-file patch.rs --force

# 2. Edge-Verified Multi-File Rename & Hardened Safe Delete
chronos rename compute_tax calculate_vat --file src/math.rs --dry-run
chronos delete legacy_helper --file src/cleanup.rs

# 3. Embedded Rhai 5D Scripting (Eval & Interactive REPL)
chronos eval "hotspots(5)" --readonly --json
chronos repl                                     # Interactive 5D graph REPL (use --readonly to lock mutations)

# 4. 1-Call Context Explore (Wildcards, Signature-Only, Batch Queries & Blast-Radius)
chronos explore "processOrdersWithUserInfo" --max-symbols 2 --blast-radius
chronos explore "UserService/*" --no-body --depth 2 --queries handleAuth

# 5. Full 4-Pillar Audit & Executive Glance
chronos audit src/services/orderService.ts
chronos glance src/services/orderService.ts --symbol processOrdersWithUserInfo

# 6. Hydrate Offline Runtime Telemetry & Inspect Architecture
chronos hydrate profile.cpuprofile --json
chronos arch chronos-core/src
chronos search --query "blast radius" --limit 5
chronos search --pattern ".*Parser.*" --min-loop-depth 2
chronos snippet handle_blast_radius

# 7. Read-Only OpenCypher Query against Embedded KuzuDB 0.11
chronos query "MATCH (n:ChronosNode) WHERE n.transitive_loop_depth >= 2 RETURN n.name, n.loop_depth, n.transitive_loop_depth LIMIT 10"
chronos query "MATCH (a:ChronosNode)-[r:CO_CHANGES_WITH]->(b:ChronosNode) RETURN a.name, b.name, r.call_site_line AS confidence_pct"

# 8. Git Diff Blast-Radius CI Gate & Real-Time Contract Monitor
chronos status --watch src/services/
chronos diff --staged --json --ci                # Exits non-zero on HIGH risk

# 9. Universal Client Scaffolding & Hook Entrypoint
chronos init --client all --with-hooks
chronos hook session-start --client antigravity
```

---

## Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)

> [!WARNING]
> **Why static keyword scanners flag system-level primitives in this repository:**
> 
> `CHRONOS-AGENT` bundles low-level debugging, profiling, and subprocess orchestration components that trigger heuristic keyword scanners:
>
> 1. **eBPF Kernel Tracepoints (`chronos-core/src/telemetry/ebpf_adapter.rs`):** Attaches Linux uprobe/tracepoints for low-overhead latency profiling.
> 2. **Chrome DevTools Protocol (`chronos-core/src/telemetry/cdp_client.rs`):** Connects to local V8 inspector WebSockets for live CPU and heap allocation sampling.
> 3. **Subprocess Mutation Runner (`chronos-core/src/mutation/runner.rs`):** Spawns isolated test-runner processes inside temporary sandboxes with timeout watchdogs.
> 4. **Client Scaffold Generator (`chronos-cli/src/commands/mcp_info.rs`):** Writes workspace MCP configs for `.codex/`, `.cursor/`, `.claude/`, and `.agents/`.
>
> **Runtime Guardrails Enforced by Default:** Every MCP and CLI session validates paths inside workspace root (`validate_workspace_path`), blocks write tools and write actions under `--readonly`, separates `ScriptProfile::ReadOnly` (zero write functions, zero filesystem/network/shell I/O, bounded `50_000` operations) from CLI scripts, preserves encoding atomically with rollback via `AtomicFileWriter`, enforces read-only OpenCypher (`validate_readonly_cypher`), and tags extracted snippets with `_meta.provenance.untrusted_content`. See [`SECURITY.md`](./SECURITY.md) for the complete 4-layer threat model.

---

## License

Distributed under the **MIT License**. See [LICENSE](./LICENSE) for details.
