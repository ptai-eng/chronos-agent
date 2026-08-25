//! # Graph Query Engine
//!
//! High-level query API for the 4D Knowledge Graph.
//! Used by MCP tools and CLI to answer questions like:
//! - "What functions are bottlenecks?"
//! - "What is the blast radius of changing this function?"
//! - "What tests cover this code path?"

use crate::graph::schema::{ChronosGraph, ChronosNode, NodeId, NodeKind};
use serde::{Deserialize, Serialize};

/// Result of a 4D graph query — structured for MCP tool responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_type: String,
    pub nodes_found: usize,
    pub summary: String,
    pub details: Vec<NodeSummary>,
}

/// A compact summary of a single node for query results.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub risk_level: RiskLevel,
    pub cognitive_load: f32,
    /// Present only when telemetry is available
    pub telemetry_summary: Option<TelemetrySummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetrySummary {
    pub exec_time_p50_ms: f64,
    pub exec_time_p99_ms: f64,
    pub invocation_count: u64,
    pub cpu_percent: f32,
    pub is_hot_path: bool,
    pub is_bottleneck: bool,
    pub bottleneck_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl NodeSummary {
    pub fn from_node(node: &ChronosNode) -> Self {
        let risk_level = if let Some(t) = &node.telemetry {
            if t.is_bottleneck { RiskLevel::Critical }
            else if t.is_hot_path { RiskLevel::High }
            else if t.cpu_percent_self > 10.0 { RiskLevel::Medium }
            else { RiskLevel::Low }
        } else {
            // Cold-start: risk from cognitive load
            if node.heuristics.cognitive_load_score >= 70.0 { RiskLevel::High }
            else if node.heuristics.cognitive_load_score >= 40.0 { RiskLevel::Medium }
            else { RiskLevel::Low }
        };

        let telemetry_summary = node.telemetry.as_ref().map(|t| TelemetrySummary {
            exec_time_p50_ms: t.exec_time_ms_p50,
            exec_time_p99_ms: t.exec_time_ms_p99,
            invocation_count: t.invocation_count,
            cpu_percent: t.cpu_percent_self,
            is_hot_path: t.is_hot_path,
            is_bottleneck: t.is_bottleneck,
            bottleneck_reason: t.bottleneck_reason.clone(),
        });

        NodeSummary {
            id: node.id.to_string(),
            name: node.name.clone(),
            kind: format!("{:?}", node.kind),
            file: node.location.file.to_string_lossy().to_string(),
            line: node.location.start_line,
            risk_level,
            cognitive_load: node.heuristics.cognitive_load_score,
            telemetry_summary,
        }
    }
}

/// High-level query operations on the ChronosGraph.
pub struct GraphQueryEngine<'g> {
    graph: &'g ChronosGraph,
}

impl<'g> GraphQueryEngine<'g> {
    pub fn new(graph: &'g ChronosGraph) -> Self {
        Self { graph }
    }

    /// Query: Find all bottleneck nodes (requires telemetry).
    pub fn find_bottlenecks(&self) -> QueryResult {
        let nodes = self.graph.bottleneck_nodes();
        let details: Vec<NodeSummary> = nodes.iter().map(|n| NodeSummary::from_node(n)).collect();
        let summary = if details.is_empty() {
            "✅ No bottlenecks detected in current telemetry data.".to_string()
        } else {
            format!(
                "⚠️  {} bottleneck(s) detected. Top offender: '{}' ({})",
                details.len(),
                details[0].name,
                details[0].telemetry_summary.as_ref()
                    .map(|t| format!("p99={}ms, cpu={:.1}%", t.exec_time_p99_ms, t.cpu_percent))
                    .unwrap_or_default()
            )
        };

        QueryResult {
            query_type: "find_bottlenecks".to_string(),
            nodes_found: details.len(),
            summary,
            details,
        }
    }

    /// Query: Find all hot-path nodes.
    pub fn find_hot_paths(&self) -> QueryResult {
        let nodes = self.graph.hot_path_nodes();
        let details: Vec<NodeSummary> = nodes.iter().map(|n| NodeSummary::from_node(n)).collect();
        QueryResult {
            query_type: "find_hot_paths".to_string(),
            nodes_found: details.len(),
            summary: format!("🔥 {} hot-path function(s) identified.", details.len()),
            details,
        }
    }

    /// Query: What functions call the given target? (Blast-radius caller chain)
    pub fn callers_chain(&self, target_id: &NodeId, max_depth: usize) -> QueryResult {
        let mut all_callers = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        
        queue.push_back((target_id.clone(), 0));
        
        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }
            
            for caller in self.graph.callers_of(&current_id) {
                if all_callers.insert(caller.id.clone()) {
                    queue.push_back((caller.id.clone(), depth + 1));
                }
            }
        }
        
        let details: Vec<NodeSummary> = all_callers.into_iter()
            .filter_map(|id| self.graph.get_node(&id))
            .map(|n| NodeSummary::from_node(n))
            .collect();
            
        let target_name = self.graph.get_node(target_id)
            .map(|n| n.name.as_str())
            .unwrap_or("unknown");
            
        QueryResult {
            query_type: "callers_chain".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "📡 '{}' is called by {} function(s) (up to depth {}).",
                target_name, details.len(), max_depth
            ),
            details,
        }
    }

    /// Query: What test cases cover a given function?
    pub fn covering_tests(&self, target_id: &NodeId) -> QueryResult {
        let tests = self.graph.find_covering_tests(target_id);
        let details: Vec<NodeSummary> = tests.iter().map(|n| NodeSummary::from_node(n)).collect();
        let target_name = self.graph.get_node(target_id)
            .map(|n| n.name.as_str())
            .unwrap_or("unknown");
        QueryResult {
            query_type: "covering_tests".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "🧪 Found {} test case(s) that cover '{}'. Mutation arena will run only these.",
                details.len(), target_name
            ),
            details,
        }
    }

    /// Query: Functions with high cognitive complexity (cold-start quality signal).
    pub fn high_complexity_functions(&self, min_score: f32) -> QueryResult {
        let mut nodes: Vec<&ChronosNode> = self.graph.nodes_by_kind(&NodeKind::Function)
            .into_iter()
            .filter(|n| n.heuristics.cognitive_load_score >= min_score)
            .collect();
            
        // Sort before mapping to avoid creating NodeSummary structs unnecessarily
        nodes.sort_by(|a, b| b.heuristics.cognitive_load_score.partial_cmp(&a.heuristics.cognitive_load_score).unwrap());
        
        let details: Vec<NodeSummary> = nodes.iter().map(|n| NodeSummary::from_node(n)).collect();

        QueryResult {
            query_type: "high_complexity_functions".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "🧩 {} function(s) with cognitive load score >= {:.0} (high refactoring risk).",
                details.len(), min_score
            ),
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::*;
    use std::path::PathBuf;

    fn make_fn_node(file: &str, name: &str) -> ChronosNode {
        let id = NodeId::new(file, name, &NodeKind::Function);
        ChronosNode::new(
            id, NodeKind::Function, name,
            format!("{file}::{name}"),
            SourceLocation { file: PathBuf::from(file), start_line: 1, start_col: 0, end_line: 10, end_col: 1 },
        )
    }

    #[test]
    fn test_find_bottlenecks_empty() {
        let graph = ChronosGraph::new();
        let qe = GraphQueryEngine::new(&graph);
        let result = qe.find_bottlenecks();
        assert_eq!(result.nodes_found, 0);
        assert!(result.summary.contains("No bottlenecks"));
    }

    #[test]
    fn test_find_bottlenecks_with_data() {
        let mut graph = ChronosGraph::new();
        let mut node = make_fn_node("src/api.ts", "slowFunction");
        let id = node.id.clone();
        node.heuristics.cyclomatic_complexity = 10;
        node.heuristics.compute_cognitive_load();
        graph.add_node(node);

        graph.hydrate_node(&id, TelemetrySnapshot {
            captured_at: chrono::Utc::now(),
            exec_time_ms_p50: 200.0,
            exec_time_ms_p95: 800.0,
            exec_time_ms_p99: 1500.0,
            alloc_delta_kb: 1024.0,
            invocation_count: 500,
            is_hot_path: true,
            cpu_percent_self: 60.0,
            is_bottleneck: true,
            bottleneck_reason: Some("Test bottleneck".to_string()),
        });

        let qe = GraphQueryEngine::new(&graph);
        let result = qe.find_bottlenecks();
        assert_eq!(result.nodes_found, 1);
        assert_eq!(result.details[0].name, "slowFunction");
        assert_eq!(result.details[0].risk_level, RiskLevel::Critical);
    }
}
