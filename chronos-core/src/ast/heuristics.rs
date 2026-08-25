//! # Heuristics Module
//!
//! Standalone heuristic scoring — used as cold-start fallback when no
//! telemetry is available. Provides static quality signals to AI agents.

use crate::graph::schema::StaticHeuristics;

/// Compute a "refactoring priority" ranking from static heuristics alone.
/// Returns a score 0.0..=1.0 where 1.0 = highest refactoring urgency.
pub fn refactoring_priority(h: &StaticHeuristics) -> f32 {
    let score = h.cognitive_load_score / 100.0;
    score.min(1.0)
}

/// Generate a human-readable cold-start advisory for AI agents.
pub fn cold_start_advisory(h: &StaticHeuristics, function_name: &str) -> String {
    let _priority = refactoring_priority(h);
    
    let mut advice = vec![];
    
    if h.cyclomatic_complexity > 15 {
        advice.push(format!(
            "High cyclomatic complexity ({}) — consider splitting into smaller functions",
            h.cyclomatic_complexity
        ));
    }
    if h.fan_out > 8 {
        advice.push(format!(
            "High fan-out ({}) — this function depends on too many others (violation of Single Responsibility)",
            h.fan_out
        ));
    }
    if h.param_count > 4 {
        advice.push(format!(
            "{} parameters — consider using a parameter object/DTO pattern",
            h.param_count
        ));
    }
    if h.sloc > 50 {
        advice.push(format!(
            "{} source lines — function is too long (target < 30 SLOC)",
            h.sloc
        ));
    }

    if advice.is_empty() {
        format!("✅ `{}` looks clean (cognitive load: {:.0})", function_name, h.cognitive_load_score)
    } else {
        format!(
            "⚠️ `{}` (cognitive load: {:.0}/100):\n{}",
            function_name,
            h.cognitive_load_score,
            advice.iter().map(|a| format!("  • {}", a)).collect::<Vec<_>>().join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_function_advisory() {
        let mut h = StaticHeuristics::default();
        h.cyclomatic_complexity = 2;
        h.fan_out = 2;
        h.param_count = 2;
        h.sloc = 10;
        h.compute_cognitive_load();
        
        let advisory = cold_start_advisory(&h, "cleanFunction");
        assert!(advisory.contains("✅"), "Clean function should get green light");
    }

    #[test]
    fn test_complex_function_advisory() {
        let mut h = StaticHeuristics::default();
        h.cyclomatic_complexity = 20;
        h.fan_out = 12;
        h.param_count = 6;
        h.sloc = 80;
        h.compute_cognitive_load();
        
        let advisory = cold_start_advisory(&h, "godFunction");
        assert!(advisory.contains("⚠️"), "Complex function should get warning");
        assert!(advisory.contains("cyclomatic"), "Should mention cyclomatic complexity");
    }
}
