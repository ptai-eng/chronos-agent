//! # Graph-Guided Test Selector
//!
//! Uses the 4D Knowledge Graph to select ONLY the tests that actually
//! exercise the mutated code path.
//!
//! ## Key insight
//! Instead of running the full test suite for each mutant (O(n*tests)),
//! we trace backwards through the call graph to find the minimal set
//! of tests that call the mutated function. This reduces test execution
//! from minutes to seconds.

use crate::graph::schema::{ChronosGraph, NodeId, NodeKind};
use tracing::debug;

/// Result of test selection for a specific mutant target
#[derive(Debug)]
pub struct TestSelection {
    pub target_node_id: NodeId,
    pub selected_tests: Vec<SelectedTest>,
    pub selection_strategy: SelectionStrategy,
}

#[derive(Debug)]
pub struct SelectedTest {
    pub test_name: String,
    pub test_file: String,
    pub test_line: usize,
    pub coverage_confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectionStrategy {
    /// Found tests via 4D graph traversal (precise)
    GraphTraced,
    /// Fell back to filename-based heuristic (module name matching)
    FilenameFallback,
    /// No coverage information — must run all tests
    RunAll,
}

/// Select the minimal set of test cases that cover a target function.
///
/// Strategy:
/// 1. Try graph traversal: walk inbound CALLS edges to find TestCase nodes
/// 2. Fallback: look for test files matching the source file's module name
/// 3. Last resort: indicate RunAll
pub fn select_covering_tests<'g>(
    graph: &'g ChronosGraph,
    target_id: &NodeId,
    max_tests: usize,
) -> TestSelection {
    // Strategy 1: Graph traversal
    let covering_tests = graph.find_covering_tests(target_id);

    if !covering_tests.is_empty() {
        let selected: Vec<SelectedTest> = covering_tests
            .into_iter()
            .take(max_tests)
            .map(|n| SelectedTest {
                test_name: n.name.clone(),
                test_file: n.location.file.to_string_lossy().to_string(),
                test_line: n.location.start_line,
                coverage_confidence: 0.9, // High confidence via graph
            })
            .collect();

        debug!(
            "Graph-guided selection: {} tests for {:?}",
            selected.len(),
            target_id
        );

        return TestSelection {
            target_node_id: target_id.clone(),
            selected_tests: selected,
            selection_strategy: SelectionStrategy::GraphTraced,
        };
    }

    // Strategy 2: Look at all test nodes and find those with likely coverage
    // based on naming heuristics (e.g., "user.test.ts" covers "user.ts" functions)
    let target_name = graph.get_node(target_id)
        .map(|n| n.name.as_str())
        .unwrap_or("")
        .to_lowercase();

    let test_nodes = graph.nodes_by_kind(&NodeKind::TestCase);
    let fuzzy_matches: Vec<SelectedTest> = test_nodes
        .into_iter()
        .filter(|n| {
            let test_name_lower = n.name.to_lowercase();
            let test_file_lower = n.location.file.to_string_lossy().to_lowercase();
            // Match if test name or file mentions the target function name
            test_name_lower.contains(&target_name) || test_file_lower.contains(&target_name)
        })
        .take(max_tests)
        .map(|n| SelectedTest {
            test_name: n.name.clone(),
            test_file: n.location.file.to_string_lossy().to_string(),
            test_line: n.location.start_line,
            coverage_confidence: 0.5, // Lower confidence via name heuristic
        })
        .collect();

    if !fuzzy_matches.is_empty() {
        debug!(
            "Filename heuristic: {} tests for {:?}",
            fuzzy_matches.len(),
            target_id
        );
        return TestSelection {
            target_node_id: target_id.clone(),
            selected_tests: fuzzy_matches,
            selection_strategy: SelectionStrategy::FilenameFallback,
        };
    }

    // Strategy 3: No coverage info, must run all
    debug!("No coverage found for {:?} — will run all tests", target_id);
    TestSelection {
        target_node_id: target_id.clone(),
        selected_tests: vec![],
        selection_strategy: SelectionStrategy::RunAll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::*;
    use std::path::PathBuf;

    fn make_graph_with_test_coverage() -> (ChronosGraph, NodeId) {
        let mut graph = ChronosGraph::new();
        
        // Create function node
        let fn_id = NodeId::new("src/user.ts", "getUser", &NodeKind::Function);
        let fn_node = ChronosNode::new(
            fn_id.clone(),
            NodeKind::Function,
            "getUser",
            "UserService::getUser",
            SourceLocation { file: PathBuf::from("src/user.ts"), start_line: 5, start_col: 0, end_line: 10, end_col: 1 },
        );
        graph.add_node(fn_node);

        // Create test node
        let test_id = NodeId::new("tests/user.test.ts", "it(should return user by id)", &NodeKind::TestCase);
        let test_node = ChronosNode::new(
            test_id.clone(),
            NodeKind::TestCase,
            "it(should return user by id)",
            "tests/user.test.ts::it(should return user by id)",
            SourceLocation { file: PathBuf::from("tests/user.test.ts"), start_line: 5, start_col: 0, end_line: 12, end_col: 1 },
        );
        graph.add_node(test_node);

        // Create Tests edge: test → getUser
        graph.add_edge(&test_id, &fn_id, ChronosEdge::new(EdgeKind::Tests)).ok();

        (graph, fn_id)
    }

    #[test]
    fn test_graph_guided_selection() {
        let (graph, fn_id) = make_graph_with_test_coverage();
        let selection = select_covering_tests(&graph, &fn_id, 10);
        
        assert_eq!(selection.selection_strategy, SelectionStrategy::GraphTraced,
            "Should use graph-guided selection when Tests edges exist");
        assert!(!selection.selected_tests.is_empty(),
            "Should find the covering test");
        assert!(selection.selected_tests[0].coverage_confidence > 0.8);
    }

    #[test]
    fn test_fallback_to_run_all_when_no_tests() {
        let mut graph = ChronosGraph::new();
        let fn_id = NodeId::new("src/obscure.ts", "weirdFunction", &NodeKind::Function);
        let fn_node = ChronosNode::new(
            fn_id.clone(),
            NodeKind::Function,
            "weirdFunction",
            "obscure::weirdFunction",
            SourceLocation { file: PathBuf::from("src/obscure.ts"), start_line: 1, start_col: 0, end_line: 5, end_col: 1 },
        );
        graph.add_node(fn_node);

        let selection = select_covering_tests(&graph, &fn_id, 10);
        assert_eq!(selection.selection_strategy, SelectionStrategy::RunAll);
    }
}
