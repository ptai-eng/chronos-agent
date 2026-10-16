//! # Tool: chronos_query_hotspots

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::graph::query::{GraphQueryEngine, RiskLevel};
use serde_json::Value;

pub async fn handle_query_hotspots(state: SharedState, params: Option<Value>) -> CallToolResult {
    let top_n = params.as_ref()
        .and_then(|p| p.get("top_n"))
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    let filter = params.as_ref()
        .and_then(|p| p.get("filter"))
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .to_string();

    let st = state.read().await;
    let report = st.graph.status_report();

    if report.total_nodes == 0 {
        return CallToolResult::text(
            "⚠️ **4D Graph is empty.**\n\nCall `chronos_analyze_file` first to populate the graph.",
        );
    }

    let engine = GraphQueryEngine::new(&st.graph);

    let mut output = format!(
        "## 🔥 CHRONOS Hotspot Report\n\n\
         **Graph:** {} nodes | {} functions | {} test cases\n\
         **Hydrated:** {}/{} | 🔥 Hot paths: {} | ⚠️ Bottlenecks: {}\n\n",
        report.total_nodes,
        report.function_nodes,
        report.test_nodes,
        report.hydrated_nodes,
        report.total_nodes,
        report.hot_paths,
        report.bottlenecks,
    );

    // Bottlenecks section
    if filter == "all" || filter == "bottlenecks" {
        let result = engine.find_bottlenecks();
        if result.details.is_empty() {
            output.push_str("### ⚡ Runtime Bottlenecks\n_No live telemetry data yet. Showing structural PageRank bottlenecks instead:_\n\n");
            let pagerank = st.graph.compute_pagerank(20, 0.85);
            let mut ranked: Vec<_> = st.graph.nodes_by_kind(&chronos_core::graph::schema::NodeKind::Function)
                .into_iter()
                .map(|n| (n, pagerank.get(&n.id).copied().unwrap_or(0.0)))
                .collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            if !ranked.is_empty() {
                output.push_str("| # | Symbol | File | PageRank | Fan-In | Cognitive Load |\n");
                output.push_str("|:--|:-------|:-----|:--------:|:------:|:--------------:|\n");
                for (i, (node, pr)) in ranked.into_iter().take(top_n).enumerate() {
                    output.push_str(&format!(
                        "| {} | `{}` | {} | {:.3} | {} | {:.0} |\n",
                        i + 1,
                        node.name,
                        short_path(&node.location.file.to_string_lossy()),
                        pr,
                        node.heuristics.fan_in,
                        node.heuristics.cognitive_load_score,
                    ));
                }
                output.push('\n');
            }
        } else {
            output.push_str("### 🔴 Top Bottlenecks\n\n");
            output.push_str("| # | Symbol | File | p99 (ms) | CPU% | Calls | Risk |\n");
            output.push_str("|:--|:-------|:-----|:--------:|:----:|:-----:|:----:|\n");
            for (i, node) in result.details.iter().take(top_n).enumerate() {
                let (p99, cpu, calls) = node.telemetry_summary.as_ref()
                    .map(|t| (t.exec_time_p99_ms, t.cpu_percent, t.invocation_count))
                    .unwrap_or((0.0, 0.0, 0));
                output.push_str(&format!(
                    "| {} | `{}` | {} | {:.1} | {:.1}% | {} | {} |\n",
                    i + 1, node.name, short_path(&node.file),
                    p99, cpu, calls, risk_emoji(&node.risk_level),
                ));
            }
            output.push('\n');
        }
    }

    // Hot paths section
    if filter == "all" || filter == "hot_paths" {
        let result = engine.find_hot_paths();
        if !result.details.is_empty() {
            output.push_str("### 🟠 Hot-Path Functions\n\n");
            output.push_str("| # | Symbol | File | Calls | p50 (ms) | Hot |\n");
            output.push_str("|:--|:-------|:-----|:-----:|:--------:|:---:|\n");
            for (i, node) in result.details.iter().take(top_n).enumerate() {
                let (calls, p50) = node.telemetry_summary.as_ref()
                    .map(|t| (t.invocation_count, t.exec_time_p50_ms))
                    .unwrap_or((0, 0.0));
                output.push_str(&format!(
                    "| {} | `{}` | {} | {} | {:.1} | 🔥 |\n",
                    i + 1, node.name, short_path(&node.file), calls, p50,
                ));
            }
            output.push('\n');
        }
    }

    // High complexity (static heuristics) — use cognitive load threshold
    if filter == "all" || filter == "high_complexity" {
        let result = engine.high_complexity_functions(40.0);
        if !result.details.is_empty() {
            output.push_str("### 🟡 High Complexity (Static — No Telemetry Yet)\n\n");
            output.push_str("| # | Symbol | File | Cognitive Load | Risk |\n");
            output.push_str("|:--|:-------|:-----|:--------------:|:----:|\n");
            for (i, node) in result.details.iter().take(top_n).enumerate() {
                output.push_str(&format!(
                    "| {} | `{}` | {} | {:.0} | {} |\n",
                    i + 1, node.name, short_path(&node.file),
                    node.cognitive_load, risk_emoji(&node.risk_level),
                ));
            }
            output.push('\n');
        }
    }

    if report.hot_paths == 0 && report.bottlenecks == 0 {
        output.push_str("\n> 💡 **Tip:** Telemetry is empty — run Node.js with `node --inspect-brk` to populate runtime data.\n");
    }
    output.push_str(&format!(
        "\n_Provenance: `_meta.provenance.untrusted_content = true` | Edges: {} (resolved: {}, unresolved: {})_\n",
        report.total_edges, report.resolved_calls, report.unresolved_calls
    ));

    CallToolResult::text(output)
}

fn short_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    if parts.len() > 3 {
        format!("…/{}", parts[parts.len() - 2..].join("/"))
    } else {
        path.to_string()
    }
}

fn risk_emoji(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Critical => "🔴 CRITICAL",
        RiskLevel::High => "🟠 HIGH",
        RiskLevel::Medium => "🟡 MEDIUM",
        RiskLevel::Low => "🟢 LOW",
    }
}
