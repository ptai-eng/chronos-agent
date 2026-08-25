//! # Standalone HTML/SVG Dashboard Generator
//!
//! Generates a zero-dependency, standalone HTML+SVG dashboard for blast-radius visualization.
//! Activated only when risk level is HIGH or CRITICAL.
//!
//! ## Design: Editorial Dark-tech (Ratified Spec)
//! - Dark background (#0d0d0d) with high-contrast risk colors
//! - Interactive: hover to expand nodes, click to view details
//! - Zero external dependencies — single HTML file, open anywhere

use crate::blast_radius::impact_matrix::{BlastRadiusReport, RiskZone};

/// Generate a standalone HTML dashboard for blast-radius visualization
pub fn generate_html_dashboard(report: &BlastRadiusReport) -> String {
    let risk_color = match report.overall_risk {
        RiskZone::Critical => "#ff3b30",
        RiskZone::High => "#ff9500",
        RiskZone::Medium => "#ffcc00",
        RiskZone::Safe => "#34c759",
    };
    let risk_label = report.overall_risk.label();
    let risk_emoji = report.overall_risk.emoji();

    let changed_nodes_html = report.changed_nodes.iter()
        .map(|n| format!("<code class=\"changed\">{}</code>", html_escape(n)))
        .collect::<Vec<_>>()
        .join(", ");

    let affected_list_html = report.affected_nodes.iter()
        .take(50)
        .map(|n| {
            let zone_color = match n.risk_zone {
                RiskZone::Critical => "#ff3b30",
                RiskZone::High => "#ff9500",
                RiskZone::Medium => "#ffcc00",
                RiskZone::Safe => "#34c759",
            };
            format!(
                r#"<tr class="affected-row">
                    <td><span class="risk-dot" style="background:{zone_color}"></span>{}</td>
                    <td>{}</td>
                    <td class="line-num">{}</td>
                    <td class="reason">{}</td>
                    <td><div class="score-bar" style="width:{:.0}%"></div>{:.0}%</td>
                </tr>"#,
                html_escape(&n.node_name),
                html_escape(&n.file),
                n.line,
                html_escape(&n.reason),
                n.impact_score * 100.0,
                n.impact_score * 100.0,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let routes_html = if report.affected_routes.is_empty() {
        "<p class=\"none\">No API routes affected.</p>".to_string()
    } else {
        report.affected_routes.iter()
            .map(|r| format!("<span class=\"route-badge\">{}</span>", html_escape(r)))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let hot_path_indicator = if report.hot_path_affected {
        r#"<div class="alert alert-hot">🔥 HOT PATH AFFECTED — Performance regression risk</div>"#
    } else {
        ""
    };

    let contract_indicator = if report.contract_breaking {
        r#"<div class="alert alert-contract">⚠️ CONTRACT BREAKING — Update all callers</div>"#
    } else {
        ""
    };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>CHRONOS Blast-Radius Report — {risk_label}</title>
<style>
  :root {{
    --bg: #0d0d0d;
    --surface: #1a1a1a;
    --border: #2a2a2a;
    --text: #e8e8e8;
    --muted: #888;
    --risk: {risk_color};
    --critical: #ff3b30;
    --high: #ff9500;
    --medium: #ffcc00;
    --safe: #34c759;
    --font: 'SF Mono', 'JetBrains Mono', 'Consolas', monospace;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    background: var(--bg);
    color: var(--text);
    font-family: var(--font);
    font-size: 13px;
    padding: 24px;
    min-height: 100vh;
  }}
  header {{
    border-left: 4px solid var(--risk);
    padding-left: 16px;
    margin-bottom: 32px;
  }}
  header .badge {{
    display: inline-block;
    background: var(--risk);
    color: #000;
    font-weight: 700;
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 3px;
    letter-spacing: 0.1em;
    margin-bottom: 8px;
  }}
  header h1 {{
    font-size: 22px;
    font-weight: 800;
    color: var(--text);
    margin-bottom: 4px;
  }}
  header .subtitle {{
    color: var(--muted);
    font-size: 12px;
  }}
  .metrics-grid {{
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 12px;
    margin-bottom: 32px;
  }}
  .metric-card {{
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 16px;
    text-align: center;
  }}
  .metric-card .value {{
    font-size: 32px;
    font-weight: 800;
    color: var(--risk);
    line-height: 1;
  }}
  .metric-card .label {{
    color: var(--muted);
    font-size: 11px;
    margin-top: 4px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }}
  .alert {{
    padding: 10px 16px;
    border-radius: 4px;
    margin-bottom: 12px;
    font-weight: 600;
    font-size: 12px;
  }}
  .alert-hot {{ background: #3a1f00; border: 1px solid var(--high); color: var(--high); }}
  .alert-contract {{ background: #3a0000; border: 1px solid var(--critical); color: var(--critical); }}
  section {{
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    margin-bottom: 20px;
    overflow: hidden;
  }}
  section h2 {{
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--muted);
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    background: #111;
  }}
  section .content {{ padding: 16px; }}
  code.changed {{
    background: #1a2a1a;
    color: var(--safe);
    padding: 2px 6px;
    border-radius: 3px;
    border: 1px solid #2a4a2a;
    font-size: 12px;
  }}
  table {{
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }}
  th {{
    text-align: left;
    color: var(--muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 8px;
    border-bottom: 1px solid var(--border);
  }}
  td {{ padding: 8px; border-bottom: 1px solid var(--border); }}
  .affected-row:hover {{ background: #222; }}
  .risk-dot {{
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    margin-right: 8px;
  }}
  .line-num {{ color: var(--muted); font-size: 11px; }}
  .reason {{ color: var(--muted); font-style: italic; font-size: 11px; }}
  .score-bar {{
    display: inline-block;
    height: 4px;
    background: var(--risk);
    border-radius: 2px;
    margin-right: 6px;
    vertical-align: middle;
    max-width: 60px;
  }}
  .route-badge {{
    display: inline-block;
    background: #111a2a;
    border: 1px solid #1a3a5a;
    color: #5ab3ff;
    padding: 4px 10px;
    border-radius: 4px;
    margin: 4px;
    font-size: 12px;
  }}
  .none {{ color: var(--muted); font-style: italic; padding: 8px 0; }}
  footer {{
    margin-top: 32px;
    text-align: center;
    color: var(--muted);
    font-size: 11px;
  }}
  footer .logo {{ color: var(--risk); font-weight: 800; }}
</style>
</head>
<body>
<header>
  <div class="badge">{risk_emoji} CHRONOS BLAST-RADIUS — {risk_label}</div>
  <h1>Impact Analysis Report</h1>
  <p class="subtitle">Changed: {changed_nodes_html} | Score: {impact_pct:.0}%</p>
</header>

{hot_path_indicator}
{contract_indicator}

<div class="metrics-grid">
  <div class="metric-card">
    <div class="value">{direct_callers}</div>
    <div class="label">Direct Callers</div>
  </div>
  <div class="metric-card">
    <div class="value">{transitive_callers}</div>
    <div class="label">Transitive Impact</div>
  </div>
  <div class="metric-card">
    <div class="value">{total_affected}</div>
    <div class="label">Total Nodes</div>
  </div>
  <div class="metric-card">
    <div class="value" style="color: {risk_color}">{impact_pct:.0}%</div>
    <div class="label">Impact Score</div>
  </div>
</div>

<section>
  <h2>Affected Nodes — Sorted by Risk</h2>
  <div class="content">
    <table>
      <thead>
        <tr>
          <th>Node</th>
          <th>File</th>
          <th>Line</th>
          <th>Reason</th>
          <th>Impact</th>
        </tr>
      </thead>
      <tbody>
        {affected_list_html}
      </tbody>
    </table>
  </div>
</section>

<section>
  <h2>API Endpoints Affected</h2>
  <div class="content">
    {routes_html}
  </div>
</section>

<footer>
  <span class="logo">CHRONOS-AGENT</span> — 4D Knowledge Graph + Runtime Telemetry + Mutation Arena
</footer>
</body>
</html>"#,
        risk_color = risk_color,
        risk_label = risk_label,
        risk_emoji = risk_emoji,
        changed_nodes_html = changed_nodes_html,
        impact_pct = report.impact_score * 100.0,
        hot_path_indicator = hot_path_indicator,
        contract_indicator = contract_indicator,
        direct_callers = report.direct_callers_count,
        transitive_callers = report.transitive_callers_count,
        total_affected = report.affected_nodes.len(),
        affected_list_html = affected_list_html,
        routes_html = routes_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blast_radius::impact_matrix::{AffectedNode, BlastRadiusReport, RiskZone};

    #[test]
    fn test_html_generation_valid() {
        let report = BlastRadiusReport {
            changed_nodes: vec!["getUser".to_string()],
            overall_risk: RiskZone::Critical,
            impact_score: 0.85,
            affected_nodes: vec![
                AffectedNode {
                    node_name: "handleRequest".to_string(),
                    file: "src/api.ts".to_string(),
                    line: 25,
                    risk_zone: RiskZone::Critical,
                    reason: "Direct caller".to_string(),
                    impact_score: 0.9,
                }
            ],
            affected_routes: vec!["GET /users/:id".to_string()],
            direct_callers_count: 3,
            transitive_callers_count: 7,
            hot_path_affected: true,
            contract_breaking: true,
        };

        let html = generate_html_dashboard(&report);
        assert!(html.starts_with("<!DOCTYPE html>"), "Should be valid HTML");
        assert!(html.contains("CRITICAL"), "Should show risk level");
        assert!(html.contains("getUser"), "Should show changed node");
        assert!(html.contains("HOT PATH AFFECTED"), "Should show hot path warning");
        assert!(html.contains("CONTRACT BREAKING"), "Should show contract warning");
        assert!(html.len() > 5000, "Dashboard should be substantial HTML");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>alert('xss')</script>"), 
            "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;");
    }
}
