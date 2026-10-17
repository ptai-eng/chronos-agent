use chronos_core::graph::{
    persistent::PersistentGraph,
    schema::{
        ChronosGraph, ChronosNode, NodeId, NodeKind, SourceLocation,
        StaticHeuristics, TelemetrySnapshot,
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
        contract_signature: None,
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
        location: SourceLocation {
            file: std::path::PathBuf::from("src/it's-utils.ts"),
            start_line: 1,
            start_col: 0,
            end_line: 5,
            end_col: 0,
        },
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
    // Should not panic — tables must be created
    PersistentGraph::open(dir.path()).unwrap();
}

#[test]
fn kuzu_open_idempotent_second_open() {
    let dir = tempfile::tempdir().unwrap();
    // First open creates schema
    PersistentGraph::open(dir.path()).unwrap();
    // Second open must NOT error on "table already exists"
    PersistentGraph::open(dir.path()).unwrap();
}

// ─── Upsert + round-trip ──────────────────────────────────────────────────────

#[test]
fn kuzu_upsert_and_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistentGraph::open(dir.path()).unwrap();

    let node = make_function_node("src/foo.ts", "fooFunc");
    let conn = kuzu::Connection::new(&store.db).unwrap();
    store.upsert_node(&conn, &node).unwrap();

    let mut graph = ChronosGraph::new();
    let count = store.load_into_graph(&mut graph).unwrap();

    // Implementation now fully loads nodes — should be exactly 1
    assert_eq!(count, 1, "exactly one node should be loaded back");
    assert!(
        graph.nodes_by_kind(&NodeKind::Function).iter().any(|n| n.name == "fooFunc"),
        "fooFunc should be present in the graph after load"
    );
}

#[test]
fn kuzu_upsert_multiple_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistentGraph::open(dir.path()).unwrap();

    let conn = kuzu::Connection::new(&store.db).unwrap();
    store.upsert_node(&conn, &make_function_node("src/a.ts", "funcA")).unwrap();
    store.upsert_node(&conn, &make_function_node("src/b.ts", "funcB")).unwrap();
    store.upsert_node(&conn, &make_function_node("src/c.ts", "funcC")).unwrap();

    let mut graph = ChronosGraph::new();
    let count = store.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 3, "all three nodes must be persisted and loaded");
}

// ─── Persistence across restarts ──────────────────────────────────────────────

#[test]
fn kuzu_survives_restart() {
    let dir = tempfile::tempdir().unwrap();

    // Session 1: write
    {
        let store = PersistentGraph::open(dir.path()).unwrap();
        let conn = kuzu::Connection::new(&store.db).unwrap();
        store.upsert_node(&conn, &make_function_node("src/a.ts", "funcA")).unwrap();
    } // DB dropped / closed here

    // Session 2: read back — must not return 0
    {
        let store = PersistentGraph::open(dir.path()).unwrap();
        let mut graph = ChronosGraph::new();
        let count = store.load_into_graph(&mut graph).unwrap();
        assert_eq!(count, 1, "node must survive DB restart");
    }
}

// ─── Upsert idempotency ───────────────────────────────────────────────────────

#[test]
fn kuzu_upsert_same_id_twice_does_not_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistentGraph::open(dir.path()).unwrap();

    let node = make_function_node("src/foo.ts", "fooFunc");
    let conn = kuzu::Connection::new(&store.db).unwrap();
    store.upsert_node(&conn, &node).unwrap();

    // Now update the SAME node
    let mut updated = node.clone();
    updated.heuristics.cognitive_load_score = 999.0;
    store.upsert_node(&conn, &updated).unwrap();

    let mut graph = ChronosGraph::new();
    let count = store.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 1, "MERGE must not create duplicates");
}

// ─── Injection safety ─────────────────────────────────────────────────────────

#[test]
fn kuzu_upsert_handles_single_quote_in_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistentGraph::open(dir.path()).unwrap();

    // This must NOT panic or produce a Cypher syntax error
    let node = make_tricky_node();
    let conn = kuzu::Connection::new(&store.db).unwrap();
    store.upsert_node(&conn, &node).expect(
        "upsert_node should not crash with single-quote in symbol name"
    );

    let mut graph = ChronosGraph::new();
    let count = store.load_into_graph(&mut graph).unwrap();
    assert_eq!(count, 1, "tricky node must be persisted and loaded back");
}

// ─── sync_from_graph ──────────────────────────────────────────────────────────

#[test]
fn kuzu_sync_from_graph_bulk() {
    

    let dir = tempfile::tempdir().unwrap();
    let store = PersistentGraph::open(dir.path()).unwrap();

    let mut in_graph = ChronosGraph::new();
    in_graph.add_node(make_function_node("src/x.ts", "funcX"));
    in_graph.add_node(make_function_node("src/y.ts", "funcY"));

    let synced = store.sync_from_graph(&in_graph).unwrap();
    assert_eq!(synced, 2, "sync_from_graph must sync all nodes");

    let mut out_graph = ChronosGraph::new();
    let loaded = store.load_into_graph(&mut out_graph).unwrap();
    assert_eq!(loaded, 2, "loaded count must match synced count");
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

    // Session 1: populate nodes + CALLS and TESTS edges, then sync to KuzuDB
    {
        let store = PersistentGraph::open(dir.path()).unwrap();
        let mut graph = ChronosGraph::new();
        graph.add_node(caller_node);
        graph.add_node(callee_node);
        graph.add_node(test_node);
        graph.add_edge(&caller_id, &callee_id, ChronosEdge::new(EdgeKind::Calls)).unwrap();
        graph.add_edge(&test_id, &caller_id, ChronosEdge::new(EdgeKind::Tests)).unwrap();

        let synced = store.sync_from_graph(&graph).unwrap();
        assert_eq!(synced, 3, "3 nodes should be synced (along with 2 edges)");
    }

    // Session 2: reopen KuzuDB and verify edges are restored
    {
        let store = PersistentGraph::open(dir.path()).unwrap();
        let mut restored = ChronosGraph::new();
        let loaded_nodes = store.load_into_graph(&mut restored).unwrap();
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
    let store = PersistentGraph::open(dir.path()).unwrap();

    let mut graph = ChronosGraph::new();
    graph.add_node(make_function_node("src/hot.ts", "deleteUserSession"));
    store.sync_from_graph(&graph).unwrap();

    // Query complexity columns and verify string literal 'deleteUserSession' is allowed
    let res = store
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

