//! # Graph Schema — Nodes and Edges of the 4D Knowledge Graph
//!
//! Defines the complete type system for CHRONOS's in-memory graph.
//!
//! ## Node Types
//! Static topology nodes covering the full code structure:
//! - File → Class → Function (containment hierarchy)  
//! - API Routes extracted from Express/Fastify patterns
//!
//! ## 4D Telemetry Overlay
//! Each node carries an optional `TelemetrySnapshot` populated after runtime profiling.
//! When cold (no telemetry yet), heuristics fill in static complexity scores.

use chrono::{DateTime, Utc};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for graph nodes — stable across re-parses.
/// Derived from: file path + symbol name + kind (to handle same-name overloads).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(file: &str, name: &str, kind: &NodeKind) -> Self {
        let kind_str = format!("{:?}", kind);
        NodeId(format!("{file}::{kind_str}::{name}"))
    }

    pub fn file(path: &str) -> Self {
        NodeId(format!("file::{path}"))
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The kind of a node in the 4D knowledge graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// A source file (TypeScript/Python module)
    File,
    /// A class or interface definition
    Class,
    /// A function, method, or arrow function
    Function,
    /// An API route endpoint (e.g., `app.get('/users', handler)`)
    ApiRoute,
    /// A TypeScript interface or type alias (structural contract)
    TypeDefinition,
    /// A test function/describe block
    TestCase,
}

/// The kind of edge connecting two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// A calls B: function invocation
    Calls,
    /// A contains B: file contains class, class contains method
    Contains,
    /// A exposes B: file exports a function/class  
    Exposes,
    /// A imports B: file imports from another module
    Imports,
    /// A implements B: class implements interface
    Implements,
    /// A tests B: test case covers a specific function
    Tests,
}

/// Source location in the original file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// Static complexity heuristics — computed from AST alone, available immediately (cold-start).
/// These serve as the Dimension-1..3 "spatial" data before runtime telemetry arrives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StaticHeuristics {
    /// Cyclomatic complexity: counts branching (if/else/for/while/match/ternary/&&/||)
    pub cyclomatic_complexity: u32,
    /// Number of distinct callers (how many places call this function)
    pub fan_in: u32,
    /// Number of distinct callees (how many other functions this calls)
    pub fan_out: u32,
    /// Number of parameters
    pub param_count: u32,
    /// Approximate lines of code (excluding blank lines and comments)
    pub sloc: u32,
    /// Derived cognitive load score (0-100): higher = harder to understand/change
    pub cognitive_load_score: f32,
}

impl StaticHeuristics {
    /// Compute cognitive load score from component metrics.
    /// Formula inspired by SonarQube's Cognitive Complexity metric.
    pub fn compute_cognitive_load(&mut self) {
        self.cognitive_load_score = (
            self.cyclomatic_complexity as f32 * 2.0
            + self.fan_out as f32 * 1.5
            + self.param_count as f32 * 1.0
            + (self.sloc as f32 / 10.0).min(20.0)
        ).min(100.0);
    }
}

/// 4th Dimension: Runtime telemetry snapshot captured from a profiling session.
/// Populated after running `chronos profile` or via CDP WebSocket adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// When this measurement was taken
    pub captured_at: DateTime<Utc>,
    /// Total wall-clock execution time (milliseconds), p50
    pub exec_time_ms_p50: f64,
    /// Total wall-clock execution time (milliseconds), p95
    pub exec_time_ms_p95: f64,
    /// Total wall-clock execution time (milliseconds), p99
    pub exec_time_ms_p99: f64,
    /// Memory allocation delta in kilobytes (positive = allocating, negative = freeing)
    pub alloc_delta_kb: f64,
    /// Number of times this function was invoked in the profiling session
    pub invocation_count: u64,
    /// Whether this function is on a hot path (called frequently & takes significant time)
    pub is_hot_path: bool,
    /// Percentage of total profiling session CPU time attributed to this function (self time)
    pub cpu_percent_self: f32,
    /// Whether this function shows signs of being a bottleneck
    pub is_bottleneck: bool,
    /// Human-readable bottleneck reason (e.g., "N+1 pattern detected via call frequency")
    pub bottleneck_reason: Option<String>,
}

impl TelemetrySnapshot {
    /// Mark this node as a bottleneck with a reason.
    pub fn flag_bottleneck(&mut self, reason: impl Into<String>) {
        self.is_bottleneck = true;
        self.bottleneck_reason = Some(reason.into());
    }
}

/// A node in the CHRONOS 4D Knowledge Graph.
/// Combines static AST data (Dimensions 1-3) with live telemetry (Dimension 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronosNode {
    /// Stable identifier
    pub id: NodeId,
    /// The kind of code entity this node represents
    pub kind: NodeKind,
    /// Symbol name (function name, class name, file stem, etc.)
    pub name: String,
    /// Fully-qualified name including module path
    pub qualified_name: String,
    /// Source location in the original file
    pub location: SourceLocation,
    /// Static complexity scores (always available; populated at parse time)
    pub heuristics: StaticHeuristics,
    /// 4D runtime telemetry (None until profiling session completes)
    pub telemetry: Option<TelemetrySnapshot>,
    /// Public interface signature (for contract tracking)
    /// For functions: serialized parameter types + return type
    /// For API routes: HTTP method + path pattern
    pub contract_signature: Option<String>,
    /// Whether this node's public contract was recently mutated
    pub contract_mutated: bool,
}

impl ChronosNode {
    pub fn new(
        id: NodeId,
        kind: NodeKind,
        name: impl Into<String>,
        qualified_name: impl Into<String>,
        location: SourceLocation,
    ) -> Self {
        Self {
            id,
            kind,
            name: name.into(),
            qualified_name: qualified_name.into(),
            location,
            heuristics: StaticHeuristics::default(),
            telemetry: None,
            contract_signature: None,
            contract_mutated: false,
        }
    }

    /// Returns true if this node has live telemetry data (4D mode)
    pub fn is_hydrated(&self) -> bool {
        self.telemetry.is_some()
    }

    /// Hydrate this node with runtime telemetry data.
    pub fn hydrate(&mut self, telemetry: TelemetrySnapshot) {
        self.telemetry = Some(telemetry);
    }

    /// Get the display label for visualization (truncated name + risk indicator).
    pub fn display_label(&self) -> String {
        let risk = self.risk_indicator();
        format!("{risk}{}", self.name)
    }

    /// Returns a risk emoji prefix based on telemetry (or heuristics if cold).
    pub fn risk_indicator(&self) -> &'static str {
        if let Some(telem) = &self.telemetry {
            if telem.is_bottleneck { return "🔴 "; }
            if telem.is_hot_path { return "🟡 "; }
            return "🟢 ";
        }
        // Cold-start: fall back to cognitive load heuristic
        if self.heuristics.cognitive_load_score >= 70.0 { return "⚠️ "; }
        if self.heuristics.cognitive_load_score >= 40.0 { return "📊 "; }
        "📄 "
    }
}

/// An edge in the CHRONOS 4D Knowledge Graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronosEdge {
    pub kind: EdgeKind,
    /// Optional: the specific call site line number (for CALLS edges)
    pub call_site_line: Option<usize>,
    /// For test edges: the coverage path discovered during test tracing
    pub coverage_traced: bool,
}

impl ChronosEdge {
    pub fn new(kind: EdgeKind) -> Self {
        Self { kind, call_site_line: None, coverage_traced: false }
    }
}

/// The main CHRONOS 4D Knowledge Graph store.
///
/// Uses petgraph's DiGraph as the underlying structure.
/// Node lookup is O(1) via a HashMap index from NodeId → petgraph NodeIndex.
pub struct ChronosGraph {
    /// The underlying directed graph
    graph: DiGraph<ChronosNode, ChronosEdge>,
    /// Fast lookup: NodeId → petgraph NodeIndex
    node_index: HashMap<NodeId, NodeIndex>,
    /// Timestamp when last rebuilt from AST parse
    pub last_parsed: Option<DateTime<Utc>>,
    /// Timestamp when last hydrated with telemetry
    pub last_hydrated: Option<DateTime<Utc>>,
}

impl ChronosGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_index: HashMap::new(),
            last_parsed: None,
            last_hydrated: None,
        }
    }

    /// Add or replace a node in the graph.
    pub fn add_node(&mut self, node: ChronosNode) -> NodeIndex {
        let id = node.id.clone();
        if let Some(&existing_idx) = self.node_index.get(&id) {
            // Replace existing node data (re-parse update)
            self.graph[existing_idx] = node;
            existing_idx
        } else {
            let idx = self.graph.add_node(node);
            self.node_index.insert(id, idx);
            idx
        }
    }

    /// Add a directed edge between two nodes identified by their NodeIds.
    /// Returns Ok(()) if both nodes exist, Err if either is missing.
    pub fn add_edge(&mut self, from: &NodeId, to: &NodeId, edge: ChronosEdge) -> anyhow::Result<()> {
        let from_idx = self.node_index.get(from)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Source node not found: {from}"))?;
        let to_idx = self.node_index.get(to)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Target node not found: {to}"))?;
            
        // PREVENT MEMORY LEAK: Check for existing edge of same kind before adding
        let mut edges = self.graph.edges_directed(from_idx, petgraph::Direction::Outgoing);
        if !edges.any(|e| e.target() == to_idx && e.weight().kind == edge.kind) {
            self.graph.add_edge(from_idx, to_idx, edge);
        }
        Ok(())
    }

    /// Look up a node by its stable NodeId.
    pub fn get_node(&self, id: &NodeId) -> Option<&ChronosNode> {
        self.node_index.get(id).map(|&idx| &self.graph[idx])
    }

    /// Get a mutable reference to a node.
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut ChronosNode> {
        self.node_index.get(&id.clone()).copied().map(|idx| &mut self.graph[idx])
    }

    /// Hydrate a node with telemetry data. Returns true if node was found.
    pub fn hydrate_node(&mut self, id: &NodeId, telemetry: TelemetrySnapshot) -> bool {
        if let Some(node) = self.get_node_mut(id) {
            node.hydrate(telemetry);
            true
        } else {
            false
        }
    }

    /// Get all nodes of a specific kind.
    pub fn nodes_by_kind(&self, kind: &NodeKind) -> Vec<&ChronosNode> {
        self.graph.node_weights()
            .filter(|n| &n.kind == kind)
            .collect()
    }

    /// Get all hot-path nodes (requires hydration).
    pub fn hot_path_nodes(&self) -> Vec<&ChronosNode> {
        self.graph.node_weights()
            .filter(|n| n.telemetry.as_ref().map_or(false, |t| t.is_hot_path))
            .collect()
    }

    /// Get all bottleneck nodes.
    pub fn bottleneck_nodes(&self) -> Vec<&ChronosNode> {
        self.graph.node_weights()
            .filter(|n| n.telemetry.as_ref().map_or(false, |t| t.is_bottleneck))
            .collect()
    }

    /// Return direct callers of a given node (inbound CALLS edges).
    pub fn callers_of(&self, id: &NodeId) -> Vec<&ChronosNode> {
        let Some(&target_idx) = self.node_index.get(id) else { return vec![]; };
        self.graph.edges_directed(target_idx, petgraph::Direction::Incoming)
            .filter(|e| matches!(e.weight().kind, EdgeKind::Calls))
            .map(|e| &self.graph[e.source()])
            .collect()
    }

    /// Return direct callees of a given node (outbound CALLS edges).
    pub fn callees_of(&self, id: &NodeId) -> Vec<&ChronosNode> {
        let Some(&source_idx) = self.node_index.get(id) else { return vec![]; };
        self.graph.edges_directed(source_idx, petgraph::Direction::Outgoing)
            .filter(|e| matches!(e.weight().kind, EdgeKind::Calls))
            .map(|e| &self.graph[e.target()])
            .collect()
    }

    /// BFS reachability: all nodes reachable from `start` following outgoing edges.
    pub fn reachable_from(&self, start: &NodeId) -> Vec<&ChronosNode> {
        use petgraph::visit::Bfs;
        let Some(&start_idx) = self.node_index.get(start) else { return vec![]; };
        let mut bfs = Bfs::new(&self.graph, start_idx);
        let mut result = vec![];
        while let Some(idx) = bfs.next(&self.graph) {
            if idx != start_idx {
                result.push(&self.graph[idx]);
            }
        }
        result
    }

    /// Find test cases that cover a given function (traverse TEST edges + CALLS paths).
    /// Used by Mutation Arena to find the minimal set of tests to run.
    pub fn find_covering_tests(&self, target: &NodeId) -> Vec<&ChronosNode> {
        let Some(&start_idx) = self.node_index.get(target) else { return vec![]; };
        let mut covering_tests = vec![];
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        
        queue.push_back(start_idx);
        visited.insert(start_idx);
        
        while let Some(current_idx) = queue.pop_front() {
            let node = &self.graph[current_idx];
            if matches!(node.kind, NodeKind::TestCase) {
                covering_tests.push(node);
            }
            
            // Traverse incoming edges (who calls us, or who tests us)
            for edge in self.graph.edges_directed(current_idx, petgraph::Direction::Incoming) {
                let kind = &edge.weight().kind;
                if matches!(kind, EdgeKind::Calls | EdgeKind::Tests) {
                    let source_idx = edge.source();
                    if visited.insert(source_idx) {
                        queue.push_back(source_idx);
                    }
                }
            }
        }
        
        covering_tests
    }

    /// Total node count in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total edge count in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Whether the graph has any telemetry data at all.
    pub fn is_hydrated(&self) -> bool {
        self.last_hydrated.is_some()
    }

    /// Generate a brief status report for CLI/MCP output.
    pub fn status_report(&self) -> GraphStatusReport {
        let total_nodes = self.node_count();
        let mut hydrated_nodes = 0;
        let mut hot_paths = 0;
        let mut bottlenecks = 0;
        let mut function_nodes = 0;
        let mut test_nodes = 0;
        
        // O(1) pass through node weights instead of 5 separate filters O(5N)
        for n in self.graph.node_weights() {
            if n.is_hydrated() {
                hydrated_nodes += 1;
                if let Some(t) = &n.telemetry {
                    if t.is_hot_path { hot_paths += 1; }
                    if t.is_bottleneck { bottlenecks += 1; }
                }
            }
            if n.kind == NodeKind::Function { function_nodes += 1; }
            if n.kind == NodeKind::TestCase { test_nodes += 1; }
        }
        
        GraphStatusReport {
            total_nodes,
            hydrated_nodes,
            hot_paths,
            bottlenecks,
            function_nodes,
            test_nodes,
            last_parsed: self.last_parsed,
            last_hydrated: self.last_hydrated,
        }
    }
}

impl Default for ChronosGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary report on graph status for display.
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphStatusReport {
    pub total_nodes: usize,
    pub hydrated_nodes: usize,
    pub hot_paths: usize,
    pub bottlenecks: usize,
    pub function_nodes: usize,
    pub test_nodes: usize,
    pub last_parsed: Option<DateTime<Utc>>,
    pub last_hydrated: Option<DateTime<Utc>>,
}

impl std::fmt::Display for GraphStatusReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hydration_status = if self.hydrated_nodes > 0 {
            format!("🌡️  4D Hydrated ({}/{})", self.hydrated_nodes, self.total_nodes)
        } else {
            "❄️  Cold (Static AST only)".to_string()
        };
        write!(f,
            "📊 CHRONOS Graph Status\n\
             {hydration_status}\n\
             📁 Nodes: {} total ({} functions, {} tests)\n\
             🔥 Hot paths: {} | ⚠️  Bottlenecks: {}\n\
             🕸️  Edges: (see graph query for details)",
            self.total_nodes, self.function_nodes, self.test_nodes,
            self.hot_paths, self.bottlenecks
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_location(file: &str, line: usize) -> SourceLocation {
        SourceLocation {
            file: PathBuf::from(file),
            start_line: line,
            start_col: 0,
            end_line: line + 10,
            end_col: 1,
        }
    }

    fn make_function_node(file: &str, name: &str) -> ChronosNode {
        let id = NodeId::new(file, name, &NodeKind::Function);
        ChronosNode::new(
            id,
            NodeKind::Function,
            name,
            format!("{file}::{name}"),
            make_location(file, 1),
        )
    }

    #[test]
    fn test_add_and_retrieve_node() {
        let mut graph = ChronosGraph::new();
        let node = make_function_node("src/api.ts", "getUser");
        let id = node.id.clone();
        graph.add_node(node);

        let retrieved = graph.get_node(&id).expect("Node should exist");
        assert_eq!(retrieved.name, "getUser");
        assert_eq!(retrieved.kind, NodeKind::Function);
        assert!(!retrieved.is_hydrated());
    }

    #[test]
    fn test_add_edge_calls() {
        let mut graph = ChronosGraph::new();
        let caller = make_function_node("src/api.ts", "handleRequest");
        let callee = make_function_node("src/db.ts", "queryUser");
        let caller_id = caller.id.clone();
        let callee_id = callee.id.clone();
        graph.add_node(caller);
        graph.add_node(callee);

        graph.add_edge(&caller_id, &callee_id, ChronosEdge::new(EdgeKind::Calls))
            .expect("Edge should be added");

        assert_eq!(graph.callers_of(&callee_id).len(), 1);
        assert_eq!(graph.callees_of(&caller_id).len(), 1);
    }

    #[test]
    fn test_hydration() {
        let mut graph = ChronosGraph::new();
        let node = make_function_node("src/api.ts", "processOrder");
        let id = node.id.clone();
        graph.add_node(node);

        assert!(!graph.get_node(&id).unwrap().is_hydrated());

        let telemetry = TelemetrySnapshot {
            captured_at: Utc::now(),
            exec_time_ms_p50: 120.0,
            exec_time_ms_p95: 450.0,
            exec_time_ms_p99: 890.0,
            alloc_delta_kb: 512.0,
            invocation_count: 1000,
            is_hot_path: true,
            cpu_percent_self: 45.0,
            is_bottleneck: true,
            bottleneck_reason: Some("High p99 latency (890ms) with 1000 calls".to_string()),
        };

        let hydrated = graph.hydrate_node(&id, telemetry);
        assert!(hydrated);
        assert!(graph.get_node(&id).unwrap().is_hydrated());
        assert_eq!(graph.hot_path_nodes().len(), 1);
        assert_eq!(graph.bottleneck_nodes().len(), 1);
    }

    #[test]
    fn test_cold_start_heuristics() {
        let mut graph = ChronosGraph::new();
        let mut node = make_function_node("src/complex.ts", "processEverything");
        node.heuristics.cyclomatic_complexity = 25;
        node.heuristics.fan_out = 10;
        node.heuristics.param_count = 5;
        node.heuristics.sloc = 80;
        node.heuristics.compute_cognitive_load();
        
        let id = node.id.clone();
        graph.add_node(node);

        let retrieved = graph.get_node(&id).unwrap();
        // High complexity → warning indicator
        assert_eq!(retrieved.risk_indicator(), "⚠️ ");
        assert!(retrieved.heuristics.cognitive_load_score >= 70.0);
    }

    #[test]
    fn test_reachability() {
        let mut graph = ChronosGraph::new();
        let a = make_function_node("src/a.ts", "funcA");
        let b = make_function_node("src/b.ts", "funcB");
        let c = make_function_node("src/c.ts", "funcC");
        let id_a = a.id.clone();
        let id_b = b.id.clone();
        let id_c = c.id.clone();
        graph.add_node(a);
        graph.add_node(b);
        graph.add_node(c);
        
        graph.add_edge(&id_a, &id_b, ChronosEdge::new(EdgeKind::Calls)).unwrap();
        graph.add_edge(&id_b, &id_c, ChronosEdge::new(EdgeKind::Calls)).unwrap();

        let reachable = graph.reachable_from(&id_a);
        assert_eq!(reachable.len(), 2); // B and C
        let names: Vec<&str> = reachable.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"funcB"));
        assert!(names.contains(&"funcC"));
    }

    #[test]
    fn test_status_report() {
        let graph = ChronosGraph::new();
        let report = graph.status_report();
        assert_eq!(report.total_nodes, 0);
        assert!(!graph.is_hydrated());
        
        let display = report.to_string();
        assert!(display.contains("Cold"));
    }
}
