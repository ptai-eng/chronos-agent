---
name: chronos-agent
description: >-
  Use this skill whenever analyzing code architecture, checking blast-radius impact before refactoring,
  finding O(N^2) nested-loop bottlenecks, querying the 5D Knowledge Graph via OpenCypher or BM25,
  hydrating runtime telemetry profiles, or running Super-Mutant verification gates in chronos-agent.
---

# CHRONOS-AGENT 5D Code Intelligence Skill (chronos-agent)

This skill connects **Google Antigravity (`agy`)**, **Claude Code**, **OpenAI Codex**, and **OpenCode** to the **CHRONOS 5D Living Code Graph** (`chronos-mcp`).

## Primary High-Centrality Hubs in `chronos-agent`

- **Top PageRank Hubs:** `new` (`0.0211`), `new` (`0.0190`), `new` (`0.0162`), `NodeId` (`0.0134`), `default` (`0.0117`)

## Mandatory 4-Step Verification Workflow (12 MCP Tools)

1. **1-Call Subagent Discovery (`chronos_explore` — Zero Context Bloat):**
   - Call `chronos_explore` (`query`, `file_filter`, `max_symbols`) to get top BM25 symbols, verbatim source code (`start_line..=end_line`), caller/callee topology, user-facing `verification_surfaces`, and `four_angle_diagnostics` (`Reuse`, `Efficiency`, `Simplification`, `Altitude`) in a single `<15ms` call.
   - Or call `chronos_get_architecture`, `chronos_search_graph`, `chronos_get_snippet`, and `chronos_query_graph` (OpenCypher over `CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`, `CO_CHANGES_WITH`).

2. **Pre-Edit 5D Blast-Radius & Structured Findings Gate:**
   - Before modifying any public function, struct, or HTTP route, call `chronos_blast_radius` (`symbol_name`, `contract_breaking: true/false`, `detail: "summary"`).
   - Inspect `verification_surfaces` (real API routes / CLI commands / root entrypoints to exercise), `structured_findings` (line-anchored `failure_scenario` + `CONFIRMED`/`PLAUSIBLE` verdicts), `confidence_interval`, and Git `CO_CHANGES_WITH` partners.

3. **Semantic Contract Broker & 4-Angle Review Check:**
   - Call `chronos_analyze_file` (`file_path`, `detail: "complete"`) to inspect 4-Angle Code Review diagnostics (`Reuse` duplicate candidates, `Efficiency` loop scans, `Simplification` nesting depth, `Altitude` hub centrality) and `chronos_contract_status` to drain breaking signature alerts.

4. **Super-Mutant Verification Gate:**
   - Before completing a task on core business logic, run `chronos_mutation_gate` (`file_path`) or `chronos diff --staged --json --ci` to verify that unit tests kill synthetic code mutants.
