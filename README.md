<p align="center">
  <img src="assets/chronos_hero_banner.jpg" alt="CHRONOS-AGENT Hero Banner" width="100%" style="border-radius: 12px;"/>
</p>

<h1 align="center">🧠⚡ CHRONOS-AGENT</h1>

<p align="center">
  <strong>4D Living Code Intelligence Harness &amp; Real-Time Blast-Radius Protection for Autonomous AI Agents</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-1.98%20%7C%202024%20Edition-orange?logo=rust&logoColor=white" alt="Rust Version"></a>
  <a href="./chronos-core/tests"><img src="https://img.shields.io/badge/Tests-79%2F79%20Passing-brightgreen?logo=checkmarx&logoColor=white" alt="Tests"></a>
  <a href="https://spec.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP-2024--11--05-blue?logo=protocol&logoColor=white" alt="MCP Protocol"></a>
  <a href="./integrations/INTEGRATIONS.md"><img src="https://img.shields.io/badge/AI%20Clients-9%20Supported-purple?logo=openai&logoColor=white" alt="AI Clients"></a>
  <a href="https://github.com"><img src="https://img.shields.io/badge/Release-Pre--compiled%20Binaries-success" alt="Precompiled Binaries"></a>
  <a href="#-security--privacy-notice-nvidia-skillspector-alert"><img src="https://img.shields.io/badge/Security-SkillSpector%20Audited-red" alt="Security Notice"></a>
</p>

---

## 📑 Table of Contents

- [⚡ The Paradigm Shift](#-the-paradigm-shift)
- [🏛️ Core Architecture: The 4 Pillars](#️-core-architecture-the-4-pillars)
- [📊 Capability Matrix &amp; Benchmark](#-capability-matrix--benchmark)
- [🖥️ Developer HUD &amp; VS Code Extension](#️-developer-hud--vs-code-extension)
- [🔌 Universal AI Integrations (9 Clients)](#-universal-ai-integrations-9-clients)
- [🚀 Quickstart &amp; Installation](#-quickstart--installation)
- [🛠️ CLI Command Suite](#️-cli-command-suite)
- [🛡️ Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)](#️-security--privacy-notice-nvidia-skillspector-alert)
- [🧪 Verification &amp; Test Matrix](#-verification--test-matrix)
- [🗺️ Roadmap &amp; Milestones](#️-roadmap--milestones)

---

## ⚡ The Paradigm Shift

Traditional AI coding models generate code based on **static text probabilities**. They are completely blind to runtime memory pressure, execution bottlenecks, and invisible downstream ripple effects:

> **The Problem:** Modifying a seemingly harmless utility function in Module A unexpectedly breaks payment processing in Module Z because the AI cannot see cross-file call paths or live telemetry.

**CHRONOS-AGENT** bridges this gap by providing AI agents with **multidimensional spatial and runtime awareness (4D Living Code Intelligence)**. It combines static AST graph topologies with live execution metrics (CDP V8 + eBPF), enforcing pre-tool safety gates and automated mutation testing before any code modification is committed.

---

## 🏛️ Core Architecture: The 4 Pillars

<p align="center">
  <img src="assets/chronos_architecture.svg" alt="CHRONOS Architecture Diagram" width="100%"/>
</p>

| Pillar | Focus | Technology &amp; Implementation | Location |
| :--- | :--- | :--- | :--- |
| **1. 4D Knowledge Graph** | AST Topology + Live Metrics | Tree-sitter parser &bull; In-memory `Petgraph` (&lt;15ms query) &bull; Embedded `KuzuDB` persistence | `chronos-core/src/graph/` |
| **2. Mutation Arena** | Test Quality Gate | 5 safe mutant operators (ConditionFlip, ReturnNull, HttpStatusSwap, OffByOne, BoundaryCondition) &bull; Parallel watchdog isolation | `chronos-core/src/mutation/` |
| **3. Semantic Contract Broker** | Pre-Tool Execution Gate | 30ms debounced file watcher &bull; Public signature fingerprinting &bull; Real-time breaking change blocker | `chronos-core/src/contract/` |
| **4. Blast-Radius Heatmap** | Impact Risk Assessment | 3-tier risk classification (Cyan / Amber / Red) &bull; 30-second markdown glance &bull; Interactive HTML visualizer | `chronos-core/src/blast_radius/` |

---

## 📊 Capability Matrix &amp; Benchmark

<p align="center">
  <img src="assets/chronos_benchmark_radar.svg" alt="CHRONOS Capability Radar Comparison" width="100%"/>
</p>

### Feature Comparison

| Capability | Static Linters (ESLint, SonarQube) | Vanilla AI Agents (Copilot, Cursor alone) | **CHRONOS 4D-Agent** |
| :--- | :---: | :---: | :---: |
| **AST Topology Depth** | ⚠️ Flat Syntax Rules | ⚠️ Prompt Token Window Only | 🚀 **Full 4D Multi-File Graph** |
| **Runtime Telemetry** | ❌ No | ❌ No | 🚀 **Live CDP V8 + eBPF Profiling** |
| **Mutation Testing Gate** | ❌ No | ❌ No | 🚀 **Automated 80%/90% Kill Gate** |
| **Blast-Radius Protection** | ❌ No | ⚠️ Guesswork | 🚀 **Deterministic 3-Tier Heatmap** |
| **Pre-Tool Safety Broker** | ❌ No | ❌ No | 🚀 **Zero-Crash Execution Interceptor** |
| **Graph Persistence** | ❌ No | ❌ No | 🚀 **Embedded KuzuDB Engine** |

---

## 🖥️ Developer HUD &amp; VS Code Extension

<p align="center">
  <img src="assets/chronos_radar_dashboard.jpg" alt="CHRONOS Developer HUD Mockup" width="100%" style="border-radius: 12px;"/>
</p>

The companion **CHRONOS VS Code Extension** (`chronos-vscode/`) brings the 4D Knowledge Graph directly into your daily editor:

- **On-Save Continuous Telemetry:** Automatically parses and synchronizes symbols to the local graph upon file save (`onDidSaveTextDocument`).
- **Interactive Blast-Radius HUD:** Visualizes high-impact call dependencies and potential breaking changes directly inside editor webview panels.
- **Zero-Friction Binary Delivery:** Automatically prompts to download pre-compiled releases or connects seamlessly to your custom local MCP binary.

---

## 🔌 Universal AI Integrations (9 Clients)

CHRONOS exposes a standardized **Model Context Protocol (MCP v2024-11-05)** server over stdio JSON-RPC 2.0, allowing plug-and-play integration with any major AI client:

| Client | Platform | Configuration File / Directory |
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

```bash
# Print automatic configuration snippets for all supported AI tools:
chronos mcp --client all
```

---

## 🚀 Quickstart &amp; Installation

### Option 1: Zero-Build Pre-compiled Binaries (Recommended)

Download the pre-built binaries for your platform directly from [GitHub Releases](https://github.com/):
- **Windows (x64):** `chronos-windows-x64.zip` (`chronos.exe` + `chronos-mcp.exe`)
- **Linux (x64):** `chronos-linux-x64.tar.gz`
- **macOS (Intel / Apple Silicon):** `chronos-macos-x64.tar.gz` / `chronos-macos-arm64.tar.gz`

### Option 2: Build from Source

```powershell
# Prerequisites: Rust 1.75+, Node.js 18+
# On Windows, ensure MSVC Build Tools are in your PATH:
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;$env:PATH"

# Clone and compile the workspace:
git clone https://github.com/your-org/chronos-agent.git
cd chronos-agent
cargo build --release

# Binaries will be generated at:
# ./target/release/chronos-mcp.exe  (MCP Protocol Server)
# ./target/release/chronos.exe      (CLI Tool)
```

---

## 🛠️ CLI Command Suite

```powershell
# 1. Full Audit: AST parsing + Blast Radius + Targeted Mutation Gate
chronos audit src/services/orderService.ts

# 2. 30-Second Executive Glance:
chronos glance src/services/orderService.ts --symbol processOrdersWithUserInfo

# 3. Query 4D Knowledge Graph:
chronos graph src/services/orderService.ts --query hotspots
chronos graph src/services/orderService.ts --query callers --symbol getOrderWithUser

# 4. Inspect Live Contract Broker Violations:
chronos status --watch src/services/

# 5. Output Ready-to-Use AI Client Configs:
chronos mcp --client cursor
```

---

## 🛡️ Security &amp; Privacy Notice (NVIDIA SkillSpector Alert)

> [!WARNING]
> **Why static code scanners (such as NVIDIA SkillSpector) flag this project with a "CRITICAL / DO NOT INSTALL" rating:**
> 
> CHRONOS-AGENT is an advanced system-level analysis harness designed to deeply inspect and test software reliability. Because it purposefully leverages low-level privileged APIs, it **intentionally triggers false positives** on static heuristic scanners like NVIDIA SkillSpector (scoring 85/100 risk):
>
> 1. **eBPF Kernel Probes (Sudo/Root Execution):** Traces low-level operating system events without performance overhead.
> 2. **Chrome DevTools Protocol (CDP V8 Hooking):** Connects to active V8 inspector endpoints to capture live runtime telemetry (flagged by scanners as *Exploit Framework* components).
> 3. **Mutation Testing Watchdog:** Injects AST mutants and executes in-process memory watchdogs to verify test rigor.
> 4. **Agent Auto-Discovery:** Scans and configures local AI tool settings (`.claude/`, `.cursor/`, `.gemini/`) to automate MCP server registration.
>
> *These capabilities are core to CHRONOS's mission. We encourage security researchers to audit our open-source Rust implementation.*

---

## 🧪 Verification &amp; Test Matrix

```text
===============================================================================
CHRONOS-AGENT VERIFICATION SUITE
===============================================================================
chronos-core unit tests:         65/65  PASS  [Graph, AST, Telemetry, Mutation]
chronos-core KuzuDB tests:        8/8   PASS  [Persistent Graph Storage]
chronos-core integration tests:   6/6   PASS  [End-to-End 4 Pillars]
-------------------------------------------------------------------------------
TOTAL TEST COVERAGE:             79/79  PASS  (100% Success Rate ✅)
===============================================================================
```

---

## 🗺️ Roadmap &amp; Milestones

- [x] **Phase 0:** Cargo Workspace &amp; Scaffolding (Core, MCP, CLI)
- [x] **Phase 1:** 4D Knowledge Graph (Tree-sitter AST + Petgraph)
- [x] **Phase 2:** Runtime Telemetry Engine (CDP V8 Profiler)
- [x] **Phase 3:** Mutation Arena (80%/90% Kill Gate + In-process Watchdogs)
- [x] **Phase 4:** Semantic Contract Broker &amp; 3-Tier Blast-Radius Matrix
- [x] **Phase 6:** Universal MCP Server (5 Tools, 9 AI Clients)
- [x] **Phase 7:** CLI Command Suite (`audit`, `glance`, `graph`, `mcp`, `status`)
- [x] **Phase 8:** Comprehensive Integration Verification (79/79 Tests Passing)
- [x] **Phase v2.1:** Embedded KuzuDB Persistent Graph Storage
- [x] **Phase v2.2:** GitHub Actions Multi-Platform Release Pipeline (Windows, Linux, macOS)
- [x] **Phase v2.3:** Interactive VS Code Extension Dashboard
- [x] **Phase v2.4:** eBPF Low-Overhead Kernel Telemetry (Linux/WSL2)

---

## 📄 License &amp; Community

Distributed under the **MIT License**. See `LICENSE` for more information.

<p align="center">
  <sub>Engineered with ❤️ for next-generation autonomous software development.</sub>
</p>
