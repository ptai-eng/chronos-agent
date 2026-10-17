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
    assert!(report.killed_mutants <= report.total_mutants);

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

// ── v1.2 Tests: Call Edges, PageRank, CoverageReport, Multi-Language AST ─────

#[test]
fn v1_2_typescript_call_and_test_edges_and_pagerank() {
    let mut parser = TypeScriptParser::new().expect("TS parser");
    let src = r#"
        export function validateOrder(amount: number): boolean {
            return amount > 0;
        }

        export function processOrder(amount: number): boolean {
            return validateOrder(amount);
        }

        it("validates and processes order", () => {
            expect(processOrder(100)).toBe(true);
        });
    "#;
    let parsed = parser.parse_source(&PathBuf::from("src/order.spec.ts"), src).expect("parse TS");
    let mut graph = ChronosGraph::new();
    let (nodes, edges) = graph.ingest_parsed_module(&parsed);

    assert_eq!(nodes, 3, "2 functions + 1 test case");
    assert!(edges >= 2, "processOrder -> validateOrder (Calls) + test -> processOrder (Tests)");

    let pr = graph.compute_pagerank(20, 0.85);
    assert_eq!(pr.len(), 3);

    let validate_id = graph.nodes_by_kind(&NodeKind::Function)
        .into_iter()
        .find(|n| n.name == "validateOrder")
        .unwrap()
        .id
        .clone();
    let process_id = graph.nodes_by_kind(&NodeKind::Function)
        .into_iter()
        .find(|n| n.name == "processOrder")
        .unwrap()
        .id
        .clone();

    assert!(
        pr[&validate_id] > pr[&process_id],
        "callee validateOrder should have higher PageRank than caller processOrder"
    );

    let analyzer = ImpactAnalyzer::new(&graph);
    let report = analyzer.analyze(&validate_id, false);
    assert_eq!(report.direct_callers_count, 1);
    assert!(report.coverage.call_edges_available);
    assert_eq!(report.coverage.completeness, "partial"); // telemetry is cold
    assert!(report.confidence_interval.1 > report.confidence_interval.0);
}

#[test]
fn v1_2_python_and_rust_parsers_extract_call_edges() {
    use chronos_core::ast::{py_parser::PythonParser, rs_parser::RustParser};

    // Python call graph + test linkage
    let mut py = PythonParser::new().expect("Py parser");
    let py_src = r#"
def compute_tax(price):
    if price > 100:
        return price * 0.2
    return price * 0.1

def checkout(price):
    return price + compute_tax(price)

def test_checkout():
    assert checkout(50) == 55
"#;
    let py_parsed = py.parse_str(py_src, &PathBuf::from("app/billing.py")).expect("parse py");
    let mut py_graph = ChronosGraph::new();
    let (_py_nodes, py_edges) = py_graph.ingest_parsed_module(&py_parsed);
    assert!(py_edges >= 2, "Python should link checkout->compute_tax and test_checkout->checkout");

    // Rust call graph + test linkage
    let mut rs = RustParser::new().expect("Rs parser");
    let rs_src = r#"
pub fn hash_token(raw: &str) -> usize {
    if raw.is_empty() {
        return 0;
    }
    raw.len()
}

pub fn verify_session(token: &str) -> bool {
    let h = hash_token(token);
    h > 0
}

#[test]
fn test_verify_session() {
    assert!(verify_session("abc"));
}
"#;
    let rs_parsed = rs.parse_str(rs_src, &PathBuf::from("src/auth.rs")).expect("parse rs");
    let mut rs_graph = ChronosGraph::new();
    let (_rs_nodes, rs_edges) = rs_graph.ingest_parsed_module(&rs_parsed);
    assert!(rs_edges >= 2, "Rust should link verify_session->hash_token and test_verify_session->verify_session");
}

#[test]
fn v1_4_cross_boundary_frontend_backend_route_stitching() {
    use chronos_core::ast::{py_parser::PythonParser, ts_parser::TypeScriptParser};

    // Parse Frontend TS FIRST (reverse order of definition)
    let mut ts = TypeScriptParser::new().expect("TS parser");
    let ts_src = r#"
export async function loadCustomerDashboard(userId: string) {
    const res = await fetch(`/api/v1/users/${userId}`);
    return res;
}
"#;
    let ts_mod = ts
        .parse_source(&PathBuf::from("frontend/dashboard.ts"), ts_src)
        .expect("parse ts");

    // Parse Backend Python FastAPI SECOND
    let mut py = PythonParser::new().expect("Py parser");
    let py_src = r#"
@app.get("/api/v1/users/{id}")
def get_user_endpoint(id: str):
    return {"id": id}
"#;
    let py_mod = py
        .parse_str(py_src, &PathBuf::from("backend/routes.py"))
        .expect("parse py");

    let mut graph = ChronosGraph::new();
    let (_ts_nodes, ts_edges) = graph.ingest_parsed_module(&ts_mod);
    assert_eq!(ts_edges, 0, "Route not yet in graph when TS is ingested first");
    let (_py_nodes, stitched) = graph.ingest_parsed_module(&py_mod);
    assert!(stitched >= 1, "Should stitch deferred cross-language HTTP route calls");

    let route_node = graph
        .nodes_by_kind(&NodeKind::ApiRoute)
        .into_iter()
        .find(|n| n.name.contains("/api/v1/users/"))
        .expect("Backend ApiRoute /api/v1/users/{id} should exist");

    let callers = graph.callers_of(&route_node.id);
    assert!(
        callers.iter().any(|c| c.name == "loadCustomerDashboard"),
        "Frontend function loadCustomerDashboard should be linked as a caller of backend ApiRoute"
    );
}

#[test]
fn v1_4_git_temporal_co_change_coupling_surfaces_in_blast_radius() {
    use chronos_core::graph::schema::{ChronosEdge, ChronosNode, EdgeKind, NodeId, SourceLocation};

    let mut graph = ChronosGraph::new();
    let file_a_id = NodeId::new("src/schema.sql", "schema.sql", &NodeKind::File);
    let file_b_id = NodeId::new("src/models/user.rs", "user.rs", &NodeKind::File);

    graph.add_node(ChronosNode::new(
        file_a_id.clone(),
        NodeKind::File,
        "schema.sql",
        "src/schema.sql",
        SourceLocation {
            file: PathBuf::from("src/schema.sql"),
            start_line: 1,
            start_col: 0,
            end_line: 20,
            end_col: 0,
        },
    ));
    graph.add_node(ChronosNode::new(
        file_b_id.clone(),
        NodeKind::File,
        "user.rs",
        "src/models/user.rs",
        SourceLocation {
            file: PathBuf::from("src/models/user.rs"),
            start_line: 1,
            start_col: 0,
            end_line: 50,
            end_col: 0,
        },
    ));

    let mut edge = ChronosEdge::new(EdgeKind::CoChangesWith);
    edge.call_site_line = Some(85); // 85% historical co-change confidence
    graph.add_edge(&file_a_id, &file_b_id, edge).unwrap();

    let analyzer = ImpactAnalyzer::new(&graph);
    let report = analyzer.analyze(&file_a_id, false);
    assert!(
        report
            .affected_nodes
            .iter()
            .any(|n| n.node_name == "user.rs" && n.reason.contains("85% confidence")),
        "BlastRadiusReport should include git co-change partner user.rs with 85% confidence"
    );
}

#[test]
fn v1_4_offline_telemetry_hydration_completes_coverage() {
    use chronos_core::telemetry::{hydration::hydrate_graph, profile_loader::parse_profile_str};

    let mut parser = TypeScriptParser::new().unwrap();
    let src = r#"
export function verifyJwt(token: string): boolean {
    return token.length > 10;
}
export function handleLogin(token: string): boolean {
    return verifyJwt(token);
}
"#;
    let parsed = parser.parse_source(&PathBuf::from("src/auth.ts"), src).unwrap();
    let mut graph = ChronosGraph::new();
    graph.ingest_parsed_module(&parsed);

    let folded = "main;handleLogin;verifyJwt 250\nmain;handleLogin 50\n";
    let session = parse_profile_str(folded).expect("Should parse folded stacks");
    let hydration = hydrate_graph(&mut graph, &session);
    assert_eq!(hydration.hydrated_count, 2);
    assert!(graph.is_hydrated());

    let verify_id = graph
        .nodes_by_kind(&NodeKind::Function)
        .into_iter()
        .find(|n| n.name == "verifyJwt")
        .unwrap()
        .id
        .clone();
    let analyzer = ImpactAnalyzer::new(&graph);
    let report = analyzer.analyze(&verify_id, false);
    assert!(report.coverage.telemetry_hydrated);
    assert_eq!(report.coverage.completeness, "complete");
}

