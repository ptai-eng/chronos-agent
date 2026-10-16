<p align="center">
  <img src="assets/chronos_hero_banner.jpg" alt="CHRONOS-AGENT Tactile Hardware Banner" width="100%" style="border-radius: 12px;"/>
</p>

<h1 align="center">CHRONOS-AGENT</h1>

<p align="center">
  <strong>4D Living Code Graph, PageRank Blast-Radius Analysis &amp; Isolated Mutation Gating for AI Coding Agents</strong>
</p>

<p align="center">
  <a href="https://github.com/ptai-eng/chronos-agent/releases/tag/v1.2.0"><img src="https://img.shields.io/badge/Version-1.2.0-111827?style=flat-square" alt="Version 1.2.0"></a>
  <a href="https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent"><img src="https://img.shields.io/badge/VS%20Code%20Marketplace-CHRONOS%20Agent-007ACC?style=flat-square&logo=visualstudiocode&logoColor=white" alt="VS Code Marketplace"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-2024%20Edition-EB6C36?style=flat-square&logo=rust&logoColor=white" alt="Rust Version"></a>
  <a href="./chronos-core/tests"><img src="https://img.shields.io/badge/Tests-94%2F94%20Passing-1A7F5A?style=flat-square" alt="Tests 94/94 Passing"></a>
  <a href="https://spec.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP-2024--11--05-2E5AA8?style=flat-square" alt="MCP Protocol"></a>
  <a href="./integrations/INTEGRATIONS.md"><img src="https://img.shields.io/badge/AI%20Clients-9%20Supported-4B5563?style=flat-square" alt="AI Clients"></a>
  <a href="#-security--privacy-notice-nvidia-skillspector-alert"><img src="https://img.shields.io/badge/Security-SkillSpector%20Audited-B42318?style=flat-square" alt="Security Notice"></a>
</p>

---

## Table of Contents

- [What's New in v1.2.0](#whats-new-in-v120)
- [Why CHRONOS Exists](#why-chronos-exists)
- [Core Architecture: The 4 Pillars](#core-architecture-the-4-pillars)
- [Capability Matrix &amp; Benchmark](#capability-matrix--benchmark)
- [Developer HUD &amp; VS Code Extension](#developer-hud--vs-code-extension)
- [Universal AI Integrations (9 Clients)](#universal-ai-integrations-9-clients)
- [Quickstart &amp; Installation](#quickstart--installation)
- [CLI Command Suite](#cli-command-suite)
- [Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)](#security--privacy-notice-nvidia-skillspector-alert)

---

## What's New in v1.2.0

`v1.2.0` resolves every architectural limitation from `v1.1.0` and upgrades the graph, MCP server, and CLI to strict metrological and provenance contracts:

| Subsystem | v1.2.0 Upgrade |
| :--- | :--- |
| **Multi-Language Call &amp; Test Edges** | Full caller-to-callee (`CALLS`) and test-to-function (`TESTS`) edge extraction across **TypeScript/JS**, **Python** (`pytest` / `unittest`), and **Rust** (including calls inside `assert!` / `assert_eq!` macro `token_tree` blocks). |
| **Full KuzuDB Edge Persistence** | `PersistentGraph` now persists and reloads all four relationship tables (`CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`) alongside `CodeSymbol` nodes via parameterized Cypher queries. |
| **PageRank &amp; Uncertainty Propagation** | `ImpactAnalyzer` combines 20-iteration damped PageRank ($\alpha = 0.85$), structural blast radius, test coverage deficits, and live telemetry into a single risk score with explicit $[low, high]$ confidence intervals (`confidence_interval`), `coverage` completeness (`complete` \| `partial`), and `residual_unknowns`. |
| **Real `chronos diff --ci` Risk Gate** | `chronos diff` parses modified files with `LanguageDispatcher`, matches Git diff hunks against full AST function spans (`start_line..=end_line`), computes true `ImpactReport` risk levels (`LOW` / `MEDIUM` / `HIGH`), and exits non-zero under `--ci` when high-risk changes are detected. |
| **Zero-Copy Dependency Sandboxes** | `MutantSandbox` automatically links `node_modules`, `.venv`, and `venv` into each isolated worker directory via symbolic links or Windows Directory Junctions (`mklink /J`), eliminating dependency resolution failures during parallel mutation runs and detaching junctions cleanly before cleanup. |
| **MCP Provenance &amp; Progressive Detail** | All 5 MCP tools attach `_meta.provenance.untrusted_content = true` when returning identifiers parsed from repository source files and support `detail: "summary" \| "complete"` to protect LLM context windows. |

---

## Why CHRONOS Exists

AI coding agents read source files as flat text buffers. When an agent edits a utility function in `Module A`, it cannot see whether that function sits on a high-PageRank call path, lacks semantic assertions in its test suite, or runs inside a hot CPU loop in production.

**CHRONOS-AGENT** gives autonomous coding agents a live structural and runtime harness:

1. **Structural Topology + PageRank:** Builds a multi-language AST graph (`Petgraph` in-memory + `KuzuDB` on disk) with O(N) call-edge resolution and damped PageRank centrality.
2. **Runtime Telemetry Overlay:** Maps Chrome DevTools Protocol (CDP V8) CPU/memory profiles and Linux/WSL2 eBPF tracepoints directly onto AST function nodes.
3. **Graph-Guided Mutation Arena:** Instead of running an entire test suite for 20 minutes, CHRONOS queries `TESTS` edges to execute only the specific tests covering the modified symbol inside isolated parallel sandboxes.
4. **Explicit Epistemic Contracts:** Every blast-radius report discloses its observation coverage (`complete` vs. `partial`), uncertainty interval (`confidence_interval`), and unresolved dynamic call sites (`residual_unknowns`)—never conflating unobserved edges with zero risk.

---

## Core Architecture: The 4 Pillars

<p align="center">
  <img src="assets/chronos_architecture.svg" alt="CHRONOS-AGENT v1.2 Architecture Schematic" width="100%"/>
</p>

| Pillar | Responsibility | Implementation | Crate Path |
| :--- | :--- | :--- | :--- |
| **1. 4D Knowledge Graph** | Multi-language AST topology, PageRank centrality, and live telemetry state | Tree-sitter (`TS` / `Py` / `Rust`) &bull; `Petgraph` (`<15ms` traversal) &bull; Embedded `KuzuDB` node + edge persistence | `chronos-core/src/graph/` |
| **2. Mutation Arena** | Semantic test verification before code acceptance | 5 deterministic operators (`ConditionFlip`, `ReturnNull`, `HttpStatusSwap`, `OffByOne`, `BoundaryCondition`) &bull; Parallel temp-dir sandboxes with zero-copy `node_modules`/`.venv` junctions | `chronos-core/src/mutation/` |
| **3. Semantic Contract Broker** | Real-time public signature lock across concurrent agents | 300ms debounced `notify` file watcher &bull; Signature fingerprinting &bull; Breaking-change interception before commit | `chronos-core/src/contract/` |
| **4. Blast-Radius Engine** | Risk scoring with uncertainty bounds | 3-tier classification (`LOW` / `MEDIUM` / `HIGH`) &bull; `CoverageReport` &amp; `confidence_interval` &bull; Markdown glance &amp; HTML visualizer | `chronos-core/src/blast_radius/` |

---

## Capability Matrix &amp; Benchmark

<p align="center">
  <img src="assets/chronos_benchmark_radar.svg" alt="CHRONOS-AGENT v1.2 Capability Radar Benchmark" width="100%"/>
</p>

### Comparison Across Tool Classes

| Capability | Static Linters (`ESLint`, `SonarQube`) | Vanilla Coding Agents (`Copilot`, `Cursor` alone) | Static Graph MCPs (`GitNexus`, `codebase-memory`) | **CHRONOS-AGENT v1.2** |
| :--- | :---: | :---: | :---: | :---: |
| **Multi-Language Call Graph** | Single-file rules | Token window only | Static AST edges | **AST + Macro `token_tree` + PageRank** |
| **Runtime Telemetry** | No | No | No | **CDP V8 Profiler + Linux eBPF** |
| **Mutation Testing Gate** | No | No | No | **Graph-Guided Parallel Sandboxes (`<1.5s`)** |
| **Uncertainty &amp; Coverage Bounds** | No | No | Unobserved = 0 callers | **`confidence_interval` + `residual_unknowns`** |
| **Pre-Tool Contract Broker** | No | No | No | **Live Signature Diff Interceptor** |
| **Persistent Graph Storage** | No | No | External DB or JSON | **Embedded `KuzuDB` (Nodes + 4 Edge Tables)** |

---

## Developer HUD &amp; VS Code Extension

<p align="center">
  <img src="assets/chronos_radar_dashboard.svg" alt="CHRONOS-AGENT v1.2 Developer HUD & Telemetry View" width="100%"/>
</p>

> [!TIP]
> **Available on Visual Studio Marketplace:**  
> Search **"CHRONOS Agent"** in the VS Code Extensions view (`Ctrl+Shift+X`), or install from the [VS Code Marketplace Listing](https://marketplace.visualstudio.com/items?itemName=ptai-eng.chronos-agent).

The companion extension (`chronos-vscode/`) integrates the `chronos-mcp` engine into your editor:

- **Continuous Workspace Watcher:** Automatically re-indexes modified `.ts`, `.tsx`, `.js`, `.py`, and `.rs` files on save, rebuilding cross-file call edges, updating `ContractBroker` signatures, and syncing changes to `KuzuDB`.
- **Blast-Radius &amp; Uncertainty HUD:** Displays each function's composite risk score, $[low, high]$ confidence interval, PageRank centrality, and surviving mutants right inside VS Code.
- **Zero-Config Binary Discovery:** Connects to your workspace binary or downloads pre-compiled platform releases automatically.

---

## Universal AI Integrations (9 Clients)

`chronos-mcp` speaks standard **Model Context Protocol (MCP v2024-11-05)** over stdio JSON-RPC 2.0:

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
- **VS Code Extension:** `chronos-agent-1.2.0.vsix`

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
#    Supports .ts, .tsx, .js, .py, .rs
chronos audit src/services/orderService.ts
chronos audit app/routes/users.py
chronos audit chronos-core/src/graph/schema.rs

# 2. 30-Second Executive Glance (includes confidence interval & coverage report)
chronos glance src/services/orderService.ts --symbol processOrdersWithUserInfo

# 3. Query the 4D Knowledge Graph (telemetry hotspots or PageRank fallback)
chronos graph src/services/orderService.ts --query hotspots
chronos graph src/services/orderService.ts --query callers --symbol getOrderWithUser

# 4. Monitor Real-Time Contract Broker Signatures
chronos status --watch src/services/

# 5. Git Diff Blast-Radius & CI Gate (full AST span matching + ImpactAnalyzer)
chronos diff                          # Unstaged working tree changes
chronos diff --staged                 # Staged changes (pre-commit hook)
chronos diff --commit a1b2c3d         # Specific commit
chronos diff --range main..feature    # Pull request branch range
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
> Every MCP tool enforces canonical workspace path validation (`validate_path_within_workspace`), parameterized KuzuDB queries, and `_meta.provenance.untrusted_content` tagging.

---

## License

Distributed under the **MIT License**. See `LICENSE` for details.
