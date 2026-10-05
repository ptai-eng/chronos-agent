//! # Mutation Arena — Main Orchestrator
//!
//! The top-level API for the Mutation Testing Gatekeeper (Pillar 2).
//!
//! ## Quality Gate
//! Mutation Score >= 90% required to pass.
//! Score < 90% → AI is given the surviving mutants and RP prompt to write better tests.

use crate::graph::schema::ChronosGraph;
use crate::mutation::operators::{generate_mutants, MutantKind};
use crate::mutation::selector::{select_covering_tests, SelectionStrategy};
use crate::mutation::watchdog::WatchdogConfig;
use crate::graph::schema::NodeId;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

/// Default mutation score threshold — development mode (80%)
pub const MUTATION_SCORE_THRESHOLD: f32 = 0.80;

/// Strict mode threshold — use for release gates (`--strict` flag, 90%)
pub const MUTATION_SCORE_THRESHOLD_STRICT: f32 = 0.90;

/// Summary report from a mutation testing session
#[derive(Debug, Serialize, Deserialize)]
pub struct MutationReport {
    pub target_file: String,
    pub total_mutants: usize,
    pub killed_mutants: usize,
    pub survived_mutants: usize,
    pub timed_out_mutants: usize,
    pub mutation_score: f32,
    pub passed_quality_gate: bool,
    pub surviving_mutant_details: Vec<SurvivingMutantInfo>,
    pub ai_feedback: Option<String>,
    pub ran_at: DateTime<Utc>,
}

/// Details about a surviving mutant for AI feedback
#[derive(Debug, Serialize, Deserialize)]
pub struct SurvivingMutantInfo {
    pub file: String,
    pub line: usize,
    pub mutation_description: String,
    pub suggested_test_assertion: String,
}

impl MutationReport {
    /// Format for MCP tool response / AI agent context injection
    pub fn format_for_agent(&self) -> String {
        let status = if self.passed_quality_gate { "✅ PASSED" } else { "❌ FAILED" };
        let mut lines = vec![
            format!("🧬 MUTATION TESTING REPORT — {}", self.target_file),
            format!("Quality Gate: {} (threshold: {:.0}%)", status, MUTATION_SCORE_THRESHOLD * 100.0),
            format!(
                "Score: {:.1}% ({}/{} mutants killed)",
                self.mutation_score * 100.0,
                self.killed_mutants,
                self.total_mutants
            ),
        ];

        if !self.passed_quality_gate && !self.surviving_mutant_details.is_empty() {
            lines.push("\n⚠️ SURVIVING MUTANTS (tests that failed to catch these bugs):".to_string());
            for (i, s) in self.surviving_mutant_details.iter().enumerate() {
                lines.push(format!("  {}. Line {}: {}", i + 1, s.line, s.mutation_description));
                lines.push(format!("     💡 Suggested fix: {}", s.suggested_test_assertion));
            }
            lines.push("\n🎯 Action required: Add assertions to kill the above mutants, then re-run.".to_string());
        }

        lines.join("\n")
    }

    /// Generate the AI feedback prompt for rewriting tests
    pub fn generate_ai_feedback_prompt(&self) -> Option<String> {
        if self.passed_quality_gate { return None; }
        
        let surviving_descriptions: Vec<String> = self.surviving_mutant_details.iter()
            .map(|s| format!(
                "- Line {}: {} → Your test did NOT detect this bug. Suggested assertion: {}",
                s.line, s.mutation_description, s.suggested_test_assertion
            ))
            .collect();

        Some(format!(
            "Your test suite achieved only {:.1}% mutation score (threshold: {:.0}%).\n\
            The following mutations went UNDETECTED (bugs your tests missed):\n\
            {}\n\n\
            Please add specific assertions to your test cases to detect these bugs.\n\
            Focus on edge cases, boundary conditions, and error handling paths.",
            self.mutation_score * 100.0,
            MUTATION_SCORE_THRESHOLD * 100.0,
            surviving_descriptions.join("\n")
        ))
    }
}

/// The main Mutation Arena orchestrator
pub struct MutationArena<'g> {
    graph: &'g ChronosGraph,
    watchdog_config: WatchdogConfig,
}

impl<'g> MutationArena<'g> {
    pub fn new(graph: &'g ChronosGraph) -> Self {
        Self {
            graph,
            watchdog_config: WatchdogConfig::default(),
        }
    }

    pub fn with_config(mut self, config: WatchdogConfig) -> Self {
        self.watchdog_config = config;
        self
    }

    /// Run the full mutation testing cycle for a source file.
    ///
    /// 1. Generate mutants from the source file
    /// 2. Select covering tests via 4D graph
    /// 3. Run each mutant against its tests (with timeout protection)
    /// 4. Compute mutation score
    /// 5. Return structured report
    pub async fn run(
        &self,
        source_file: &Path,
        target_node_id: Option<&NodeId>,
        working_dir: &Path,
    ) -> Result<MutationReport> {
        let source = std::fs::read_to_string(source_file)
            .map_err(|e| anyhow::anyhow!("Cannot read source file: {}", e))?;
        
        let file_str = source_file.to_string_lossy().to_string();

        // Step 1: Generate mutants
        let mut mutants = generate_mutants(&source, &file_str);
        info!("Generated {} mutants for {}", mutants.len(), file_str);

        if mutants.is_empty() {
            return Ok(MutationReport {
                target_file: file_str,
                total_mutants: 0,
                killed_mutants: 0,
                survived_mutants: 0,
                timed_out_mutants: 0,
                mutation_score: 1.0, // No mutants = trivially passed
                passed_quality_gate: true,
                surviving_mutant_details: vec![],
                ai_feedback: None,
                ran_at: Utc::now(),
            });
        }

        // Step 2: Select covering tests
        let (test_files, strategy) = if let Some(node_id) = target_node_id {
            let selection = select_covering_tests(self.graph, node_id, 10);
            let files: Vec<String> = selection.selected_tests.iter()
                .map(|t| t.test_file.clone())
                .collect();
            (files, selection.selection_strategy)
        } else {
            (vec![], SelectionStrategy::RunAll)
        };

        let test_file_refs: Vec<&str> = test_files.iter().map(|s| s.as_str()).collect();

        info!(
            "Test selection strategy: {:?}, {} test files",
            strategy,
            test_file_refs.len()
        );

        // Step 3: Run mutants in parallel (sandboxed tempdir isolation)
        crate::mutation::runner::run_mutants_parallel(
            &mut mutants,
            source_file,
            &test_file_refs,
            &self.watchdog_config,
            working_dir,
        ).await?;

        // Step 4: Compute results
        let total = mutants.len();
        let killed = mutants.iter().filter(|m| m.killed).count();
        let timed_out = mutants.iter().filter(|m| m.timed_out).count();
        let survived = total - killed;
        let score = if total > 0 { killed as f32 / total as f32 } else { 1.0 };
        let passed = score >= MUTATION_SCORE_THRESHOLD;

        // Step 5: Collect surviving mutant details for AI feedback
        let surviving_details: Vec<SurvivingMutantInfo> = mutants.iter()
            .filter(|m| !m.killed)
            .map(|m| SurvivingMutantInfo {
                file: m.target_file.clone(),
                line: m.target_line,
                mutation_description: m.kind.description(),
                suggested_test_assertion: suggest_assertion_for_mutant(&m.kind),
            })
            .collect();

        let report = MutationReport {
            target_file: file_str,
            total_mutants: total,
            killed_mutants: killed,
            survived_mutants: survived,
            timed_out_mutants: timed_out,
            mutation_score: score,
            passed_quality_gate: passed,
            ai_feedback: if !passed { 
                Some(format!("Mutation score {:.1}% below threshold {:.0}%", score * 100.0, MUTATION_SCORE_THRESHOLD * 100.0))
            } else { None },
            surviving_mutant_details: surviving_details,
            ran_at: Utc::now(),
        };

        info!(
            "Mutation score: {:.1}% ({}/{}) — Gate: {}",
            score * 100.0, killed, total,
            if passed { "PASSED ✅" } else { "FAILED ❌" }
        );

        Ok(report)
    }
}

/// Generate a suggested test assertion based on the mutant kind
fn suggest_assertion_for_mutant(kind: &MutantKind) -> String {
    match kind {
        MutantKind::ConditionFlip { original, mutated: _ } => {
            format!(
                "Add a boundary test: verify behavior when the condition is exactly at the boundary (e.g., test both sides of `{}`)",
                original
            )
        }
        MutantKind::BoundaryShift { original, mutated } => {
            format!(
                "Add off-by-one tests: test with values at `{}` and `{}` boundary",
                original, mutated
            )
        }
        MutantKind::ReturnNull { function_name } => {
            format!(
                "Add null-check assertion: `expect({0}(...)).not.toBeNull()` or `expect({0}(...)).toBeDefined()`",
                function_name
            )
        }
        MutantKind::ReturnFalsy { function_name } => {
            format!(
                "Assert specific truthy return value: `expect({}).toBeTruthy()` or check exact return value",
                function_name
            )
        }
        MutantKind::ErrorSwallow { original_throw } => {
            format!(
                "Add error handling test: `expect(() => callWithBadInput()).toThrow()` to verify `{}` propagates",
                original_throw
            )
        }
        MutantKind::HttpStatusSwap { original_status, mutated_status } => {
            format!(
                "Add status code assertion: `expect(response.status).toBe({})` (not just {})",
                original_status, mutated_status
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutation_report_format() {
        let report = MutationReport {
            target_file: "src/user.ts".to_string(),
            total_mutants: 10,
            killed_mutants: 7,
            survived_mutants: 3,
            timed_out_mutants: 0,
            mutation_score: 0.70,
            passed_quality_gate: false,
            surviving_mutant_details: vec![
                SurvivingMutantInfo {
                    file: "src/user.ts".to_string(),
                    line: 15,
                    mutation_description: "Condition operator flipped: >= → <".to_string(),
                    suggested_test_assertion: "Add boundary test".to_string(),
                }
            ],
            ai_feedback: Some("Score too low".to_string()),
            ran_at: Utc::now(),
        };

        let formatted = report.format_for_agent();
        assert!(formatted.contains("FAILED"), "Should show FAILED status");
        assert!(formatted.contains("70.0%"), "Should show mutation score");
        assert!(formatted.contains("SURVIVING"), "Should list surviving mutants");

        let prompt = report.generate_ai_feedback_prompt();
        assert!(prompt.is_some(), "Should generate AI feedback prompt");
        let prompt_text = prompt.unwrap();
        assert!(prompt_text.contains("80%"), "Should mention threshold");
    }

    #[test]
    fn test_passing_report() {
        let report = MutationReport {
            target_file: "src/clean.ts".to_string(),
            total_mutants: 5,
            killed_mutants: 5,
            survived_mutants: 0,
            timed_out_mutants: 0,
            mutation_score: 1.0,
            passed_quality_gate: true,
            surviving_mutant_details: vec![],
            ai_feedback: None,
            ran_at: Utc::now(),
        };

        assert!(report.generate_ai_feedback_prompt().is_none(), "No prompt when passing");
        let formatted = report.format_for_agent();
        assert!(formatted.contains("PASSED"), "Should show PASSED status");
    }

    #[test]
    fn test_suggest_assertion_for_condition_flip() {
        let kind = MutantKind::ConditionFlip {
            original: " >= ".to_string(),
            mutated: " < ".to_string(),
        };
        let suggestion = suggest_assertion_for_mutant(&kind);
        assert!(suggestion.contains("boundary"), "Should mention boundary testing");
    }
}
