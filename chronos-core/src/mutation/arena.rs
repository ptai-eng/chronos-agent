//! # Mutation Arena — Deep Orchestrator
//!
//! Pillar 2 Quality Gatekeeper:
//! - Default dev gate: 80% (`MUTATION_SCORE_THRESHOLD`)
//! - Strict release gate (`--strict`): 90% (`MUTATION_SCORE_THRESHOLD_STRICT`)
//! - Supports Super-Mutant single-compile execution (`run_super_mutant_parallel`)
//!   and an injectable `TestExecutor` port for fast in-memory unit testing.

use crate::graph::schema::{ChronosGraph, NodeId};
use crate::mutation::operators::{generate_mutants, Mutant, MutantKind};
use crate::mutation::selector::{select_covering_tests, SelectionStrategy};
use crate::mutation::watchdog::{TestExecutor, WatchdogConfig};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Default mutation score threshold — development mode (80%)
pub const MUTATION_SCORE_THRESHOLD: f32 = 0.80;

/// Strict mode threshold — use for release gates (`--strict` flag, 90%)
pub const MUTATION_SCORE_THRESHOLD_STRICT: f32 = 0.90;

fn default_threshold() -> f32 {
    MUTATION_SCORE_THRESHOLD
}

/// Summary report from a mutation testing session
#[derive(Debug, Serialize, Deserialize)]
pub struct MutationReport {
    pub target_file: String,
    pub total_mutants: usize,
    pub killed_mutants: usize,
    pub survived_mutants: usize,
    pub timed_out_mutants: usize,
    pub mutation_score: f32,
    #[serde(default = "default_threshold")]
    pub threshold_used: f32,
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
        let threshold_pct = self.threshold_used * 100.0;
        let mut lines = vec![
            format!("🧬 MUTATION TESTING REPORT — {}", self.target_file),
            format!("Quality Gate: {} (threshold: {:.0}%)", status, threshold_pct),
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
        if self.passed_quality_gate {
            return None;
        }

        let surviving_descriptions: Vec<String> = self
            .surviving_mutant_details
            .iter()
            .map(|s| {
                format!(
                    "- Line {}: {} → Your test did NOT detect this bug. Suggested assertion: {}",
                    s.line, s.mutation_description, s.suggested_test_assertion
                )
            })
            .collect();

        Some(format!(
            "Your test suite achieved only {:.1}% mutation score (threshold: {:.0}%).\n\
            The following mutations went UNDETECTED (bugs your tests missed):\n\
            {}\n\n\
            Please add specific assertions to your test cases to detect these bugs.\n\
            Focus on edge cases, boundary conditions, and error handling paths.",
            self.mutation_score * 100.0,
            self.threshold_used * 100.0,
            surviving_descriptions.join("\n")
        ))
    }
}

/// Deep Mutation Arena orchestrator with `strict` threshold enforcement, Super-Mutant
/// single-compile execution, and an injectable `TestExecutor` seam.
pub struct MutationArena<'g> {
    graph: &'g ChronosGraph,
    watchdog_config: WatchdogConfig,
    threshold: f32,
    use_super_mutant: bool,
    custom_executor: Option<Arc<dyn TestExecutor>>,
}

impl<'g> MutationArena<'g> {
    pub fn new(graph: &'g ChronosGraph) -> Self {
        Self {
            graph,
            watchdog_config: WatchdogConfig::default(),
            threshold: MUTATION_SCORE_THRESHOLD,
            use_super_mutant: true,
            custom_executor: None,
        }
    }

    pub fn with_config(mut self, config: WatchdogConfig) -> Self {
        self.watchdog_config = config;
        self
    }

    /// Configure strict release threshold (90% when `strict == true`, 80% otherwise).
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.threshold = if strict {
            MUTATION_SCORE_THRESHOLD_STRICT
        } else {
            MUTATION_SCORE_THRESHOLD
        };
        self
    }

    /// Override the numeric mutation threshold directly.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Inject a custom `TestExecutor` adapter (e.g. an in-memory double for unit tests).
    pub fn with_executor(mut self, executor: Arc<dyn TestExecutor>) -> Self {
        self.custom_executor = Some(executor);
        self
    }

    /// Enable or disable single-compile `CHRONOS_MUTANT_ID` Super-Mutant dispatch.
    pub fn with_super_mutant(mut self, enabled: bool) -> Self {
        self.use_super_mutant = enabled;
        self
    }

    /// Run the full mutation testing cycle for a source file.
    pub async fn run(
        &self,
        source_file: &Path,
        target_node_id: Option<&NodeId>,
        working_dir: &Path,
    ) -> Result<MutationReport> {
        let source = std::fs::read_to_string(source_file)
            .map_err(|e| anyhow::anyhow!("Cannot read source file: {}", e))?;

        let file_str = source_file.to_string_lossy().to_string();
        let mut mutants = generate_mutants(&source, &file_str);
        info!("Generated {} mutants for {}", mutants.len(), file_str);

        if mutants.is_empty() {
            return Ok(MutationReport {
                target_file: file_str,
                total_mutants: 0,
                killed_mutants: 0,
                survived_mutants: 0,
                timed_out_mutants: 0,
                mutation_score: 1.0,
                threshold_used: self.threshold,
                passed_quality_gate: true,
                surviving_mutant_details: vec![],
                ai_feedback: None,
                ran_at: Utc::now(),
            });
        }

        let (test_files, strategy) = if let Some(node_id) = target_node_id {
            let selection = select_covering_tests(self.graph, node_id, 10);
            let files: Vec<String> = selection
                .selected_tests
                .iter()
                .map(|t| t.test_file.clone())
                .collect();
            (files, selection.selection_strategy)
        } else {
            (vec![], SelectionStrategy::RunAll)
        };

        let test_file_refs: Vec<&str> = test_files.iter().map(|s| s.as_str()).collect();
        info!(
            "Test selection strategy: {:?}, {} test files, gate={:.0}%",
            strategy,
            test_file_refs.len(),
            self.threshold * 100.0
        );

        if let Some(ref executor) = self.custom_executor {
            for mutant in &mut mutants {
                let res = executor
                    .execute_mutant(
                        mutant.id,
                        Some(&mutant.mutated_source),
                        source_file,
                        &test_file_refs,
                        &self.watchdog_config,
                        working_dir,
                    )
                    .await?;
                if res.timed_out {
                    mutant.timeout();
                } else if res.killed {
                    mutant.kill(format!("exit_code={}", res.exit_code.unwrap_or(-1)));
                }
            }
        } else if self.use_super_mutant && mutants.len() > 1 {
            // Partition: 1 non-overlapping primary mutant per line via single-compile Super-Mutant;
            // any colliding mutants on the same line run in standard per-mutant sandboxes.
            let mut seen_lines = HashSet::new();
            let mut primary_super: Vec<Mutant> = Vec::new();
            let mut stacked: Vec<Mutant> = Vec::new();
            for m in mutants {
                if seen_lines.insert(m.target_line) {
                    primary_super.push(m);
                } else {
                    stacked.push(m);
                }
            }
            if !primary_super.is_empty() {
                crate::mutation::runner::run_super_mutant_parallel(
                    &mut primary_super,
                    source_file,
                    &test_file_refs,
                    &self.watchdog_config,
                    working_dir,
                )
                .await?;
            }
            if !stacked.is_empty() {
                crate::mutation::runner::run_mutants_parallel(
                    &mut stacked,
                    source_file,
                    &test_file_refs,
                    &self.watchdog_config,
                    working_dir,
                )
                .await?;
            }
            mutants = primary_super;
            mutants.extend(stacked);
        } else {
            crate::mutation::runner::run_mutants_parallel(
                &mut mutants,
                source_file,
                &test_file_refs,
                &self.watchdog_config,
                working_dir,
            )
            .await?;
        }

        let total = mutants.len();
        let killed = mutants.iter().filter(|m| m.killed).count();
        let timed_out = mutants.iter().filter(|m| m.timed_out).count();
        let survived = total - killed;
        let score = if total > 0 {
            killed as f32 / total as f32
        } else {
            1.0
        };
        let passed = score >= self.threshold;

        let surviving_details: Vec<SurvivingMutantInfo> = mutants
            .iter()
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
            threshold_used: self.threshold,
            passed_quality_gate: passed,
            ai_feedback: if !passed {
                Some(format!(
                    "Mutation score {:.1}% below threshold {:.0}%",
                    score * 100.0,
                    self.threshold * 100.0
                ))
            } else {
                None
            },
            surviving_mutant_details: surviving_details,
            ran_at: Utc::now(),
        };

        info!(
            "Mutation score: {:.1}% ({}/{}) — Gate: {} ({:.0}%)",
            score * 100.0,
            killed,
            total,
            if passed { "PASSED ✅" } else { "FAILED ❌" },
            self.threshold * 100.0
        );

        Ok(report)
    }
}

fn suggest_assertion_for_mutant(kind: &MutantKind) -> String {
    match kind {
        MutantKind::ConditionFlip {
            original,
            mutated: _,
        } => {
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
        MutantKind::HttpStatusSwap {
            original_status,
            mutated_status,
        } => {
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
    use crate::mutation::watchdog::MutantTestResult;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FixedRatioExecutor {
        counter: AtomicUsize,
        survive_first_n: usize,
    }

    #[async_trait::async_trait]
    impl TestExecutor for FixedRatioExecutor {
        async fn execute_mutant(
            &self,
            mutant_id: u64,
            _mutant_source: Option<&str>,
            _original_file: &Path,
            _test_files: &[&str],
            _config: &WatchdogConfig,
            _working_dir: &Path,
        ) -> Result<MutantTestResult> {
            let idx = self.counter.fetch_add(1, Ordering::SeqCst);
            let survive = idx < self.survive_first_n;
            Ok(MutantTestResult {
                mutant_id,
                killed: !survive,
                timed_out: false,
                exit_code: Some(if survive { 0 } else { 1 }),
                duration_ms: 1,
                output: String::new(),
            })
        }
    }

    #[tokio::test]
    async fn test_strict_90_percent_gate_rejects_scores_between_80_and_90() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("math.ts");
        let src = r#"
export function a(x: number) { if (x > 0) return x + 1; return 0; }
export function b(x: number) { if (x > 0) return x + 1; return 0; }
export function c(x: number) { if (x > 0) return x + 1; return 0; }
export function d(x: number) { if (x > 0) return x + 1; return 0; }
export function e(x: number) { if (x > 0) return x + 1; return 0; }
export function f(x: number) { if (x > 0) return x + 1; return 0; }
"#;
        std::fs::write(&src_path, src).unwrap();

        let total = generate_mutants(src, &src_path.to_string_lossy()).len();
        assert!(total >= 7, "expected >= 7 mutants, got {total}");
        // Choose surviving mutant count so kill ratio lands cleanly inside [0.80, 0.90)
        let survive_n = (1..total)
            .find(|&s| {
                let score = (total - s) as f32 / total as f32;
                (0.80..0.90).contains(&score)
            })
            .expect("should find surviving count producing 80%-90% score");

        let graph = ChronosGraph::new();
        let exec_dev = Arc::new(FixedRatioExecutor {
            counter: AtomicUsize::new(0),
            survive_first_n: survive_n,
        });
        let dev_report = MutationArena::new(&graph)
            .with_strict(false)
            .with_executor(exec_dev)
            .run(&src_path, None, dir.path())
            .await
            .unwrap();
        assert!(
            dev_report.mutation_score >= 0.80 && dev_report.mutation_score < 0.90,
            "expected score in [0.80, 0.90), got {}",
            dev_report.mutation_score
        );
        assert!(
            dev_report.passed_quality_gate,
            "80%-90% score must pass 80% dev gate"
        );

        let exec_strict = Arc::new(FixedRatioExecutor {
            counter: AtomicUsize::new(0),
            survive_first_n: survive_n,
        });
        let strict_report = MutationArena::new(&graph)
            .with_strict(true)
            .with_executor(exec_strict)
            .run(&src_path, None, dir.path())
            .await
            .unwrap();
        assert!(
            !strict_report.passed_quality_gate,
            "BUG-01 fix: 80%-90% score must FAIL 90% strict gate (score={})",
            strict_report.mutation_score
        );
        assert!(strict_report.format_for_agent().contains("threshold: 90%"));
        assert!(strict_report
            .generate_ai_feedback_prompt()
            .unwrap()
            .contains("threshold: 90%"));
    }

    #[test]
    fn test_mutation_report_format() {
        let report = MutationReport {
            target_file: "src/user.ts".to_string(),
            total_mutants: 10,
            killed_mutants: 7,
            survived_mutants: 3,
            timed_out_mutants: 0,
            mutation_score: 0.70,
            threshold_used: MUTATION_SCORE_THRESHOLD,
            passed_quality_gate: false,
            surviving_mutant_details: vec![SurvivingMutantInfo {
                file: "src/user.ts".to_string(),
                line: 15,
                mutation_description: "Condition operator flipped: >= → <".to_string(),
                suggested_test_assertion: "Add boundary test".to_string(),
            }],
            ai_feedback: Some("Score too low".to_string()),
            ran_at: Utc::now(),
        };

        let formatted = report.format_for_agent();
        assert!(formatted.contains("FAILED"), "Should show FAILED status");
        assert!(formatted.contains("70.0%"), "Should show mutation score");
        assert!(
            formatted.contains("SURVIVING"),
            "Should list surviving mutants"
        );

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
            threshold_used: MUTATION_SCORE_THRESHOLD,
            passed_quality_gate: true,
            surviving_mutant_details: vec![],
            ai_feedback: None,
            ran_at: Utc::now(),
        };

        assert!(
            report.generate_ai_feedback_prompt().is_none(),
            "No prompt when passing"
        );
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
        assert!(
            suggestion.contains("boundary"),
            "Should mention boundary testing"
        );
    }
}
