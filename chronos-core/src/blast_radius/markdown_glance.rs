//! # 30-Second Glance Generator
//!
//! Generates the compact Markdown summary (<=5 lines) for PR comments and CLI output.
//! Only triggers the full HTML dashboard for HIGH/CRITICAL risk.

use crate::blast_radius::impact_matrix::BlastRadiusReport;

/// Generate the 30-second Markdown glance summary
pub fn generate_glance_markdown(report: &BlastRadiusReport) -> String {
    let risk_emoji = report.overall_risk.emoji();
    let risk_label = report.overall_risk.label();
    let score_pct = (report.impact_score * 100.0) as u32;
    let ci_low_pct = (report.confidence_interval.0 * 100.0) as u32;
    let ci_high_pct = (report.confidence_interval.1 * 100.0) as u32;

    let mut lines = vec![
        format!(
            "## {} CHRONOS Blast-Radius: **{}** ({score_pct}% risk, CI: {ci_low_pct}%–{ci_high_pct}%, PageRank: {:.3})",
            risk_emoji, risk_label, report.pagerank_score
        ),
    ];

    // Line 2: Changed functions
    let changed = report.changed_nodes.join("`, `");
    lines.push(format!("**Changed:** `{changed}`"));

    // Line 3: Impact summary
    let hot_indicator = if report.hot_path_affected { " 🔥 Hot-path affected!" } else { "" };
    let contract_indicator = if report.contract_breaking { " ⚠️ Contract breaking!" } else { "" };
    lines.push(format!(
        "**Impact:** {} direct callers | {} transitive callers{}{}",
        report.direct_callers_count,
        report.transitive_callers_count,
        hot_indicator,
        contract_indicator,
    ));

    // Line 4: Coverage & Residual Unknowns
    lines.push(format!(
        "**Coverage:** `{}` (resolved calls: {}, unresolved: {}, telemetry: {})",
        report.coverage.completeness,
        report.coverage.resolved_calls,
        report.coverage.unresolved_calls,
        if report.coverage.telemetry_hydrated { "hydrated" } else { "cold" },
    ));
    if !report.residual_unknowns.is_empty() {
        lines.push(format!("**Residual Unknowns:** {}", report.residual_unknowns.join("; ")));
    }

    // Line 5: Top affected nodes (max 3)
    let top_affected: Vec<String> = report.affected_nodes.iter()
        .take(3)
        .map(|n| format!("`{}` ({})", n.node_name, n.risk_zone.label()))
        .collect();
    if !top_affected.is_empty() {
        lines.push(format!("**Top affected:** {}", top_affected.join(", ")));
    }

    // Line 6: API routes (if any)
    if !report.affected_routes.is_empty() {
        let routes: Vec<String> = report.affected_routes.iter().take(3)
            .map(|r| format!("`{}`", r))
            .collect();
        lines.push(format!("**API endpoints affected:** {}", routes.join(", ")));
    }

    // Dashboard hint for high risk
    if report.needs_visual_dashboard() {
        lines.push("\n> 📊 Full blast-radius dashboard: Run `chronos glance --html` for interactive visualization.".to_string());
    }

    lines.join("\n")
}

/// Generate a compact one-liner for terminal display
pub fn generate_inline_status(report: &BlastRadiusReport) -> String {
    let emoji = report.overall_risk.emoji();
    let label = report.overall_risk.label();
    format!(
        "{emoji} [{label}] {} callers affected (hot-path: {}, contract: {}, coverage: {})",
        report.direct_callers_count + report.transitive_callers_count,
        if report.hot_path_affected { "YES" } else { "no" },
        if report.contract_breaking { "BREAKING" } else { "safe" },
        report.coverage.completeness,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blast_radius::impact_matrix::{AffectedNode, BlastRadiusReport, CoverageReport, RiskZone};

    fn make_report(risk: RiskZone, direct: usize, transitive: usize, hot: bool, contract: bool) -> BlastRadiusReport {
        BlastRadiusReport {
            changed_nodes: vec!["getUser".to_string()],
            overall_risk: risk,
            impact_score: 0.8,
            confidence_interval: (0.65, 0.95),
            pagerank_score: 0.25,
            coverage: CoverageReport::default(),
            residual_unknowns: vec![],
            affected_nodes: (0..direct.min(3))
                .map(|i| AffectedNode {
                    node_name: format!("caller_{}", i),
                    file: "src/api.ts".to_string(),
                    line: i + 1,
                    risk_zone: RiskZone::High,
                    reason: "Direct caller".to_string(),
                    impact_score: 0.7,
                })
                .collect(),
            affected_routes: vec!["GET /users/:id".to_string()],
            direct_callers_count: direct,
            transitive_callers_count: transitive,
            hot_path_affected: hot,
            contract_breaking: contract,
        }
    }

    #[test]
    fn test_critical_report_format() {
        let report = make_report(RiskZone::Critical, 3, 8, true, true);
        let md = generate_glance_markdown(&report);
        
        assert!(md.contains("CRITICAL"), "Should show CRITICAL");
        assert!(md.contains("🔴"), "Should show red emoji");
        assert!(md.contains("Hot-path"), "Should mention hot-path");
        assert!(md.contains("Contract breaking"), "Should mention contract breaking");
        assert!(md.contains("dashboard"), "Should suggest dashboard for critical risk");
    }

    #[test]
    fn test_safe_report_no_dashboard_hint() {
        let report = make_report(RiskZone::Safe, 0, 0, false, false);
        let md = generate_glance_markdown(&report);
        assert!(!md.contains("dashboard"), "Safe reports should not suggest dashboard");
    }

    #[test]
    fn test_inline_status() {
        let report = make_report(RiskZone::High, 5, 3, true, false);
        let status = generate_inline_status(&report);
        assert!(status.contains("[HIGH]"), "Should contain risk level");
        assert!(status.contains("hot-path: YES"), "Should mention hot path");
    }
}
