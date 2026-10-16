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
    /// Number of call references successfully resolved to graph edges
    pub resolved_call_sites: usize,
    /// Number of dynamic or external call sites that could not be statically resolved
    pub unresolved_call_sites: usize,
}

impl ChronosGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_index: HashMap::new(),
            last_parsed: None,
            last_hydrated: None,
            resolved_call_sites: 0,
            unresolved_call_sites: 0,
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

    /// Return all directed edges in the graph as `(source_id, target_id, edge)` triples.
    pub fn all_edges(&self) -> Vec<(NodeId, NodeId, ChronosEdge)> {
        self.graph
            .edge_references()
            .map(|e| {
                let src = self.graph[e.source()].id.clone();
                let dst = self.graph[e.target()].id.clone();
                (src, dst, e.weight().clone())
            })
            .collect()
    }

    /// Ingest all symbols and call references from a `ParsedModule` into the graph.
    ///
    /// Automatically links `Calls` and `Tests` edges, updates `fan_in`/`fan_out` heuristics,
    /// and tracks resolved vs. unresolved call sites for coverage reporting.
    /// Returns `(nodes_inserted, edges_added)`.
    pub fn ingest_parsed_module(
        &mut self,
        parsed: &crate::ast::ts_parser::ParsedModule,
    ) -> (usize, usize) {
        let mut inserted = 0usize;
        for node in parsed.all_nodes() {
            self.add_node(node.clone());
            inserted += 1;
        }

        // Build symbol name -> NodeId index across all callable/targetable nodes
        let mut name_to_id: HashMap<String, NodeId> = HashMap::new();
        let mut test_ids: std::collections::HashSet<NodeId> = std::collections::HashSet::new();

        for node in self.graph.node_weights() {
            if matches!(
                node.kind,
                NodeKind::Function | NodeKind::Class | NodeKind::TestCase | NodeKind::ApiRoute
            ) {
                name_to_id.insert(node.name.clone(), node.id.clone());
                if !node.qualified_name.is_empty() {
                    name_to_id.insert(node.qualified_name.clone(), node.id.clone());
                }
                if let Some(short) = node.name.rsplit(|c| c == '.' || c == ':').next() {
                    if !short.is_empty() {
                        name_to_id.entry(short.to_string()).or_insert_with(|| node.id.clone());
                    }
                }
                if node.kind == NodeKind::TestCase {
                    test_ids.insert(node.id.clone());
                }
            }
        }

        let mut edges_added = 0usize;
        self.unresolved_call_sites += parsed.unresolved_call_sites;

        for call_ref in &parsed.call_refs {
            let caller_id = name_to_id.get(&call_ref.caller_name).cloned();
            let callee_id = name_to_id.get(&call_ref.callee_name).cloned();

            match (caller_id, callee_id) {
                (Some(caller), Some(callee)) if caller != callee => {
                    let call_edge = ChronosEdge {
                        kind: EdgeKind::Calls,
                        call_site_line: Some(call_ref.call_site_line),
                        coverage_traced: test_ids.contains(&caller),
                    };
                    if self.add_edge(&caller, &callee, call_edge).is_ok() {
                        edges_added += 1;
                        self.resolved_call_sites += 1;
                    }
                    if test_ids.contains(&caller) {
                        let _ = self.add_edge(
                            &caller,
                            &callee,
                            ChronosEdge {
                                kind: EdgeKind::Tests,
                                call_site_line: Some(call_ref.call_site_line),
                                coverage_traced: true,
                            },
                        );
                    }
                }
                (Some(_), None) => {
                    if !is_common_builtin(&call_ref.callee_name) {
                        self.unresolved_call_sites += 1;
                    }
                }
                _ => {}
            }
        }

        // Refresh fan_in / fan_out on nodes
        let all_ids: Vec<NodeId> = self.node_index.keys().cloned().collect();
        for id in all_ids {
            let fin = self.callers_of(&id).len() as u32;
            let fout = self.callees_of(&id).len() as u32;
            if let Some(node) = self.get_node_mut(&id) {
                node.heuristics.fan_in = fin;
                node.heuristics.fan_out = node.heuristics.fan_out.max(fout);
                node.heuristics.compute_cognitive_load();
            }
        }

        self.last_parsed = Some(Utc::now());
        (inserted, edges_added)
    }

    /// Compute PageRank centrality across all CALLS edges in the graph.
    ///
    /// Inspired by NetworkX (`nx.pagerank`): identifies structural hub/bottleneck functions
    /// that transitive callers depend on heavily, even before runtime telemetry is available.
    pub fn compute_pagerank(&self, iterations: usize, damping: f64) -> HashMap<NodeId, f64> {
        let n = self.graph.node_count();
        if n == 0 {
            return HashMap::new();
        }
        let init_val = 1.0 / (n as f64);
        let mut ranks: HashMap<NodeIndex, f64> = self
            .graph
            .node_indices()
            .map(|idx| (idx, init_val))
            .collect();

        let base_rank = (1.0 - damping) / (n as f64);

        for _ in 0..iterations {
            let mut next_ranks: HashMap<NodeIndex, f64> = self
                .graph
                .node_indices()
                .map(|idx| (idx, base_rank))
                .collect();

            let mut dangling_sum = 0.0;
            for idx in self.graph.node_indices() {
                let out_targets: Vec<NodeIndex> = self
                    .graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .filter(|e| matches!(e.weight().kind, EdgeKind::Calls))
                    .map(|e| e.target())
                    .collect();

                let r = *ranks.get(&idx).unwrap_or(&0.0);
                if out_targets.is_empty() {
                    dangling_sum += r;
                } else {
                    let share = damping * r / (out_targets.len() as f64);
                    for t in out_targets {
                        *next_ranks.entry(t).or_insert(base_rank) += share;
                    }
                }
            }

            let dangling_share = damping * dangling_sum / (n as f64);
            for val in next_ranks.values_mut() {
                *val += dangling_share;
            }
            ranks = next_ranks;
        }

        ranks
            .into_iter()
            .map(|(idx, score)| (self.graph[idx].id.clone(), score))
            .collect()
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

    /// Alias for `find_covering_tests` returning all test cases covering `target`.
    pub fn tests_for(&self, target: &NodeId) -> Vec<&ChronosNode> {
        self.find_covering_tests(target)
    }

    /// Total node count in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total edge count in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Return references to all nodes in the graph.
    pub fn all_nodes(&self) -> Vec<&ChronosNode> {
        self.graph.node_weights().collect()
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
            total_edges: self.edge_count(),
            resolved_calls: self.resolved_call_sites,
            unresolved_calls: self.unresolved_call_sites,
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

fn is_common_builtin(name: &str) -> bool {
    matches!(
        name,
        "log"
            | "info"
            | "warn"
            | "error"
            | "debug"
            | "print"
            | "println"
            | "format"
            | "len"
            | "push"
            | "pop"
            | "map"
            | "filter"
            | "collect"
            | "iter"
            | "into_iter"
            | "unwrap"
            | "expect"
            | "ok"
            | "err"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "clone"
            | "to_string"
            | "as_str"
            | "toBeDefined"
            | "toBe"
            | "toEqual"
            | "describe"
            | "it"
            | "test"
            | "assert"
            | "assert_eq"
            | "new"
            | "default"
            | "json"
            | "status"
            | "Router"
            | "Flask"
            | "dumps"
            | "loads"
    )
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
    pub total_edges: usize,
    pub resolved_calls: usize,
    pub unresolved_calls: usize,
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
             🕸️  Edges: {} total (resolved calls: {}, unresolved: {})",
            self.total_nodes, self.function_nodes, self.test_nodes,
            self.hot_paths, self.bottlenecks,
            self.total_edges, self.resolved_calls, self.unresolved_calls
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
