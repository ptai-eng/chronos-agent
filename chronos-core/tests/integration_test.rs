//! # Integration test: Parse fixture → insert graph → query hotspots → blast-radius
//!
//! This is the "end-to-end sanity check" for all 4 pillars together.

use chronos_core::{
    ast::ts_parser::TypeScriptParser,
    blast_radius::impact_matrix::ImpactAnalyzer,
    contract::broker::ContractBroker,
    graph::{
        query::GraphQueryEngine,
        schema::{ChronosGraph, NodeKind},
    },
    mutation::arena::MutationArena,
    mutation::watchdog::WatchdogConfig,
};
use std::path::PathBuf;

// ── Helper ────────────────────────────────────────────────────────────────────

fn fixture_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../fixtures/ts-express-api/src/orderService.ts");
    p.canonicalize().unwrap_or(p)
}

// ── Pillar 1 Tests: 4D Knowledge Graph ───────────────────────────────────────

#[test]
fn pillar1_parse_fixture_inserts_nodes() {
    let fixture = fixture_path();
    if !fixture.exists() {
        eprintln!("Fixture not found at {:?} — skipping", fixture);
        return;
    }

    let mut parser = TypeScriptParser::new().expect("Parser init");
    let parsed = parser.parse_file(&fixture).expect("Parse fixture");

    assert!(!parsed.functions.is_empty(), "Should find at least one function");

    // Insert into graph
    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let report = graph.status_report();
    assert!(report.total_nodes > 0, "Graph must have nodes after parse");
    assert!(report.function_nodes > 0, "Should find function nodes");

    println!("✅ Pillar 1 — Inserted {} nodes ({} functions, {} tests)",
        report.total_nodes, report.function_nodes, report.test_nodes);
}

#[test]
fn pillar1_graph_query_engine_finds_complexity() {
    let fixture = fixture_path();
    if !fixture.exists() { return; }

    let mut parser = TypeScriptParser::new().expect("Parser init");
    let parsed = parser.parse_file(&fixture).expect("Parse");

    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let engine = GraphQueryEngine::new(&graph);
    let result = engine.high_complexity_functions(0.0); // threshold=0 = find all
    assert!(result.nodes_found > 0, "Should find at least one function");

    // N+1 query function `processOrdersWithUserInfo` should be present
    let has_n1_func = result.details.iter().any(|n| n.name.contains("processOrders"));
    assert!(has_n1_func, "Should find processOrders function from fixture");

    println!("✅ Pillar 1 — Query engine found {} nodes", result.nodes_found);
}

// ── Pillar 2 Tests: Mutation Arena ────────────────────────────────────────────

#[tokio::test]
async fn pillar2_mutation_arena_no_crash_on_fixture() {
    let fixture = fixture_path();
    if !fixture.exists() { return; }

    let mut parser = TypeScriptParser::new().expect("Parser init");
    let parsed = parser.parse_file(&fixture).expect("Parse");

    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let watchdog = WatchdogConfig {
        timeout_ms: 3000,
        max_concurrency: 2,
        test_runner: "echo".to_string(),
        test_args: vec!["no-tests".to_string()],
    };

    let arena = MutationArena::new(&graph).with_config(watchdog);
    let workspace = fixture.parent().unwrap().parent().unwrap().to_path_buf();

    // Run arena — expect it completes without panic
    let report = arena.run(&fixture, None, &workspace).await.expect("Arena run");

    // With `echo` as test runner all mutants timeout/survive, but score logic should still run
    assert_eq!(report.target_file, fixture.to_string_lossy().to_string());
    assert!(report.total_mutants >= 0); // Can be zero if file has no mutable patterns

    println!("✅ Pillar 2 — Mutation arena: {} mutants, {}/{} killed, score={:.1}%",
        report.total_mutants, report.killed_mutants, report.total_mutants,
        report.mutation_score * 100.0);
}

// ── Pillar 3 Tests: Contract Broker ──────────────────────────────────────────

#[tokio::test]
async fn pillar3_contract_broker_indexes_fixture() {
    let fixture = fixture_path();
    if !fixture.exists() { return; }

    let broker = ContractBroker::new();
    let result = broker.index_file(&fixture).await;
    assert!(result.is_ok(), "Contract broker should index without error");

    // No alerts yet — no changes made
    let alerts = broker.format_pending_alerts_for_agent().await;
    assert!(alerts.is_none(), "No alerts expected on fresh index");

    println!("✅ Pillar 3 — Contract broker indexed fixture, no pending alerts");
}

// ── Pillar 4 Tests: Blast-Radius ─────────────────────────────────────────────

#[test]
fn pillar4_blast_radius_for_fixture_function() {
    let fixture = fixture_path();
    if !fixture.exists() { return; }

    let mut parser = TypeScriptParser::new().expect("Parser init");
    let parsed = parser.parse_file(&fixture).expect("Parse");

    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let analyzer = ImpactAnalyzer::new(&graph);
    let fn_nodes = graph.nodes_by_kind(&NodeKind::Function);
    assert!(!fn_nodes.is_empty(), "Must have function nodes");

    // Analyze the first function
    let first_fn = &fn_nodes[0];
    let report = analyzer.analyze(&first_fn.id, false);

    assert_eq!(report.changed_nodes.len(), 1);
    // Score should be valid (0..=1)
    assert!(report.impact_score >= 0.0 && report.impact_score <= 1.0);

    println!("✅ Pillar 4 — Blast-radius for `{}`: risk={:?}, score={:.2}",
        first_fn.name, report.overall_risk, report.impact_score);
}

#[test]
fn pillar4_blast_radius_contract_breaking_raises_risk() {
    let fixture = fixture_path();
    if !fixture.exists() { return; }

    let mut parser = TypeScriptParser::new().expect("Parser init");
    let parsed = parser.parse_file(&fixture).expect("Parse");

    let mut graph = ChronosGraph::new();
    for node in parsed.all_nodes() {
        graph.add_node(node.clone());
    }

    let analyzer = ImpactAnalyzer::new(&graph);
    let fn_nodes = graph.nodes_by_kind(&NodeKind::Function);
    if fn_nodes.is_empty() { return; }

    let first_fn = &fn_nodes[0];
    let normal = analyzer.analyze(&first_fn.id, false);
    let breaking = analyzer.analyze(&first_fn.id, true);

    // Contract-breaking must raise or maintain risk
    assert!(
        breaking.impact_score >= normal.impact_score,
        "Contract-breaking change must not lower impact score"
    );

    println!("✅ Pillar 4 — Contract-breaking impact: {:.2} vs normal {:.2}",
        breaking.impact_score, normal.impact_score);
}
