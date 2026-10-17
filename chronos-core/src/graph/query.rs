//! # Graph Query Engine
//!
//! High-level query API for the 4D Knowledge Graph.
//! Used by MCP tools and CLI to answer questions like:
//! - "What functions are bottlenecks?"
//! - "What is the blast radius of changing this function?"
//! - "What tests cover this code path?"
//! - BM25 + Regex symbol search (`search_graph`)
//! - Verbatim source snippet extraction with callers/callees/tests (`get_code_snippet`)
//! - Community-clustered architecture & O(N^2) hotspot overview (`get_architecture`)

use crate::graph::schema::{ArchCluster, ChronosGraph, ChronosNode, NodeId, NodeKind, StaticHeuristics};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a 4D graph query — structured for MCP tool responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_type: String,
    pub nodes_found: usize,
    pub summary: String,
    pub details: Vec<NodeSummary>,
}

/// A compact summary of a single node for query results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub risk_level: RiskLevel,
    pub cognitive_load: f32,
    pub cyclomatic: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub loop_depth: u32,
    pub transitive_loop_depth: u32,
    pub linear_scan_in_loop: u32,
    pub alloc_in_loop: u32,
    pub is_recursive: bool,
    /// Present only when telemetry is available
    pub telemetry_summary: Option<TelemetrySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            if t.is_bottleneck {
                RiskLevel::Critical
            } else if t.is_hot_path {
                RiskLevel::High
            } else if t.cpu_percent_self > 10.0 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            }
        } else if node.heuristics.cognitive_load_score >= 70.0
            || node.heuristics.transitive_loop_depth >= 3
            || node.heuristics.linear_scan_in_loop >= 2
        {
            RiskLevel::High
        } else if node.heuristics.cognitive_load_score >= 40.0
            || node.heuristics.transitive_loop_depth >= 2
            || node.heuristics.linear_scan_in_loop >= 1
        {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
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
            file: node.location.file.to_string_lossy().replace('\\', "/"),
            line: node.location.start_line,
            end_line: node.location.end_line,
            risk_level,
            cognitive_load: node.heuristics.cognitive_load_score,
            cyclomatic: node.heuristics.cyclomatic_complexity,
            fan_in: node.heuristics.fan_in,
            fan_out: node.heuristics.fan_out,
            loop_depth: node.heuristics.loop_depth,
            transitive_loop_depth: node.heuristics.transitive_loop_depth,
            linear_scan_in_loop: node.heuristics.linear_scan_in_loop,
            alloc_in_loop: node.heuristics.alloc_in_loop,
            is_recursive: node.heuristics.is_recursive,
            telemetry_summary,
        }
    }
}

/// Request parameters for BM25 + Regex + Structural Graph Search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphSearchRequest {
    /// Natural-language or keyword query (splits camelCase & snake_case, ranked by BM25 + label boost)
    pub query: Option<String>,
    /// Regex pattern to match against symbol name or qualified_name
    pub name_pattern: Option<String>,
    /// Optional substring or regex filter on file path
    pub file_pattern: Option<String>,
    /// Optional node kind filter: "Function" | "Class" | "ApiRoute" | "TestCase"
    pub kind: Option<String>,
    /// Minimum total degree (`fan_in + fan_out`)
    pub min_degree: Option<u32>,
    /// Minimum nested loop depth (`transitive_loop_depth`)
    pub min_loop_depth: Option<u32>,
    /// Max results per page (default 50)
    pub limit: usize,
    /// Pagination offset (default 0)
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredNodeMatch {
    pub score: f32,
    pub pagerank: f64,
    pub node: NodeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSearchResponse {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub results: Vec<ScoredNodeMatch>,
}

/// Full code snippet response with caller/callee/test topology and AST complexity metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippetResult {
    pub id: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub source_code: String,
    pub pagerank_score: f64,
    pub heuristics: StaticHeuristics,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
    pub covering_tests: Vec<String>,
}

/// Workspace architecture overview with community clusters and algorithmic bottleneck candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureReport {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub languages: HashMap<String, usize>,
    pub node_counts_by_kind: HashMap<String, usize>,
    pub clusters: Vec<ArchCluster>,
    pub entry_points: Vec<NodeSummary>,
    pub top_pagerank_hubs: Vec<ScoredNodeMatch>,
    pub algorithmic_hotspots: Vec<NodeSummary>,
}

/// High-level query operations on the ChronosGraph.
pub struct GraphQueryEngine<'g> {
    graph: &'g ChronosGraph,
}

impl<'g> GraphQueryEngine<'g> {
    pub fn new(graph: &'g ChronosGraph) -> Self {
        Self { graph }
    }

    /// Query: Find all bottleneck nodes (requires telemetry, falls back to O(N^2) static signals).
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
                details[0]
                    .telemetry_summary
                    .as_ref()
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
            if depth >= max_depth {
                continue;
            }

            for caller in self.graph.callers_of(&current_id) {
                if all_callers.insert(caller.id.clone()) {
                    queue.push_back((caller.id.clone(), depth + 1));
                }
            }
        }

        let details: Vec<NodeSummary> = all_callers
            .into_iter()
            .filter_map(|id| self.graph.get_node(&id))
            .map(NodeSummary::from_node)
            .collect();

        let target_name = self
            .graph
            .get_node(target_id)
            .map(|n| n.name.as_str())
            .unwrap_or("unknown");

        QueryResult {
            query_type: "callers_chain".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "📡 '{}' is called by {} function(s) (up to depth {}).",
                target_name,
                details.len(),
                max_depth
            ),
            details,
        }
    }

    /// Query: What test cases cover a given function?
    pub fn covering_tests(&self, target_id: &NodeId) -> QueryResult {
        let tests = self.graph.find_covering_tests(target_id);
        let details: Vec<NodeSummary> = tests.iter().map(|n| NodeSummary::from_node(n)).collect();
        let target_name = self
            .graph
            .get_node(target_id)
            .map(|n| n.name.as_str())
            .unwrap_or("unknown");
        QueryResult {
            query_type: "covering_tests".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "🧪 Found {} test case(s) that cover '{}'. Mutation arena will run only these.",
                details.len(),
                target_name
            ),
            details,
        }
    }

    /// Query: Functions with high cognitive complexity (cold-start quality signal).
    pub fn high_complexity_functions(&self, min_score: f32) -> QueryResult {
        let mut nodes: Vec<&ChronosNode> = self
            .graph
            .nodes_by_kind(&NodeKind::Function)
            .into_iter()
            .filter(|n| n.heuristics.cognitive_load_score >= min_score)
            .collect();

        nodes.sort_by(|a, b| {
            b.heuristics
                .cognitive_load_score
                .partial_cmp(&a.heuristics.cognitive_load_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let details: Vec<NodeSummary> = nodes.iter().map(|n| NodeSummary::from_node(n)).collect();

        QueryResult {
            query_type: "high_complexity_functions".to_string(),
            nodes_found: details.len(),
            summary: format!(
                "🧩 {} function(s) with cognitive load score >= {:.0} (high refactoring risk).",
                details.len(),
                min_score
            ),
            details,
        }
    }

    /// Search the 4D knowledge graph using BM25 identifier tokenization (`camelCase` / `snake_case`),
    /// regex matching, degree/loop filters, and structural label boosting.
    pub fn search_graph(&self, req: &GraphSearchRequest) -> GraphSearchResponse {
        let limit = if req.limit == 0 { 50 } else { req.limit.min(500) };
        let pagerank = self.graph.compute_pagerank(15, 0.85);

        let name_regex = req
            .name_pattern
            .as_ref()
            .and_then(|pat| Regex::new(pat).ok());
        let file_regex = req
            .file_pattern
            .as_ref()
            .and_then(|pat| Regex::new(pat).ok());

        let query_tokens: Vec<String> = req
            .query
            .as_ref()
            .map(|q| tokenize_identifier(q))
            .unwrap_or_default();

        // Compute document frequencies for BM25 IDF across all nodes
        let all_nodes = self.graph.all_nodes();
        let total_docs = all_nodes.len().max(1) as f32;
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        if !query_tokens.is_empty() {
            for node in &all_nodes {
                let mut seen = std::collections::HashSet::new();
                for tok in tokenize_identifier(&node.name)
                    .into_iter()
                    .chain(tokenize_identifier(&node.qualified_name))
                {
                    seen.insert(tok);
                }
                for tok in seen {
                    *doc_freq.entry(tok).or_insert(0) += 1;
                }
            }
        }

        let mut matches: Vec<ScoredNodeMatch> = Vec::new();

        for node in all_nodes {
            // Filter by kind
            if let Some(kind_filter) = &req.kind {
                let node_kind_str = format!("{:?}", node.kind);
                if !node_kind_str.eq_ignore_ascii_case(kind_filter) {
                    continue;
                }
            }

            // Filter by file pattern
            let file_str = node.location.file.to_string_lossy().replace('\\', "/");
            if let Some(fp) = &req.file_pattern {
                let matched = file_regex
                    .as_ref()
                    .map(|re| re.is_match(&file_str))
                    .unwrap_or_else(|| file_str.contains(fp));
                if !matched {
                    continue;
                }
            }

            // Filter by degree
            if let Some(min_deg) = req.min_degree {
                let deg = node.heuristics.fan_in + node.heuristics.fan_out;
                if deg < min_deg {
                    continue;
                }
            }

            // Filter by loop depth
            if let Some(min_ld) = req.min_loop_depth {
                if node.heuristics.transitive_loop_depth < min_ld {
                    continue;
                }
            }

            // Filter by regex name_pattern
            if let Some(pat) = &req.name_pattern {
                let matched = name_regex
                    .as_ref()
                    .map(|re| re.is_match(&node.name) || re.is_match(&node.qualified_name))
                    .unwrap_or_else(|| node.name.contains(pat));
                if !matched {
                    continue;
                }
            }

            // Compute BM25 + structural boost score
            let mut score = match node.kind {
                NodeKind::Function => 10.0,
                NodeKind::ApiRoute => 8.0,
                NodeKind::Class => 5.0,
                NodeKind::TestCase => 3.0,
                NodeKind::TypeDefinition => 2.0,
                NodeKind::File => 1.0,
            };

            if !query_tokens.is_empty() {
                let node_tokens = tokenize_identifier(&node.name);
                let qn_tokens = tokenize_identifier(&node.qualified_name);
                let mut bm25_hits = 0usize;
                let mut bm25_score = 0.0f32;

                for qtok in &query_tokens {
                    let tf_name = node_tokens
                        .iter()
                        .filter(|t| *t == qtok || (qtok.len() >= 3 && t.starts_with(qtok.as_str())))
                        .count() as f32
                        * 2.0;
                    let tf_qn = qn_tokens
                        .iter()
                        .filter(|t| *t == qtok || (qtok.len() >= 3 && t.starts_with(qtok.as_str())))
                        .count() as f32;
                    let sub_match = if node.name.to_lowercase().contains(qtok) { 1.0 } else { 0.0 };
                    let tf = tf_name + tf_qn + sub_match;

                    if tf > 0.0 {
                        bm25_hits += 1;
                        let df = *doc_freq.get(qtok).unwrap_or(&1) as f32;
                        let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.1);
                        let k1 = 1.2f32;
                        bm25_score += idf * ((tf * (k1 + 1.0)) / (tf + k1));
                    }
                }

                if bm25_hits == 0 {
                    continue;
                }
                score += bm25_score * 10.0 + (bm25_hits as f32) * 15.0;
                if node.name.eq_ignore_ascii_case(req.query.as_deref().unwrap_or("")) {
                    score += 50.0;
                }
            }

            let pr = pagerank.get(&node.id).copied().unwrap_or(0.0);
            score += (pr * 25.0) as f32;

            matches.push(ScoredNodeMatch {
                score,
                pagerank: pr,
                node: NodeSummary::from_node(node),
            });
        }

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.name.cmp(&b.node.name))
        });

        let total = matches.len();
        let results: Vec<ScoredNodeMatch> = matches
            .into_iter()
            .skip(req.offset)
            .take(limit)
            .collect();
        let has_more = total > req.offset + results.len();

        GraphSearchResponse {
            total,
            offset: req.offset,
            limit,
            has_more,
            results,
        }
    }

    /// Retrieve the exact source snippet (`start_line..=end_line`) for a symbol
    /// along with its callers, callees, covering tests, PageRank, and AST complexity metrics.
    pub fn get_code_snippet(
        &self,
        symbol_name: &str,
        file_filter: Option<&str>,
    ) -> Option<CodeSnippetResult> {
        let all_nodes = self.graph.all_nodes();
        let matched = all_nodes
            .iter()
            .find(|n| {
                let name_ok = n.name == symbol_name
                    || n.qualified_name == symbol_name
                    || n.name.ends_with(&format!(".{symbol_name}"))
                    || n.name.ends_with(&format!("::{symbol_name}"));
                let file_ok = file_filter.map_or(true, |f| {
                    n.location
                        .file
                        .to_string_lossy()
                        .replace('\\', "/")
                        .contains(&f.replace('\\', "/"))
                });
                name_ok && file_ok
            })
            .copied()?;

        let pagerank = self.graph.compute_pagerank(15, 0.85);
        let pr = pagerank.get(&matched.id).copied().unwrap_or(0.0);

        let source_code = if let Ok(content) = std::fs::read_to_string(&matched.location.file) {
            let lines: Vec<&str> = content.lines().collect();
            let start = matched.location.start_line.saturating_sub(1);
            let end = matched.location.end_line.min(lines.len()).max(start + 1);
            lines[start..end]
                .iter()
                .enumerate()
                .map(|(idx, line)| format!("{:>4} | {}", start + idx + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            format!(
                "// Source file not readable on disk: {}",
                matched.location.file.display()
            )
        };

        let callers = self
            .graph
            .callers_of(&matched.id)
            .into_iter()
            .map(|n| n.name.clone())
            .collect();
        let callees = self
            .graph
            .callees_of(&matched.id)
            .into_iter()
            .map(|n| n.name.clone())
            .collect();
        let covering_tests = self
            .graph
            .tests_for(&matched.id)
            .into_iter()
            .map(|n| n.name.clone())
            .collect();

        Some(CodeSnippetResult {
            id: matched.id.to_string(),
            name: matched.name.clone(),
            qualified_name: matched.qualified_name.clone(),
            kind: format!("{:?}", matched.kind),
            file: matched.location.file.to_string_lossy().replace('\\', "/"),
            start_line: matched.location.start_line,
            end_line: matched.location.end_line,
            source_code,
            pagerank_score: pr,
            heuristics: matched.heuristics.clone(),
            callers,
            callees,
            covering_tests,
        })
    }

    /// Generate a comprehensive architecture overview: community clusters (Cohesion/Coupling),
    /// language breakdown, entry points, top PageRank hubs, and hidden O(N^2) hotspot candidates.
    pub fn get_architecture(&self, path_prefix: Option<&str>) -> ArchitectureReport {
        let normalized_prefix = path_prefix.map(|p| p.replace('\\', "/"));
        let all_nodes: Vec<&ChronosNode> = self
            .graph
            .all_nodes()
            .into_iter()
            .filter(|n| {
                normalized_prefix.as_ref().map_or(true, |pref| {
                    n.location
                        .file
                        .to_string_lossy()
                        .replace('\\', "/")
                        .contains(pref)
                })
            })
            .collect();

        let mut languages: HashMap<String, usize> = HashMap::new();
        let mut node_counts_by_kind: HashMap<String, usize> = HashMap::new();

        for node in &all_nodes {
            *node_counts_by_kind
                .entry(format!("{:?}", node.kind))
                .or_insert(0) += 1;
            let ext = node
                .location
                .file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            let lang = match ext {
                "ts" | "tsx" => "TypeScript",
                "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
                "py" => "Python",
                "rs" => "Rust",
                "go" => "Go",
                "c" | "h" => "C",
                "cpp" | "cc" | "cxx" | "hpp" => "C++",
                "java" => "Java",
                other => other,
            };
            *languages.entry(lang.to_string()).or_insert(0) += 1;
        }

        let pagerank = self.graph.compute_pagerank(15, 0.85);
        let mut hubs: Vec<ScoredNodeMatch> = all_nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Class))
            .map(|n| {
                let pr = pagerank.get(&n.id).copied().unwrap_or(0.0);
                ScoredNodeMatch {
                    score: (pr * 100.0) as f32,
                    pagerank: pr,
                    node: NodeSummary::from_node(n),
                }
            })
            .collect();
        hubs.sort_by(|a, b| {
            b.pagerank
                .partial_cmp(&a.pagerank)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hubs.truncate(10);

        let entry_points: Vec<NodeSummary> = all_nodes
            .iter()
            .filter(|n| {
                matches!(n.kind, NodeKind::ApiRoute)
                    || (n.kind == NodeKind::Function
                        && n.heuristics.fan_in == 0
                        && n.heuristics.fan_out > 0)
            })
            .take(15)
            .map(|n| NodeSummary::from_node(n))
            .collect();

        let mut algorithmic_hotspots: Vec<NodeSummary> = all_nodes
            .iter()
            .filter(|n| {
                n.heuristics.transitive_loop_depth >= 2
                    || n.heuristics.linear_scan_in_loop >= 1
                    || n.heuristics.is_recursive
            })
            .map(|n| NodeSummary::from_node(n))
            .collect();
        algorithmic_hotspots.sort_by(|a, b| {
            b.transitive_loop_depth
                .cmp(&a.transitive_loop_depth)
                .then_with(|| b.linear_scan_in_loop.cmp(&a.linear_scan_in_loop))
                .then_with(|| {
                    b.cognitive_load
                        .partial_cmp(&a.cognitive_load)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        algorithmic_hotspots.truncate(15);

        let clusters = self.graph.detect_communities();

        ArchitectureReport {
            total_nodes: all_nodes.len(),
            total_edges: self.graph.edge_count(),
            languages,
            node_counts_by_kind,
            clusters,
            entry_points,
            top_pagerank_hubs: hubs,
            algorithmic_hotspots,
        }
    }
}

/// Split `camelCase`, `PascalCase`, `snake_case`, and whitespace into lowercase tokens.
pub fn tokenize_identifier(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = input.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            continue;
        }
        if c.is_uppercase() && !current.is_empty() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
            if prev_lower || next_lower {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::*;
    use std::path::PathBuf;

    fn make_fn_node(file: &str, name: &str) -> ChronosNode {
        let id = NodeId::new(file, name, &NodeKind::Function);
        ChronosNode::new(
            id,
            NodeKind::Function,
            name,
            format!("{file}::{name}"),
            SourceLocation {
                file: PathBuf::from(file),
                start_line: 1,
                start_col: 0,
                end_line: 10,
                end_col: 1,
            },
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

        graph.hydrate_node(
            &id,
            TelemetrySnapshot {
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
            },
        );

        let qe = GraphQueryEngine::new(&graph);
        let result = qe.find_bottlenecks();
        assert_eq!(result.nodes_found, 1);
        assert_eq!(result.details[0].name, "slowFunction");
        assert_eq!(result.details[0].risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_tokenize_camel_and_snake_case() {
        let toks = tokenize_identifier("processOrdersWithUserInfo_v2");
        assert_eq!(
            toks,
            vec!["process", "orders", "with", "user", "info", "v2"]
        );
    }

    #[test]
    fn test_bm25_search_and_architecture() {
        let mut graph = ChronosGraph::new();
        let mut n1 = make_fn_node("src/orders.ts", "processOrdersWithUserInfo");
        n1.heuristics.loop_depth = 2;
        n1.heuristics.transitive_loop_depth = 2;
        n1.heuristics.linear_scan_in_loop = 1;
        n1.heuristics.compute_cognitive_load();
        let id1 = n1.id.clone();
        let n2 = make_fn_node("src/users.py", "fetch_user_profile");
        let id2 = n2.id.clone();

        graph.add_node(n1);
        graph.add_node(n2);
        graph.add_edge(&id1, &id2, ChronosEdge::new(EdgeKind::Calls)).unwrap();

        let qe = GraphQueryEngine::new(&graph);
        let res = qe.search_graph(&GraphSearchRequest {
            query: Some("order user".to_string()),
            limit: 10,
            ..Default::default()
        });
        assert!(!res.results.is_empty());
        assert_eq!(res.results[0].node.name, "processOrdersWithUserInfo");

        let arch = qe.get_architecture(None);
        assert_eq!(arch.total_nodes, 2);
        assert_eq!(arch.algorithmic_hotspots.len(), 1);
        assert!(!arch.clusters.is_empty());
    }
}
