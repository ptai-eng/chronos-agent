use chronos_core::graph::{
    persistent::GraphRepository,
    schema::{
        ChronosGraph, ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics,
        TelemetrySnapshot,
    },
};

fn make_function_node(file: &str, name: &str) -> ChronosNode {
    ChronosNode {
        id: NodeId(format!("{}::Function::{}", file, name)),
        name: name.to_string(),
        qualified_name: format!("module::{}", name),
        kind: NodeKind::Function,
        location: SourceLocation {
            file: std::path::PathBuf::from(file),
            start_line: 1,
            start_col: 0,
            end_line: 10,
            end_col: 1,
            start_byte: 0,
            end_byte: 120,
        },
        heuristics: StaticHeuristics {
            cognitive_load_score: 5.0,
            cyclomatic_complexity: 2,
            fan_in: 1,
            fan_out: 2,
            param_count: 1,
            sloc: 10,
            loop_count: 1,
            loop_depth: 2,
            transitive_loop_depth: 3,
            linear_scan_in_loop: 1,
            alloc_in_loop: 0,
            is_recursive: false,
        },
        telemetry: Some(TelemetrySnapshot {
            captured_at: chrono::Utc::now(),
            exec_time_ms_p50: 1.0,
            exec_time_ms_p95: 1.5,
            exec_time_ms_p99: 2.0,
            alloc_delta_kb: 0.0,
            invocation_count: 10,
            cpu_percent_self: 5.0,
            is_hot_path: false,
            is_bottleneck: false,
            bottleneck_reason: None,
        }),
        contract_signature: Some("fn(x: number) -> number".to_string()),
        contract_mutated: false,
    }
}

/// Nodes with names/paths containing special characters to test injection safety.
fn make_tricky_node() -> ChronosNode {
    ChronosNode {
        id: NodeId("src/it's-utils.ts::Function::it's_a_func".to_string()),
        name: "it's_a_func".to_string(),
        qualified_name: "module::it's_a_func".to_string(),
        kind: NodeKind::Function,
        location: SourceLocation::lines("src/it's-utils.ts", 1, 5),
        heuristics: StaticHeuristics::default(),
        telemetry: None,
        contract_signature: None,
        contract_mutated: false,
    }
}

// ─── Schema creation ──────────────────────────────────────────────────────────

#[test]
fn kuzu_open_creates_schema() {
    let dir = tempfile::tempdir().unwrap();
    GraphRepository::open(dir.path()).unwrap();
}

#[test]
fn kuzu_open_idempotent_second_open() {
    let dir = tempfile::tempdir().unwrap();
    GraphRepository::open(dir.path()).unwrap();
    GraphRepository::open(dir.path()).unwrap();
}

// ─── Upsert & load round-trip ─────────────────────────────────────────────────

#[test]
fn kuzu_upsert_and_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let node = make_function_node("src/foo.ts", "fooFunc");
    repo.upsert_single_node(&node).unwrap();

    let mut graph = ChronosGraph::new();
    let count = repo.load_into_graph(&mut graph).unwrap();

    assert_eq!(count, 1, "exactly one node should be loaded back");
    let loaded = graph
        .nodes_by_kind(&NodeKind::Function)
        .into_iter()
        .find(|n| n.name == "fooFunc")
        .expect("fooFunc should be present after load");
    assert_eq!(loaded.qualified_name, "module::fooFunc");
    assert_eq!(
        loaded.contract_signature.as_deref(),
        Some("fn(x: number) -> number")
    );
    assert_eq!(loaded.location.start_byte, 0);
    assert_eq!(loaded.location.end_byte, 120);
}

#[test]
fn kuzu_upsert_multiple_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    repo.upsert_single_node(&make_function_node("src/a.ts", "funcA")).unwrap();
    repo.upsert_single_node(&make_function_node("src/b.ts", "funcB")).unwrap();
    repo.upsert_single_node(&make_function_node("src/c.ts", "funcC")).unwrap();

    let mut graph = ChronosGraph::new();
    let count = repo.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 3, "all three nodes must be persisted and loaded");
}

// ─── Persistence across restarts & BUG-03 lossless updates ────────────────────

#[test]
fn kuzu_survives_restart() {
    let dir = tempfile::tempdir().unwrap();

    {
        let repo = GraphRepository::open(dir.path()).unwrap();
        repo.upsert_single_node(&make_function_node("src/a.ts", "funcA")).unwrap();
    }

    {
        let repo = GraphRepository::open(dir.path()).unwrap();
        let mut graph = ChronosGraph::new();
        let count = repo.load_into_graph(&mut graph).unwrap();
        assert_eq!(count, 1, "node must survive DB restart");
    }
}

#[test]
fn kuzu_upsert_updates_shifted_start_line_sloc_and_params_on_match() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let node = make_function_node("src/foo.ts", "fooFunc");
    repo.upsert_single_node(&node).unwrap();

    let mut shifted = node.clone();
    shifted.location.start_line = 42;
    shifted.location.end_line = 75;
    shifted.location.start_byte = 900;
    shifted.location.end_byte = 1400;
    shifted.heuristics.param_count = 4;
    shifted.heuristics.sloc = 33;
    shifted.heuristics.cognitive_load_score = 99.0;
    shifted.contract_signature = Some("fn(a: i32, b: i32) -> bool".to_string());
    repo.upsert_single_node(&shifted).unwrap();

    let mut graph = ChronosGraph::new();
    let count = repo.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 1, "MERGE must not create duplicates");
    let loaded = graph.get_node(&node.id).unwrap();
    assert_eq!(loaded.location.start_line, 42, "BUG-03: ON MATCH SET must update start_line");
    assert_eq!(loaded.location.end_line, 75);
    assert_eq!(loaded.location.start_byte, 900);
    assert_eq!(loaded.location.end_byte, 1400);
    assert_eq!(loaded.heuristics.param_count, 4);
    assert_eq!(loaded.heuristics.sloc, 33);
    assert_eq!(
        loaded.contract_signature.as_deref(),
        Some("fn(a: i32, b: i32) -> bool")
    );
}

// ─── Injection safety ─────────────────────────────────────────────────────────

#[test]
fn kuzu_upsert_handles_single_quote_in_name() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let node = make_tricky_node();
    repo.upsert_single_node(&node)
        .expect("upsert should not crash with single-quote in symbol name");

    let mut graph = ChronosGraph::new();
    let count = repo.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 1, "tricky node must be persisted and loaded back");
}

// ─── sync_from_graph & sync_file_delta ────────────────────────────────────────

#[test]
fn kuzu_sync_from_graph_bulk() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let mut in_graph = ChronosGraph::new();
    in_graph.add_node(make_function_node("src/x.ts", "funcX"));
    in_graph.add_node(make_function_node("src/y.ts", "funcY"));

    let synced = repo.sync_from_graph(&in_graph).unwrap();
    assert_eq!(synced, 2, "sync_from_graph must sync all nodes");

    let mut out_graph = ChronosGraph::new();
    let loaded = repo.load_into_graph(&mut out_graph).unwrap();
    assert_eq!(loaded, 2, "loaded count must match synced count");
}

#[test]
fn kuzu_sync_file_delta_evicts_and_updates_single_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let mut graph = ChronosGraph::new();
    graph.add_node(make_function_node("src/a.ts", "staleFn"));
    graph.add_node(make_function_node("src/b.ts", "untouchedFn"));
    repo.sync_from_graph(&graph).unwrap();

    // Update only src/a.ts in memory: remove staleFn, add freshFn
    graph.remove_nodes_for_file(std::path::Path::new("src/a.ts"));
    graph.add_node(make_function_node("src/a.ts", "freshFn"));
    let synced = repo
        .sync_file_delta(&graph, std::path::Path::new("src/a.ts"), Some("deadbeef"))
        .unwrap();
    assert_eq!(synced, 1);

    let mut reloaded = ChronosGraph::new();
    repo.load_into_graph(&mut reloaded).unwrap();
    assert!(!reloaded.all_nodes().iter().any(|n| n.name == "staleFn"));
    assert!(reloaded.all_nodes().iter().any(|n| n.name == "freshFn"));
    assert!(reloaded.all_nodes().iter().any(|n| n.name == "untouchedFn"));
}

#[test]
fn kuzu_persists_and_restores_edges_across_restart() {
    use chronos_core::graph::schema::{ChronosEdge, EdgeKind};

    let dir = tempfile::tempdir().unwrap();
    let caller_node = make_function_node("src/api.ts", "handleOrder");
    let callee_node = make_function_node("src/service.ts", "saveOrder");
    let caller_id = caller_node.id.clone();
    let callee_id = callee_node.id.clone();

    let mut test_node = make_function_node("src/order.test.ts", "test_handleOrder");
    test_node.id = NodeId("src/order.test.ts::TestCase::test_handleOrder".to_string());
    test_node.kind = NodeKind::TestCase;
    let test_id = test_node.id.clone();

    {
        let repo = GraphRepository::open(dir.path()).unwrap();
        let mut graph = ChronosGraph::new();
        graph.add_node(caller_node);
        graph.add_node(callee_node);
        graph.add_node(test_node);
        graph.add_edge(&caller_id, &callee_id, ChronosEdge::new(EdgeKind::Calls)).unwrap();
        graph.add_edge(&test_id, &caller_id, ChronosEdge::new(EdgeKind::Tests)).unwrap();

        let synced = repo.sync_from_graph(&graph).unwrap();
        assert_eq!(synced, 3, "3 nodes should be synced (along with 2 edges)");
    }

    {
        let repo = GraphRepository::open(dir.path()).unwrap();
        let mut restored = ChronosGraph::new();
        let loaded_nodes = repo.load_into_graph(&mut restored).unwrap();
        assert_eq!(loaded_nodes, 3, "all 3 nodes must be restored");

        let status = restored.status_report();
        assert_eq!(status.total_edges, 2, "both CALLS and TESTS edges must survive restart");

        let callers = restored.callers_of(&callee_id);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "handleOrder");

        let tests = restored.tests_for(&caller_id);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "test_handleOrder");
    }
}

#[test]
fn kuzu_readonly_cypher_query_and_complexity_columns() {
    let dir = tempfile::tempdir().unwrap();
    let repo = GraphRepository::open(dir.path()).unwrap();

    let mut graph = ChronosGraph::new();
    graph.add_node(make_function_node("src/hot.ts", "deleteUserSession"));
    repo.sync_from_graph(&graph).unwrap();

    let res = repo
        .execute_readonly_cypher(
            "MATCH (f:ChronosNode) WHERE f.name = 'deleteUserSession' AND f.transitive_loop_depth >= 2 RETURN f.name, f.transitive_loop_depth, f.linear_scan_in_loop",
            100,
        )
        .expect("Read-only Cypher query should succeed");

    assert_eq!(res.total_rows, 1);
    assert_eq!(res.rows[0][0], serde_json::json!("deleteUserSession"));
    assert_eq!(res.rows[0][1], serde_json::json!(3));
    assert_eq!(res.rows[0][2], serde_json::json!(1));
}

#[test]
fn kuzu_readonly_cypher_blocks_mutations() {
    use chronos_core::graph::persistent::validate_readonly_cypher;

    assert!(validate_readonly_cypher("MATCH (n:ChronosNode) DELETE n").is_err());
    assert!(validate_readonly_cypher("CREATE (n:ChronosNode {id: 'x'})").is_err());
    assert!(validate_readonly_cypher("MATCH (n:ChronosNode) SET n.name = 'pwn' RETURN n").is_err());
    assert!(validate_readonly_cypher("COPY ChronosNode FROM 'secret.csv'").is_err());
    assert!(validate_readonly_cypher("MATCH (n) RETURN n; DROP TABLE ChronosNode").is_err());
    assert!(
        validate_readonly_cypher("MATCH (n:ChronosNode) WHERE n.name = 'CREATE_OR_DELETE' RETURN n.name")
            .is_ok(),
        "Keywords inside quoted string literals must be allowed"
    );
}

#[test]
fn kuzu_incremental_xxhash64_merkle_sync_skips_unchanged_and_evicts_modified() {
    use chronos_core::ast::dispatcher::LanguageDispatcher;

    let dir = tempfile::tempdir().unwrap();
    let db_dir = dir.path().join(".chronos").join("graph.kuzu");
    std::fs::create_dir_all(db_dir.parent().unwrap()).unwrap();
    let repo = GraphRepository::open(&db_dir).unwrap();

    let file_a = dir.path().join("service_a.ts");
    let file_b = dir.path().join("service_b.ts");
    std::fs::write(&file_a, "export function oldHelper(x: number) { return x + 1; }\n").unwrap();
    std::fs::write(&file_b, "export function stableFunc(y: number) { return y * 2; }\n").unwrap();

    let files = vec![file_a.clone(), file_b.clone()];
    let mut graph = ChronosGraph::new();
    let mut dispatcher = LanguageDispatcher::new().unwrap();

    let stats1 = repo
        .sync_workspace_incremental(&mut graph, &files, &mut dispatcher)
        .unwrap();
    assert_eq!(stats1.re_parsed_files, 2);
    assert_eq!(stats1.unchanged_files, 0);
    assert!(graph.all_nodes().iter().any(|n| n.name == "oldHelper"));

    let stats2 = repo
        .sync_workspace_incremental(&mut graph, &files, &mut dispatcher)
        .unwrap();
    assert_eq!(stats2.re_parsed_files, 0);
    assert_eq!(stats2.unchanged_files, 2);

    std::fs::write(&file_a, "export function newHelper(x: number) { return x + 2; }\n").unwrap();
    let stats3 = repo
        .sync_workspace_incremental(&mut graph, &files, &mut dispatcher)
        .unwrap();
    assert_eq!(stats3.re_parsed_files, 1);
    assert_eq!(stats3.unchanged_files, 1);
    assert!(
        !graph.all_nodes().iter().any(|n| n.name == "oldHelper"),
        "Stale symbol oldHelper must be evicted from in-memory graph"
    );
    assert!(
        graph.all_nodes().iter().any(|n| n.name == "newHelper"),
        "New symbol newHelper must be present"
    );
}
