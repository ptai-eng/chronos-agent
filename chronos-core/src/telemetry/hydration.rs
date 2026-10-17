//! # Graph Hydration Engine
//!
//! Bridges the telemetry session data into the 4D Knowledge Graph.
//! Maps function names from profiler output to graph nodes.

use crate::graph::schema::{ChronosGraph, NodeId, NodeKind, TelemetrySnapshot};
use crate::telemetry::adapter::TelemetrySession;
use chrono::Utc;
use tracing::{debug, info};

/// Hydrate graph nodes with telemetry data from a profiling session.
///
/// Strategy: Try exact match first, then suffix match (for qualified names).
/// Nodes that don't match any profiler data remain in cold-start mode.
pub fn hydrate_graph(graph: &mut ChronosGraph, session: &TelemetrySession) -> HydrationReport {
    let mut report = HydrationReport::default();
    report.total_profiled = session.functions.len();

    // Build a lookup: function_name → telemetry data
    let telem_lookup: std::collections::HashMap<&str, &crate::telemetry::adapter::TelemetryData> = session
        .functions
        .iter()
        .map(|f| (f.function_name.as_str(), f))
        .collect();

    // Get all executable node IDs (Function, ApiRoute) that can be hydrated
    let node_ids: Vec<NodeId> = graph
        .all_nodes()
        .into_iter()
        .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::ApiRoute))
        .map(|n| n.id.clone())
        .collect();

    let _total_session_cpu = session.functions.iter().map(|f| f.cpu_self_us).sum::<u64>().max(1);

    for node_id in node_ids {
        let node_name = graph.get_node(&node_id)
            .map(|n| n.name.clone())
            .unwrap_or_default();

        // Try to find matching telemetry data
        let telem_data = telem_lookup.get(node_name.as_str())
            .or_else(|| {
                // Suffix match: "getUser" matches "UserService::getUser", "UserService.getUser", or "pkg/user.getUser"
                telem_lookup.iter()
                    .find(|(k, _)| {
                        **k == node_name
                            || k.ends_with(&format!("::{}", node_name))
                            || k.ends_with(&format!(".{}", node_name))
                            || k.ends_with(&format!("/{}", node_name))
                    })
                    .map(|(_, v)| v)
            });

        if let Some(data) = telem_data {
            let snapshot = TelemetrySnapshot {
                captured_at: Utc::now(),
                exec_time_ms_p50: data.p50_ms(),
                exec_time_ms_p95: data.p95_ms(),
                exec_time_ms_p99: data.p99_ms(),
                alloc_delta_kb: data.alloc_delta_kb,
                invocation_count: data.invocation_count,
                is_hot_path: data.is_hot_path(),
                cpu_percent_self: data.cpu_percent_self(),
                is_bottleneck: data.is_bottleneck(),
                bottleneck_reason: data.bottleneck_reason(),
            };

            if graph.hydrate_node(&node_id, snapshot) {
                report.hydrated_count += 1;
                if data.is_bottleneck() {
                    report.bottlenecks_found += 1;
                }
                debug!("Hydrated node: {} (cpu={:.1}%)", node_name, data.cpu_percent_self());
            }
        } else {
            report.cold_count += 1;
        }
    }

    graph.last_hydrated = Some(Utc::now());

    info!(
        "Hydration complete: {}/{} nodes hydrated, {} bottlenecks found",
        report.hydrated_count,
        report.hydrated_count + report.cold_count,
        report.bottlenecks_found
    );

    report
}

/// Report on the hydration operation
#[derive(Debug, Default)]
pub struct HydrationReport {
    pub total_profiled: usize,
    pub hydrated_count: usize,
    pub cold_count: usize,
    pub bottlenecks_found: usize,
}

impl std::fmt::Display for HydrationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "🌡️  Hydration: {}/{} nodes now 4D | {} still cold | {} bottlenecks",
            self.hydrated_count,
            self.hydrated_count + self.cold_count,
            self.cold_count,
            self.bottlenecks_found
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::*;
    use crate::telemetry::adapter::TelemetryData;
    use std::path::PathBuf;

    fn make_graph_with_functions(names: &[&str]) -> ChronosGraph {
        let mut graph = ChronosGraph::new();
        for name in names {
            let id = NodeId::new("src/api.ts", name, &NodeKind::Function);
            let node = ChronosNode::new(
                id, NodeKind::Function, *name, format!("Module::{}", name),
                SourceLocation { file: PathBuf::from("src/api.ts"), start_line: 1, start_col: 0, end_line: 10, end_col: 1 },
            );
            graph.add_node(node);
        }
        graph
    }

    fn make_session(functions: Vec<(&str, u64, u64, u64)>) -> TelemetrySession {
        let mut session = TelemetrySession::new("test");
        for (name, count, cpu_us, total_cpu) in functions {
            session.functions.push(TelemetryData {
                function_name: name.to_string(),
                execution_times_ms: vec![10.0; count as usize],
                alloc_delta_kb: 0.0,
                invocation_count: count,
                cpu_self_us: cpu_us,
                session_cpu_total_us: total_cpu,
                source_url: None,
                line_number: None,
            });
        }
        session
    }

    #[test]
    fn test_hydration_exact_match() {
        let mut graph = make_graph_with_functions(&["getUser", "processOrder"]);
        let session = make_session(vec![
            ("getUser", 100, 50_000, 100_000),
            ("processOrder", 50, 40_000, 100_000),
        ]);

        let report = hydrate_graph(&mut graph, &session);
        assert_eq!(report.hydrated_count, 2);
        assert_eq!(report.cold_count, 0);
        assert!(graph.is_hydrated());
    }

    #[test]
    fn test_hydration_partial_match() {
        let mut graph = make_graph_with_functions(&["getUser"]);
        let session = make_session(vec![
            ("UserService::getUser", 100, 50_000, 100_000),
        ]);

        let report = hydrate_graph(&mut graph, &session);
        assert_eq!(report.hydrated_count, 1, "Should match via suffix");
    }

    #[test]
    fn test_cold_nodes_remain_cold() {
        let mut graph = make_graph_with_functions(&["unusedFunc"]);
        let session = make_session(vec![
            ("someOtherFunc", 10, 1_000, 100_000),
        ]);

        let report = hydrate_graph(&mut graph, &session);
        assert_eq!(report.cold_count, 1, "Unmatched node should remain cold");
        assert_eq!(report.hydrated_count, 0);
    }

    #[test]
    fn test_bottleneck_detection_during_hydration() {
        let mut graph = make_graph_with_functions(&["slowQuery"]);
        let session = make_session(vec![
            // High CPU + many invocations = bottleneck
            ("slowQuery", 100, 80_000, 100_000),
        ]);
        // Need to add execution times to trigger bottleneck
        let mut session = session;
        session.functions[0].execution_times_ms = vec![500.0; 100]; // high p99

        let report = hydrate_graph(&mut graph, &session);
        assert_eq!(report.bottlenecks_found, 1);
    }
}
