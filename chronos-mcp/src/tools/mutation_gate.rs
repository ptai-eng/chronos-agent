//! # Tool: chronos_mutation_gate
//!
//! Run targeted mutation testing on a file and return the mutation score.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::mutation::{
    arena::{MutationArena, MUTATION_SCORE_THRESHOLD, MUTATION_SCORE_THRESHOLD_STRICT},
    watchdog::WatchdogConfig,
};
use serde_json::Value;
use std::path::PathBuf;
use tracing::info;

pub async fn handle_mutation_gate(state: SharedState, params: Option<Value>) -> CallToolResult {
    let file_path_str = match params.as_ref()
        .and_then(|p| p.get("file_path"))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return CallToolResult::error("Missing required parameter: file_path"),
    };

    let test_command = params.as_ref()
        .and_then(|p| p.get("test_command"))
        .and_then(|v| v.as_str())
        .unwrap_or("npx jest --passWithNoTests")
        .to_string();

    let strict_mode = params.as_ref()
        .and_then(|p| p.get("strict"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let threshold = if strict_mode {
        MUTATION_SCORE_THRESHOLD_STRICT
    } else {
        MUTATION_SCORE_THRESHOLD
    };

    let (abs_path, workspace_root) = {
        let st = state.read().await;
        let p = PathBuf::from(&file_path_str);
        let abs = if p.is_absolute() { p } else { st.workspace_root.join(&p) };
        (abs, st.workspace_root.clone())
    };

    if !abs_path.exists() {
        return CallToolResult::error(format!(
            "File not found: {}\nMake sure the path is correct.",
            abs_path.display()
        ));
    }

    info!(
        "🧬 Mutation gate: {} ({} mode, {:.0}% threshold)",
        abs_path.display(),
        if strict_mode { "STRICT" } else { "dev" },
        threshold * 100.0
    );

    // Build arena using graph reference
    // Parse test command into runner + args
    let test_parts: Vec<String> = test_command
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let (test_runner, test_args) = if test_parts.is_empty() {
        ("npx".to_string(), vec!["jest".to_string(), "--passWithNoTests".to_string()])
    } else {
        (test_parts[0].clone(), test_parts[1..].to_vec())
    };

    let watchdog_config = WatchdogConfig {
        timeout_ms: 5000,
        max_concurrency: 4,
        test_runner,
        test_args,
    };

    let report = {
        let st = state.read().await;
        let arena = MutationArena::new(&st.graph).with_config(watchdog_config);
        arena.run(&abs_path, None, &workspace_root).await
    };

    let report = match report {
        Ok(r) => r,
        Err(e) => return CallToolResult::error(format!("Mutation arena error: {}", e)),
    };

    let gate_icon = if report.passed_quality_gate { "✅" } else { "❌" };
    let mode_label = if strict_mode {
        "STRICT (Release Gate — 90%)"
    } else {
        "Dev Gate (80%)"
    };

    let mut output = format!(
        r#"{gate_icon} **CHRONOS Mutation Gate — {}**

**File:** `{}`
**Mode:** {}
**Threshold:** {:.0}%

## Results
| Metric | Value |
|:-------|:------|
| Total mutants | {} |
| Killed (caught) | {} |
| Survived (missed) | {} |
| Timed out | {} |
| **Mutation score** | **{:.1}%** |
| **Gate status** | **{}** |

{}"#,
        if report.passed_quality_gate { "PASS" } else { "FAIL" },
        file_path_str,
        mode_label,
        threshold * 100.0,
        report.total_mutants,
        report.killed_mutants,
        report.survived_mutants,
        report.timed_out_mutants,
        report.mutation_score * 100.0,
        if report.passed_quality_gate {
            "✅ PASSED — Test suite quality is sufficient"
        } else {
            "❌ FAILED — Add more specific assertions"
        },
        report.format_for_agent(),
    );

    if !report.passed_quality_gate {
        if let Some(prompt) = report.generate_ai_feedback_prompt() {
            output.push_str(&format!(
                "\n## 🤖 AI Feedback\n\n```\n{}\n```\n",
                prompt
            ));
        }
    }

    CallToolResult::text(output)
}
