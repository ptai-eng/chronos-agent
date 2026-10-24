Run a full 5D CHRONOS architectural, 4-angle review, blast-radius, and mutation audit on `$ARGUMENTS`:

1. Call `chronos_explore` on `$ARGUMENTS` to retrieve verbatim source snippets, caller/callee chains, `verification_surfaces` (real API/CLI entrypoints), and 4-Angle Code Review diagnostics (`Reuse`, `Efficiency`, `Simplification`, `Altitude`).
2. Call `chronos_blast_radius` on the target symbol or file in `$ARGUMENTS` (`detail: "complete"`) and report the `RiskZone`, 95% `confidence_interval`, `structured_findings` (`failure_scenario`), `residual_unknowns`, and Git `CO_CHANGES_WITH` partners.
3. Call `chronos_contract_status` to check for any breaking signature drift.
4. If code changes were made, call `chronos_mutation_gate` to verify semantic test coverage and exercise the reported `verification_surfaces`.
