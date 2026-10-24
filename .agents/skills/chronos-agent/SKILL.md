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

- **Top PageRank Hubs:** `new` (`0.0210`), `new` (`0.0190`), `new` (`0.0162`), `NodeId` (`0.0135`), `default` (`0.0117`)

## Mandatory 4-Step Verification Workflow

1. **Graph Discovery First (Zero Context Bloat):**
   - Call `chronos_get_architecture` to inspect Label-Propagation community clusters, cohesion scores, and $O(N^2)$ loop hotspots.
   - Call `chronos_search_graph` (BM25 + regex + `min_loop_depth`) or `chronos_get_snippet` to read exact function spans (`start_line..=end_line`) with callers, callees, and covering tests.
   - Call `chronos_query_graph` for custom read-only OpenCypher queries over `ChronosNode` (`CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`, `CO_CHANGES_WITH`).

2. **Pre-Edit 5D Blast-Radius Gate:**
   - Before modifying any public function, struct, or HTTP route, call `chronos_blast_radius` (`symbol_name`, `contract_breaking: true/false`, `detail: "summary"`).
   - Inspect the `confidence_interval`, `residual_unknowns`, and Git temporal co-change partners (`CO_CHANGES_WITH`).

3. **Semantic Contract Broker Check:**
   - After modifying function signatures or API endpoints, call `chronos_contract_status` to ensure no downstream callers were broken.

4. **Super-Mutant Verification Gate:**
   - Before completing a task on core business logic, run `chronos_mutation_gate` (`file_path`) or `chronos diff --staged --json --ci` to verify that unit tests kill synthetic code mutants.
