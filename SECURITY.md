# Security Policy & Threat Model (`CHRONOS-AGENT`)

## Supported Versions

| Version | Supported | Security Audit Status |
| :--- | :---: | :--- |
| **`1.5.x`** | ✅ Yes | Active (4-Layer Defense-in-Depth + Codex Security Ready) |
| **`1.4.x`** | ✅ Yes | Security fixes backported |
| `< 1.4.0` | ❌ No | Upgrade to `>= 1.5.0` recommended |

---

## Architecture & Threat Model

`CHRONOS-AGENT` operates as a local **Model Context Protocol (MCP)** server (`chronos-mcp`) and CLI (`chronos`) that parses untrusted polyglot repositories, executes OpenCypher queries over an embedded **KuzuDB C++ FFI** graph engine, spawns isolated test-runner subprocesses for **Super-Mutant testing**, and connects to runtime telemetry sources (Chrome DevTools Protocol WebSockets, Linux eBPF tracepoints, and offline profiler artifacts).

To protect autonomous coding agents (such as **OpenAI Codex CLI**, **ChatGPT Codex Cloud**, **Cursor**, **Claude Code**, and **Google Antigravity**) against malicious repositories, `CHRONOS-AGENT` enforces a **4-Layer Defense-in-Depth Architecture**:

### 1. Canonical Workspace Path Containment (`validate_path_within_workspace`)
- Every MCP tool that reads or writes files (`chronos_analyze_file`, `chronos_mutation_gate`, `chronos_get_snippet`, `chronos_hydrate_telemetry`, `chronos_generate_agents_md`) canonicalizes target paths and verifies containment inside `workspace_root`.
- Any path containing parent-directory traversal (`..`), symlink escapes outside the workspace root, or unauthorized absolute paths is rejected before filesystem access occurs.

### 2. Read-Only OpenCypher Validator & Parameterized KuzuDB FFI (`validate_readonly_cypher`)
- User- and agent-supplied Cypher queries in `chronos_query_graph` and `chronos query` pass through a strict lexical and keyword gate (`validate_readonly_cypher` in [`chronos-core/src/graph/persistent.rs`](./chronos-core/src/graph/persistent.rs)).
- Mutating or administrative Cypher clauses (`CREATE`, `MERGE`, `DELETE`, `DETACH`, `SET`, `DROP`, `ALTER`, `COPY`, `LOAD`, `INSTALL`, `CALL`) are blocked prior to C++ FFI execution.
- Internal graph synchronization (`sync_from_graph`, `sync_workspace_incremental`) sanitizes and escapes all string literals and enforces bounded row limits (`max_rows <= 100,000`) to prevent memory exhaustion.

### 3. Ephemeral Sandbox & Watchdog Isolation for Mutation Testing
- The **Super-Mutant Engine** ([`chronos-core/src/mutation/`](./chronos-core/src/mutation/)) never mutates working-tree source files in place.
- All synthetic mutants are written inside an isolated temporary sandbox (`tempfile::TempDir`) and executed under a hard timeout watchdog (`WatchdogConfig`, default `5,000 ms` per mutant id via `CHRONOS_MUTANT_ID`) that terminates runaway infinite-loop mutants and cleans up temporary artifacts on drop.

### 4. Indirect Prompt-Injection Provenance Tagging (`_meta.provenance.untrusted_content`)
- Identifier names, route literals, docstrings, and code snippets extracted from external repositories are untrusted inputs that could contain adversarial prompt-injection payloads targeting LLM agents.
- Every MCP tool response attaches explicit provenance metadata:
  ```json
  "_meta": {
    "provenance": {
      "untrusted_content": true,
      "source": "chronos-5d-graph"
    }
  }
  ```
- Blast-radius and architecture reports explicitly separate observed graph edges from unresolved dynamic calls (`coverage.completeness` and `residual_unknowns`), preventing silent false-negative safety claims.

---

## Why Static Heuristic Scanners Flag System-Level Harnesses

Static keyword scanners (such as NVIDIA SkillSpector) may flag files in `chronos-core/src/telemetry/` or `chronos-core/src/mutation/` because `CHRONOS-AGENT` intentionally implements:
1. **eBPF Kernel/Uprobe Tracepoints (`ebpf_adapter.rs`):** Low-overhead Linux function latency profiling.
2. **Chrome DevTools Protocol WebSockets (`cdp_client.rs`):** Local V8 inspector sampling (`ws://127.0.0.1:9229`).
3. **Subprocess Test Execution (`runner.rs` / `watchdog.rs`):** Sandboxed unit-test execution for mutation kill-score verification.

`CHRONOS-AGENT` makes **zero outbound network requests** to external servers and transmits **zero telemetry** outside your local machine.

---

## Reporting a Vulnerability

If you discover a security vulnerability in `CHRONOS-AGENT`, please report it responsibly:
- **Primary Contact:** Hoang Kim Phat Tai (`phattai09052006@gmail.com` | `24207039@student.hcmus.edu.vn`)
- **GitHub Security Advisories:** Use the [Report a vulnerability](https://github.com/ptai-eng/chronos-agent/security/advisories/new) button on GitHub.

We acknowledge all vulnerability reports within **48 hours** and ship verified security patches with regression tests in the next point release.
