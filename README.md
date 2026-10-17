<p align="center">
  <img src="assets/chronos_hero_banner.jpg" alt="CHRONOS-AGENT Tactile Hardware Banner" width="100%" style="border-radius: 12px;"/>
</p>

<h1 align="center">CHRONOS-AGENT</h1>

<p align="center">
  <strong>5D Living Code Graph, Polyglot Cypher Engine, Cross-Boundary Route Stitching, Git Co-Change Blast-Radius &amp; Super-Mutant Gating for AI Coding Agents</strong>
</p>

<p align="center">
  <a href="https://github.com/ptai-eng/chronos-agent/releases/tag/v1.4.0"><img src="https://img.shields.io/badge/Version-1.4.0-111827?style=flat-square" alt="Version 1.4.0"></a>
  <a href="https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent"><img src="https://img.shields.io/badge/VS%20Code%20Marketplace-CHRONOS%20Agent-007ACC?style=flat-square&logo=visualstudiocode&logoColor=white" alt="VS Code Marketplace"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2024%20Edition-EB6C36?style=flat-square&logo=rust&logoColor=white" alt="Rust Version"></a>
  <a href="./chronos-core/tests"><img src="https://img.shields.io/badge/Tests-111%2F111%20Passing-1A7F5A?style=flat-square" alt="Tests 111/111 Passing"></a>
  <a href="https://spec.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP-10%20Tools%20%28v2024--11--05%29-2E5AA8?style=flat-square" alt="MCP Protocol"></a>
  <a href="./integrations/INTEGRATIONS.md"><img src="https://img.shields.io/badge/AI%20Clients-9%20Supported-4B5563?style=flat-square" alt="AI Clients"></a>
  <a href="#security--privacy-notice-nvidia-skillspector-alert"><img src="https://img.shields.io/badge/Security-SkillSpector%20Audited-B42318?style=flat-square" alt="Security Notice"></a>
</p>

---

## Table of Contents

- [What's New in v1.4.0](#whats-new-in-v140)
- [Why CHRONOS Exists](#why-chronos-exists)
- [Core Architecture: The 4 Pillars](#core-architecture-the-4-pillars)
- [Capability Matrix &amp; Benchmark](#capability-matrix--benchmark)
- [Developer HUD &amp; VS Code Extension](#developer-hud--vs-code-extension)
- [Universal AI Integrations (9 Clients &amp; 10 MCP Tools)](#universal-ai-integrations-9-clients--10-mcp-tools)
- [Quickstart &amp; Installation](#quickstart--installation)
- [CLI Command Suite](#cli-command-suite)
- [Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)](#security--privacy-notice-nvidia-skillspector-alert)

---

## What's New in v1.4.0

`v1.4.0` resolves the 5 core architectural bottlenecks of polyglot graph analysis, mutation compile times, and cross-platform telemetry hydration—with **zero new external crate dependencies**:

| Subsystem | v1.4.0 Upgrade |
| :--- | :--- |
| **1. Incremental `xxHash64` Merkle Graph Cache** | Persists per-file `xxHash64` digests in `KuzuDB` (`ChronosFileHash` table). Unchanged files are skipped in `<1ms` without invoking Tree-sitter; modified files automatically evict stale/deleted symbols (`remove_nodes_for_file`) before re-ingesting. |
| **2. Cross-Boundary API &amp; Route Stitching (Frontend $\leftrightarrow$ Backend)** | Extracts HTTP route handlers (`@app.get`, `#[get]`, `@GetMapping`, `http.HandleFunc`, `app.get`, `.route`) and client calls (`fetch`, `axios`, `requests`, `http.Get`) across all 7 languages, normalizes path parameters (`:id`, `{id}`, `${userId}` $\rightarrow$ `/{*}`), and stitches cross-language `CALLS` edges regardless of file parse order via 2-pass `resolve_pending_calls_and_routes()`. |
| **3. 5th Dimension: Git Temporal Co-Change Coupling (`CO_CHANGES_WITH`)** | Mines `git log` commit history (`git_coupling.rs`) to compute conditional co-change probability $P(B \mid A) = \frac{\lvert C(A) \cap C(B)\rvert}{\lvert C(A)\rvert}$, persists `CO_CHANGES_WITH` edges in `KuzuDB`, and surfaces historically coupled files (`.sql` migrations, configs, DTOs) directly in `BlastRadiusReport`. |
| **4. Env-Switched Super-Mutant Single-Compile Engine** | Combines all non-overlapping line mutations into a single source artifact guarded by runtime `CHRONOS_MUTANT_ID` checks (`build_super_mutant_source` across Rust, Go, Python, TS/JS, C/C++, Java), reducing compiled-language mutation testing from $O(N)$ compilation passes to **$O(1)$ single compile**. |
| **5. Universal Offline Telemetry Hydrator (`chronos hydrate` / `chronos_hydrate_telemetry`)** | Hydrates the 4th Dimension (p50/p95/p99 latency, CPU% self, hot paths, bottlenecks) on Windows, macOS, and Linux from **4 standard profiler formats** without needing live root eBPF or `--inspect`: Chrome/V8 `.cpuprofile` JSON, `py-spy`/Speedscope JSON, Folded Stack Flamegraphs (`cargo-flamegraph`/`perf`/`pprof`), and OpenTelemetry (OTLP) trace JSON. |

<details>
<summary><strong>Previous Release Highlights (v1.3.0 &amp; v1.2.0)</strong></summary>

- **v1.3.0 — Polyglot 7-Language AST &amp; Cypher Engine:** Expanded Tree-sitter dispatcher to TypeScript/JS, Python, Rust, Go, C, C++, and Java; added interprocedural `transitive_loop_depth` and `linear_scan_in_loop` $O(N^2)$ detection; added read-only OpenCypher (`chronos_query_graph`), BM25 identifier search (`chronos_search_graph`), verbatim code snippets (`chronos_get_snippet`), and Label Propagation community clustering (`chronos_get_architecture`).
- **v1.2.0 — KuzuDB Edge Persistence &amp; PageRank Uncertainty:** Full KuzuDB persistence across restarts, 20-iteration damped PageRank ($\alpha = 0.85$) with explicit `confidence_interval` and `residual_unknowns`, `chronos diff --ci` AST span risk gate, and zero-copy `node_modules`/`.venv` symlink sandboxes.

</details>

---

## Why CHRONOS Exists

AI coding agents read source files as flat text buffers. When an agent edits a utility function in `Module A`, it cannot see whether that function sits on a high-PageRank call path, is called across language boundaries by a frontend `fetch()` client, co-evolves with a database schema file in Git history, triggers a transitive $O(N^2)$ nested loop, or runs inside a hot CPU loop in production.

**CHRONOS-AGENT** gives autonomous coding agents a unified 5D structural, evolutionary, and runtime verification harness:

1. **Polyglot Structural Topology + Cross-Boundary Stitching + Cypher:** Builds a 7-language AST graph (`Petgraph` in-memory + incremental `xxHash64` `KuzuDB` on disk) that stitches Frontend $\leftrightarrow$ Backend HTTP routes and is queryable via OpenCypher, BM25 identifier search, and Label Propagation community clustering.
2. **5th-Dimension Git Temporal Co-Change Coupling:** Discovers hidden evolutionary coupling ($P(B \mid A) \ge 40\%$) from Git commit history even when zero static AST imports exist.
3. **Live &amp; Offline Runtime Telemetry Overlay:** Maps live CDP V8 / Linux eBPF probes or offline `.cpuprofile`, Speedscope, Folded Flamegraph, and OTLP traces directly onto AST function nodes.
4. **Super-Mutant &amp; Graph-Guided Mutation Arena:** Queries `TESTS` edges to execute only covering tests and uses `CHRONOS_MUTANT_ID` env-switching for single-compile parallel evaluation.
5. **Explicit Epistemic Contracts:** Every blast-radius report discloses its observation coverage (`complete` vs. `partial`), uncertainty interval (`confidence_interval`), and unresolved dynamic call sites (`residual_unknowns`)—never conflating unobserved edges with zero risk.

---

## Core Architecture: The 4 Pillars

<p align="center">
  <img src="assets/chronos_architecture.svg" alt="CHRONOS-AGENT v1.4 Architecture Schematic" width="100%"/>
</p>

| Pillar | Responsibility | Implementation | Crate Path |
| :--- | :--- | :--- | :--- |
| **1. 5D Knowledge Graph &amp; Cypher** | 7-language AST topology, cross-boundary HTTP route stitching, Git temporal co-changes, incremental `xxHash64` Merkle cache, OpenCypher query engine, BM25 search, and telemetry hydration | Tree-sitter (`TS`/`JS`/`Py`/`Rs`/`Go`/`C`/`C++`/`Java`) &bull; `Petgraph` (`<15ms` traversal) &bull; Embedded `KuzuDB` (`CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`, `CO_CHANGES_WITH`) | `chronos-core/src/graph/` |
| **2. Super-Mutant Arena** | Semantic test verification before code acceptance | 5 deterministic operators &bull; `CHRONOS_MUTANT_ID` single-compile Super-Mutant engine &bull; Parallel temp-dir sandboxes with zero-copy `node_modules`/`.venv` junctions | `chronos-core/src/mutation/` |
| **3. Semantic Contract Broker** | Real-time public signature lock across concurrent agents | 300ms debounced `notify` file watcher &bull; Signature fingerprinting &bull; Breaking-change interception before commit | `chronos-core/src/contract/` |
| **4. Blast-Radius Engine** | Risk scoring with uncertainty bounds &amp; Git coupling | 4-tier classification (`SAFE` / `MEDIUM` / `HIGH` / `CRITICAL`) &bull; Git co-change partners &bull; `CoverageReport` &amp; `confidence_interval` | `chronos-core/src/blast_radius/` |

---

## Capability Matrix &amp; Benchmark

<p align="center">
  <img src="assets/chronos_benchmark_radar.svg" alt="CHRONOS-AGENT v1.4 Capability Radar Benchmark" width="100%"/>
</p>

### Comparison Across Tool Classes

| Capability | Static Linters (`ESLint`, `SonarQube`) | Vanilla Coding Agents (`Copilot`, `Cursor` alone) | Static Graph MCPs (`GitNexus`, `codebase-memory`) | **CHRONOS-AGENT v1.4** |
| :--- | :---: | :---: | :---: | :---: |
| **Polyglot Call &amp; Test Graph** | Single-file rules | Token window only | Static AST edges | **7 Languages + Incremental `xxHash64` Merkle Cache** |
| **Cross-Language HTTP Route Stitching** | No | No | File-local only | **2-Pass Normalized Route Bridge (`/{*}`)** |
| **Git Temporal Co-Change Coupling** | No | No | No | **`CO_CHANGES_WITH` Conditional Probability ($P(B\mid A)$)** |
| **Cypher &amp; BM25 Code Discovery** | No | Grep / Glob only | Read-only Cypher + BM25 | **Read-Only KuzuDB Cypher + BM25 + Snippet Context** |
| **Hidden $O(N^2)$ Loop Detection** | Basic lint rules | No | Intra-function only | **Interprocedural `transitive_loop_depth` + `linear_scan_in_loop`** |
| **Runtime &amp; Offline Telemetry** | No | No | No | **V8 `.cpuprofile` + Speedscope + Flamegraph + OTLP + CDP/eBPF** |
| **Mutation Testing Gate** | No | No | No | **Super-Mutant Single-Compile + Graph-Guided Sandboxes** |
| **Uncertainty &amp; Coverage Bounds** | No | No | Unobserved = 0 callers | **`confidence_interval` + `residual_unknowns`** |

---

## Developer HUD &amp; VS Code Extension

<p align="center">
  <img src="assets/chronos_radar_dashboard.svg" alt="CHRONOS-AGENT v1.4 Developer HUD & Telemetry View" width="100%"/>
</p>

> [!TIP]
> **Available on Visual Studio Marketplace:**  
> Search **"CHRONOS Agent"** in the VS Code Extensions view (`Ctrl+Shift+X`), or install from the [VS Code Marketplace Listing](https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent).

The companion extension (`chronos-vscode/`) integrates the `chronos-mcp` engine into your editor:

- **Incremental Polyglot Watcher:** Automatically re-indexes modified `.ts`, `.tsx`, `.js`, `.py`, `.rs`, `.go`, `.c`, `.cpp`, and `.java` files on save, evicting stale nodes, rebuilding cross-boundary call edges, and syncing to `KuzuDB`.
- **Blast-Radius &amp; Uncertainty HUD:** Displays each function's composite risk score, $[low, high]$ confidence interval, PageRank centrality, Git co-change partners, transitive loop depth, and surviving mutants right inside VS Code.
- **Zero-Config Binary Discovery:** Connects to your workspace binary or downloads pre-compiled platform releases automatically.

---

## Universal AI Integrations (9 Clients &amp; 10 MCP Tools)

`chronos-mcp` speaks standard **Model Context Protocol (MCP v2024-11-05)** over stdio JSON-RPC 2.0 and exposes **10 specialized tools**:

| Tool Name | Category | Description |
| :--- | :--- | :--- |
| `chronos_analyze_file` | Ingestion | Incrementally parse source file (7 language families), evict stale symbols, extract AST &amp; HTTP routes, and persist to KuzuDB. |
| `chronos_blast_radius` | Risk &amp; Bounds | Compute PageRank blast-radius, Git temporal co-change partners, risk score, `confidence_interval`, and `residual_unknowns`. |
| `chronos_mutation_gate` | Verification | Run isolated parallel &amp; Super-Mutant mutation testing on covering tests before accepting edits. |
| `chronos_query_hotspots` | Telemetry | Identify runtime telemetry bottlenecks, PageRank hubs, and high-complexity functions. |
| `chronos_contract_status` | Contracts | Verify public signature locks and breaking changes across concurrent agent sessions. |
| `chronos_query_graph` | **Cypher** | Execute read-only OpenCypher queries (`MATCH (n:ChronosNode) ...`) over KuzuDB (`CALLS`, `TESTS`, `CO_CHANGES_WITH`, etc.). |
| `chronos_search_graph` | **Discovery** | Search symbols via BM25 identifier tokenization (`camelCase`/`snake_case`), regex, and loop/degree filters. |
| `chronos_get_snippet` | **Context** | Read exact `start_line..=end_line` source code with callers, callees, covering tests, and loop metrics. |
| `chronos_get_architecture` | **Topology** | Incrementally index workspace, mine Git co-changes, and discover architectural communities &amp; hidden $O(N^2)$ hotspots. |
| `chronos_hydrate_telemetry` | **v1.4 Telemetry** | Hydrate the 4th Dimension from offline `.cpuprofile`, Speedscope/`py-spy`, Folded Flamegraph, or OTLP trace artifacts. |

| Client | Platform | Configuration Path |
| :--- | :--- | :--- |
| **Antigravity (Google)** | CLI / IDE | `~/.gemini/antigravity/mcp/` |
| **Cursor** | IDE | `.cursor/mcp.json` |
| **Claude Desktop** | Desktop | `%APPDATA%\Claude\claude_desktop_config.json` |
| **Windsurf** | IDE | `.windsurf/mcp.json` |
| **Cline** | VS Code Extension | `settings.json` |
| **OpenAI Codex CLI** | CLI | `~/.codex/config.json` |
| **OpenCode** | CLI | `.opencode.toml` |
| **Continue** | VS Code / JetBrains | `.continue/config.json` |
| **Zed** | Editor | `~/.config/zed/settings.json` |

Generate ready-to-paste configuration snippets for any client:

```bash
chronos mcp --client all
```

---

## Quickstart &amp; Installation

### Option 1: Pre-Compiled Binaries

Download the latest release archive from [GitHub Releases](https://github.com/ptai-eng/chronos-agent/releases):
- **Windows (x64):** `chronos-windows-x64.zip` (`chronos.exe` + `chronos-mcp.exe`)
- **Linux (x64):** `chronos-linux-x64.tar.gz`
- **macOS (Apple Silicon):** `chronos-macos-arm64.tar.gz`
- **VS Code Extension:** `chronos-agent-1.4.0.vsix`

### Option 2: Build from Source

```powershell
# Prerequisites: Rust 1.85+ (2024 Edition), CMake / MSVC Build Tools (for KuzuDB C++ FFI)
git clone https://github.com/ptai-eng/chronos-agent.git
cd chronos-agent
cargo build --release

# Compiled binaries:
# ./target/release/chronos-mcp.exe  (MCP Server)
# ./target/release/chronos.exe      (CLI Harness)
```

---

## CLI Command Suite

```powershell
# 1. Full 4-Pillar Audit (AST + Blast Radius + Contract Check + Mutation Gate)
#    Supports .ts, .tsx, .js, .py, .rs, .go, .c, .cpp, .java
chronos audit src/services/orderService.ts
chronos audit chronos-core/src/graph/schema.rs

# 2. Hydrate 4D Runtime Telemetry from Offline Profiles (v1.4)
#    Auto-detects V8 .cpuprofile, Speedscope/py-spy JSON, Folded Flamegraph, or OTLP Trace JSON
chronos hydrate profile.cpuprofile --json
chronos hydrate flamegraph.folded

# 3. Architectural Community Discovery, Incremental Merkle Sync & O(N^2) Hotspots
chronos arch
chronos arch chronos-core/src

# 4. BM25 Identifier Search & Regex Discovery
chronos search --query "blast radius" --limit 5
chronos search --pattern ".*Parser.*" --min-loop-depth 2

# 5. Verbatim Function Snippet + Callers/Callees/Tests Context
chronos snippet handle_blast_radius

# 6. Read-Only OpenCypher Query against KuzuDB (including CO_CHANGES_WITH & complexity)
chronos query "MATCH (n:ChronosNode) WHERE n.transitive_loop_depth >= 2 RETURN n.name, n.loop_depth, n.transitive_loop_depth, n.linear_scan_in_loop LIMIT 10"
chronos query "MATCH (a:ChronosNode)-[r:CO_CHANGES_WITH]->(b:ChronosNode) RETURN a.name, b.name, r.call_site_line AS confidence_pct"

# 7. 30-Second Executive Glance (includes confidence interval, git co-changes & coverage report)
chronos glance src/services/orderService.ts --symbol processOrdersWithUserInfo

# 8. Monitor Real-Time Contract Broker Signatures
chronos status --watch src/services/

# 9. Git Diff Blast-Radius & CI Gate (full AST span matching + ImpactAnalyzer)
chronos diff                          # Unstaged working tree changes
chronos diff --staged --json --ci     # Machine-readable JSON; exits 1 on HIGH risk
```

---

## Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)

> [!WARNING]
> **Why static heuristic scanners (such as NVIDIA SkillSpector) flag this repository:**
> 
> `CHRONOS-AGENT` is a system-level code intelligence and runtime profiling harness. Because it intentionally interfaces with privileged debugging and kernel telemetry APIs, static keyword scanners report false-positive risk alerts:
>
> 1. **eBPF Kernel Tracepoints (`chronos-core/src/telemetry/ebpf_adapter.rs`):** Attaches Linux kernel/uprobe tracepoints for low-overhead latency profiling.
> 2. **Chrome DevTools Protocol (`chronos-core/src/telemetry/cdp_client.rs`):** Connects to local V8 inspector WebSockets to sample live CPU and heap allocations.
> 3. **Subprocess Mutation Runner (`chronos-core/src/mutation/runner.rs`):** Spawns isolated test-runner subprocesses inside temporary sandboxes with timeout watchdogs.
> 4. **MCP Client Config Generator (`chronos-cli/src/commands/mcp_config.rs`):** Emits configuration templates for `.cursor/`, `.claude/`, and `.gemini/` directories.
>
> Every MCP tool enforces canonical workspace path validation (`validate_path_within_workspace`), read-only Cypher guards (`validate_readonly_cypher`), parameterized KuzuDB writes, and `_meta.provenance.untrusted_content` tagging.

---

## License

Distributed under the **MIT License**. See `LICENSE` for details.
