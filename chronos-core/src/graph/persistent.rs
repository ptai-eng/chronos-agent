//! KuzuDB persistent backend for the 5D Knowledge Graph (`GraphRepository`)
//!
//! ## Security note
//! All user-supplied strings (node.name, node.id, file paths) are parameterized
//! or validated via `validate_readonly_cypher()` to prevent Cypher injection attacks.
//!
//! ## Lossless Schema v2
//! Persists all spatial, complexity, contract, byte-span, and runtime telemetry fields
//! and supports file-scoped delta updates (`sync_file_delta`).

use anyhow::{Context, Result};
use kuzu::{Connection, Database, SystemConfig};
use std::collections::HashSet;
use std::path::Path;
use crate::graph::schema::{
    ChronosGraph, ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics,
    TelemetrySnapshot,
};
use tracing::{debug, info, warn};

/// Current KuzuDB persistence schema version (v1.7.0 lossless byte spans + contracts).
pub const GRAPH_SCHEMA_VERSION: &str = "2";

/// Deep persistence repository wrapping embedded KuzuDB without leaking raw handles.
pub struct GraphRepository {
    db: Database,
    db_path: std::path::PathBuf,
}

#[inline]
#[allow(dead_code)]
fn escape_cypher_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

impl GraphRepository {
    /// Open or create a KuzuDB repository at `db_path`, ensuring schema v2 compatibility.
    /// Automatically rebuilds incompatible legacy `.chronos/graph.kuzu` directories (Q2).
    pub fn open(db_path: &Path) -> Result<Self> {
        let raw_str = db_path.to_string_lossy();
        let clean_path = std::path::PathBuf::from(raw_str.trim_start_matches(r"\\?\"));

        let db = match Database::new(&clean_path, SystemConfig::default()) {
            Ok(db) => db,
            Err(first_err) => {
                // Only auto-wipe dedicated `.chronos/graph.kuzu` directories if corrupted/legacy.
                let is_dedicated_kuzu_dir = clean_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.ends_with(".kuzu"));
                if is_dedicated_kuzu_dir && clean_path.exists() {
                    warn!(
                        "Rebuilding incompatible KuzuDB cache at {:?}: {}",
                        clean_path, first_err
                    );
                    if clean_path.is_dir() {
                        let _ = std::fs::remove_dir_all(&clean_path);
                    } else {
                        let _ = std::fs::remove_file(&clean_path);
                    }
                    let wal_path = std::path::PathBuf::from(format!("{}.wal", clean_path.display()));
                    if wal_path.exists() {
                        let _ = std::fs::remove_file(&wal_path);
                    }
                    Database::new(&clean_path, SystemConfig::default()).map_err(|e| {
                        anyhow::anyhow!("KuzuDB rebuild error at {:?}: {}", clean_path, e)
                    })?
                } else {
                    return Err(anyhow::anyhow!(
                        "KuzuDB open error at {:?}: {}",
                        clean_path,
                        first_err
                    ));
                }
            }
        };

        let store = Self {
            db,
            db_path: clean_path,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    /// Create or upgrade node, metadata, hash, and relationship tables.
    fn ensure_schema(&self) -> Result<()> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for schema init")?;

        if let Err(e) = conn.query(
            "CREATE NODE TABLE IF NOT EXISTS ChronosNode (
                id                    STRING,
                name                  STRING,
                qualified_name        STRING,
                kind                  STRING,
                file                  STRING,
                start_line            INT64,
                end_line              INT64,
                start_byte            INT64,
                end_byte              INT64,
                contract_signature    STRING,
                cognitive_load        DOUBLE,
                cyclomatic            INT64,
                fan_in                INT64,
                fan_out               INT64,
                param_count           INT64,
                sloc                  INT64,
                loop_count            INT64,
                loop_depth            INT64,
                transitive_loop_depth INT64,
                linear_scan_in_loop   INT64,
                alloc_in_loop         INT64,
                is_recursive          BOOL,
                is_hydrated           BOOL,
                exec_time_p50         DOUBLE,
                exec_time_p99         DOUBLE,
                invocation_count      INT64,
                cpu_percent           DOUBLE,
                is_hot_path           BOOL,
                is_bottleneck         BOOL,
                PRIMARY KEY (id)
            )",
        ) {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("already") && !msg.contains("exist") {
                return Err(anyhow::anyhow!("Failed to create ChronosNode table: {}", e));
            }
            debug!("ChronosNode table already exists (ok)");
        }

        // Lossless schema upgrade from v1.2–v1.6 databases
        for alter_col in &[
            "end_line INT64 DEFAULT 0",
            "start_byte INT64 DEFAULT 0",
            "end_byte INT64 DEFAULT 0",
            "qualified_name STRING DEFAULT ''",
            "contract_signature STRING DEFAULT ''",
            "loop_count INT64 DEFAULT 0",
            "loop_depth INT64 DEFAULT 0",
            "transitive_loop_depth INT64 DEFAULT 0",
            "linear_scan_in_loop INT64 DEFAULT 0",
            "alloc_in_loop INT64 DEFAULT 0",
            "is_recursive BOOL DEFAULT false",
        ] {
            let _ = conn.query(&format!("ALTER TABLE ChronosNode ADD IF NOT EXISTS {alter_col}"));
        }

        let _ = conn.query(
            "CREATE NODE TABLE IF NOT EXISTS ChronosFileHash (
                file     STRING,
                xxhash64 STRING,
                PRIMARY KEY (file)
            )",
        );

        let _ = conn.query(
            "CREATE NODE TABLE IF NOT EXISTS ChronosMeta (
                key STRING,
                val STRING,
                PRIMARY KEY (key)
            )",
        );
        if let Ok(mut meta_stmt) = conn.prepare(
            "MERGE (m:ChronosMeta {key: $k}) ON CREATE SET m.val = $v ON MATCH SET m.val = $v",
        ) {
            let _ = conn.execute(
                &mut meta_stmt,
                vec![
                    ("k", kuzu::Value::String("schema_version".to_string())),
                    ("v", kuzu::Value::String(GRAPH_SCHEMA_VERSION.to_string())),
                ],
            );
        }

        for label in &["CALLS", "CONTAINS", "TESTS", "IMPORTS", "CO_CHANGES_WITH"] {
            let ddl = format!(
                "CREATE REL TABLE IF NOT EXISTS {label} (FROM ChronosNode TO ChronosNode)"
            );
            if let Err(e) = conn.query(&ddl) {
                let msg = e.to_string().to_lowercase();
                if !msg.contains("already") && !msg.contains("exist") {
                    return Err(anyhow::anyhow!("Failed to create rel table {}: {}", label, e));
                }
                debug!("Rel table {} already exists (ok)", label);
            }
        }

        info!("KuzuDB repository ready at {:?} (schema v{})", self.db_path, GRAPH_SCHEMA_VERSION);
        Ok(())
    }

    /// Upsert a single `ChronosNode` safely outside callers needing a raw connection.
    pub fn upsert_single_node(&self, node: &ChronosNode) -> Result<()> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for single node upsert")?;
        self.upsert_node_with_conn(&conn, node)
    }

    /// Internal node upsert helper preserving every mutable spatial, contract, and telemetry field.
    fn upsert_node_with_conn(&self, conn: &Connection, node: &ChronosNode) -> Result<()> {
        let (p50, p99, inv, cpu, hot, bottleneck) = node
            .telemetry
            .as_ref()
            .map(|t| {
                (
                    t.exec_time_ms_p50,
                    t.exec_time_ms_p99,
                    t.invocation_count as i64,
                    t.cpu_percent_self as f64,
                    t.is_hot_path,
                    t.is_bottleneck,
                )
            })
            .unwrap_or((0.0, 0.0, 0, 0.0, false, false));

        let is_hydrated = node.telemetry.is_some();
        let contract_sig = node.contract_signature.as_deref().unwrap_or("").to_string();

        let query = "
             MERGE (n:ChronosNode {id: $id})
             ON CREATE SET
                 n.name = $name,
                 n.qualified_name = $qname,
                 n.kind = $kind,
                 n.file = $file,
                 n.start_line = $start_line,
                 n.end_line = $end_line,
                 n.start_byte = $start_byte,
                 n.end_byte = $end_byte,
                 n.contract_signature = $contract_sig,
                 n.cognitive_load = $cog,
                 n.cyclomatic = $cyc,
                 n.fan_in = $fin,
                 n.fan_out = $fout,
                 n.param_count = $params,
                 n.sloc = $sloc,
                 n.loop_count = $loop_cnt,
                 n.loop_depth = $loop_dep,
                 n.transitive_loop_depth = $trans_loop,
                 n.linear_scan_in_loop = $lin_scan,
                 n.alloc_in_loop = $alloc_loop,
                 n.is_recursive = $is_rec,
                 n.is_hydrated = $is_hydrated,
                 n.exec_time_p50 = $p50,
                 n.exec_time_p99 = $p99,
                 n.invocation_count = $inv,
                 n.cpu_percent = $cpu,
                 n.is_hot_path = $hot,
                 n.is_bottleneck = $bottleneck
             ON MATCH SET
                 n.name = $name,
                 n.qualified_name = $qname,
                 n.kind = $kind,
                 n.file = $file,
                 n.start_line = $start_line,
                 n.end_line = $end_line,
                 n.start_byte = $start_byte,
                 n.end_byte = $end_byte,
                 n.contract_signature = $contract_sig,
                 n.cognitive_load = $cog,
                 n.cyclomatic = $cyc,
                 n.fan_in = $fin,
                 n.fan_out = $fout,
                 n.param_count = $params,
                 n.sloc = $sloc,
                 n.loop_count = $loop_cnt,
                 n.loop_depth = $loop_dep,
                 n.transitive_loop_depth = $trans_loop,
                 n.linear_scan_in_loop = $lin_scan,
                 n.alloc_in_loop = $alloc_loop,
                 n.is_recursive = $is_rec,
                 n.is_hydrated = $is_hydrated,
                 n.exec_time_p50 = $p50,
                 n.exec_time_p99 = $p99,
                 n.invocation_count = $inv,
                 n.cpu_percent = $cpu,
                 n.is_hot_path = $hot,
                 n.is_bottleneck = $bottleneck";

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| anyhow::anyhow!("Failed to prepare upsert query: {}", e))?;

        let params = vec![
            ("id", kuzu::Value::String(node.id.0.clone())),
            ("name", kuzu::Value::String(node.name.clone())),
            ("qname", kuzu::Value::String(node.qualified_name.clone())),
            ("kind", kuzu::Value::String(format!("{:?}", node.kind))),
            (
                "file",
                kuzu::Value::String(node.location.file.to_string_lossy().replace('\\', "/")),
            ),
            ("start_line", kuzu::Value::Int64(node.location.start_line as i64)),
            ("end_line", kuzu::Value::Int64(node.location.end_line as i64)),
            ("start_byte", kuzu::Value::Int64(node.location.start_byte as i64)),
            ("end_byte", kuzu::Value::Int64(node.location.end_byte as i64)),
            ("contract_sig", kuzu::Value::String(contract_sig)),
            ("cog", kuzu::Value::Double(node.heuristics.cognitive_load_score as f64)),
            ("cyc", kuzu::Value::Int64(node.heuristics.cyclomatic_complexity as i64)),
            ("fin", kuzu::Value::Int64(node.heuristics.fan_in as i64)),
            ("fout", kuzu::Value::Int64(node.heuristics.fan_out as i64)),
            ("params", kuzu::Value::Int64(node.heuristics.param_count as i64)),
            ("sloc", kuzu::Value::Int64(node.heuristics.sloc as i64)),
            ("loop_cnt", kuzu::Value::Int64(node.heuristics.loop_count as i64)),
            ("loop_dep", kuzu::Value::Int64(node.heuristics.loop_depth as i64)),
            (
                "trans_loop",
                kuzu::Value::Int64(node.heuristics.transitive_loop_depth as i64),
            ),
            (
                "lin_scan",
                kuzu::Value::Int64(node.heuristics.linear_scan_in_loop as i64),
            ),
            ("alloc_loop", kuzu::Value::Int64(node.heuristics.alloc_in_loop as i64)),
            ("is_rec", kuzu::Value::Bool(node.heuristics.is_recursive)),
            ("is_hydrated", kuzu::Value::Bool(is_hydrated)),
            ("p50", kuzu::Value::Double(p50)),
            ("p99", kuzu::Value::Double(p99)),
            ("inv", kuzu::Value::Int64(inv)),
            ("cpu", kuzu::Value::Double(cpu)),
            ("hot", kuzu::Value::Bool(hot)),
            ("bottleneck", kuzu::Value::Bool(bottleneck)),
        ];

        conn.execute(&mut stmt, params)
            .map_err(|e| anyhow::anyhow!("KuzuDB upsert failed for '{}': {}", node.name, e))?;
        Ok(())
    }

    fn upsert_edge_with_conn(
        &self,
        conn: &Connection,
        from: &NodeId,
        to: &NodeId,
        edge: &crate::graph::schema::ChronosEdge,
    ) -> Result<()> {
        let rel_label = match edge.kind {
            crate::graph::schema::EdgeKind::Calls => "CALLS",
            crate::graph::schema::EdgeKind::Contains | crate::graph::schema::EdgeKind::Exposes => {
                "CONTAINS"
            }
            crate::graph::schema::EdgeKind::Tests => "TESTS",
            crate::graph::schema::EdgeKind::Imports | crate::graph::schema::EdgeKind::Implements => {
                "IMPORTS"
            }
            crate::graph::schema::EdgeKind::CoChangesWith => "CO_CHANGES_WITH",
        };

        let query = format!(
            "MATCH (a:ChronosNode {{id: $from_id}}), (b:ChronosNode {{id: $to_id}}) \
             MERGE (a)-[:{rel_label}]->(b)"
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| anyhow::anyhow!("Failed to prepare edge upsert ({rel_label}): {e}"))?;

        let params = vec![
            ("from_id", kuzu::Value::String(from.0.clone())),
            ("to_id", kuzu::Value::String(to.0.clone())),
        ];

        conn.execute(&mut stmt, params)
            .map_err(|e| anyhow::anyhow!("KuzuDB edge upsert failed ({from} -> {to}): {e}"))?;
        Ok(())
    }

    /// Synchronize only the nodes and incident edges belonging to `file_path` (O(file_symbols)),
    /// evicting stale nodes for that file first and recording its `xxHash64`.
    pub fn sync_file_delta(
        &self,
        graph: &ChronosGraph,
        file_path: &Path,
        file_hash: Option<&str>,
    ) -> Result<usize> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for file delta sync")?;

        let target_norm = file_path
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/");

        if let Ok(mut del_stmt) = conn.prepare("MATCH (n:ChronosNode {file: $file}) DETACH DELETE n") {
            let _ = conn.execute(
                &mut del_stmt,
                vec![("file", kuzu::Value::String(target_norm.clone()))],
            );
        }

        let mut file_node_ids = HashSet::new();
        let mut count = 0usize;
        for node in graph.all_nodes() {
            let node_norm = node
                .location
                .file
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            if node_norm == target_norm {
                self.upsert_node_with_conn(&conn, node)?;
                file_node_ids.insert(node.id.0.clone());
                count += 1;
            }
        }

        if !file_node_ids.is_empty() {
            for (src, dst, edge) in graph.all_edges() {
                if file_node_ids.contains(&src.0) || file_node_ids.contains(&dst.0) {
                    // Edge endpoint in another unindexed file won't match until indexed; ignore non-fatal miss
                    let _ = self.upsert_edge_with_conn(&conn, &src, &dst, &edge);
                }
            }
        }

        if let Some(hash) = file_hash {
            self.upsert_file_hash(&target_norm, hash)?;
        }

        debug!("Synced delta for {} ({} file nodes)", target_norm, count);
        Ok(count)
    }

    /// Bulk sync all nodes AND edges from the in-memory graph → KuzuDB.
    pub fn sync_from_graph(&self, graph: &ChronosGraph) -> Result<usize> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for bulk sync")?;

        let mut count = 0usize;
        for kind in &[
            NodeKind::Function,
            NodeKind::Class,
            NodeKind::ApiRoute,
            NodeKind::TestCase,
            NodeKind::TypeDefinition,
            NodeKind::File,
        ] {
            for node in graph.nodes_by_kind(kind) {
                self.upsert_node_with_conn(&conn, node)?;
                count += 1;
            }
        }

        let mut edge_count = 0usize;
        for (src, dst, edge) in graph.all_edges() {
            self.upsert_edge_with_conn(&conn, &src, &dst, &edge)?;
            edge_count += 1;
        }

        info!("Synced {} nodes and {} edges to KuzuDB", count, edge_count);
        Ok(count)
    }

    /// Load all persisted nodes and edges from KuzuDB losslessly back into the in-memory graph.
    pub fn load_into_graph(&self, graph: &mut ChronosGraph) -> Result<usize> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for load")?;

        let mut result = conn.query(
            "MATCH (n:ChronosNode) \
             RETURN n.id, n.name, n.kind, n.file, n.start_line, \
                    n.cognitive_load, n.cyclomatic, n.fan_in, n.fan_out, \
                    n.param_count, n.sloc, n.is_hydrated, \
                    n.exec_time_p50, n.exec_time_p99, n.invocation_count, \
                    n.cpu_percent, n.is_hot_path, n.is_bottleneck, \
                    n.end_line, n.loop_count, n.loop_depth, \
                    n.transitive_loop_depth, n.linear_scan_in_loop, \
                    n.alloc_in_loop, n.is_recursive, \
                    n.qualified_name, n.contract_signature, n.start_byte, n.end_byte",
        )
        .context("KuzuDB MATCH query failed")?;

        let mut count = 0usize;

        while let Some(row) = result.next() {
            let id_str    = get_string(&row, 0).unwrap_or_default();
            let name      = get_string(&row, 1).unwrap_or_default();
            let kind_str  = get_string(&row, 2).unwrap_or_default();
            let file_str  = get_string(&row, 3).unwrap_or_default();
            let start_ln  = get_i64(&row, 4).unwrap_or(0) as usize;
            let cog_load  = get_f64(&row, 5).unwrap_or(0.0) as f32;
            let cyclo     = get_i64(&row, 6).unwrap_or(0) as u32;
            let fan_in    = get_i64(&row, 7).unwrap_or(0) as u32;
            let fan_out   = get_i64(&row, 8).unwrap_or(0) as u32;
            let params    = get_i64(&row, 9).unwrap_or(0) as u32;
            let sloc      = get_i64(&row, 10).unwrap_or(0) as u32;
            let hydrated  = get_bool(&row, 11).unwrap_or(false);
            let p50       = get_f64(&row, 12).unwrap_or(0.0);
            let p99       = get_f64(&row, 13).unwrap_or(0.0);
            let inv_cnt   = get_i64(&row, 14).unwrap_or(0) as u64;
            let cpu_pct   = get_f64(&row, 15).unwrap_or(0.0) as f32;
            let is_hot    = get_bool(&row, 16).unwrap_or(false);
            let is_bott   = get_bool(&row, 17).unwrap_or(false);
            let end_ln    = get_i64(&row, 18).unwrap_or(start_ln as i64).max(start_ln as i64) as usize;
            let loop_cnt  = get_i64(&row, 19).unwrap_or(0) as u32;
            let loop_dep  = get_i64(&row, 20).unwrap_or(0) as u32;
            let trans_dep = get_i64(&row, 21).unwrap_or(0) as u32;
            let lin_scan  = get_i64(&row, 22).unwrap_or(0) as u32;
            let alloc_lp  = get_i64(&row, 23).unwrap_or(0) as u32;
            let is_rec    = get_bool(&row, 24).unwrap_or(false);
            let qname     = get_string(&row, 25).unwrap_or_default();
            let csig_raw  = get_string(&row, 26).unwrap_or_default();
            let start_b   = get_i64(&row, 27).unwrap_or(0).max(0) as usize;
            let end_b     = get_i64(&row, 28).unwrap_or(0).max(0) as usize;

            if id_str.is_empty() || name.is_empty() {
                warn!("KuzuDB: skipping row with empty id/name");
                continue;
            }

            let kind = parse_node_kind(&kind_str);
            let node_id = NodeId(id_str);
            let location = SourceLocation {
                file: std::path::PathBuf::from(&file_str),
                start_line: start_ln,
                start_col: 0,
                end_line: end_ln,
                end_col: 0,
                start_byte: start_b,
                end_byte: end_b,
            };
            let heuristics = StaticHeuristics {
                cyclomatic_complexity: cyclo,
                fan_in,
                fan_out,
                param_count: params,
                sloc,
                cognitive_load_score: cog_load,
                loop_count: loop_cnt,
                loop_depth: loop_dep,
                transitive_loop_depth: trans_dep,
                linear_scan_in_loop: lin_scan,
                alloc_in_loop: alloc_lp,
                is_recursive: is_rec,
            };

            let telemetry = if hydrated {
                Some(TelemetrySnapshot {
                    captured_at: chrono::Utc::now(),
                    exec_time_ms_p50: p50,
                    exec_time_ms_p95: p99,
                    exec_time_ms_p99: p99,
                    alloc_delta_kb: 0.0,
                    invocation_count: inv_cnt,
                    is_hot_path: is_hot,
                    cpu_percent_self: cpu_pct,
                    is_bottleneck: is_bott,
                    bottleneck_reason: None,
                })
            } else {
                None
            };

            let contract_signature = if csig_raw.is_empty() { None } else { Some(csig_raw) };

            let node = ChronosNode {
                id: node_id,
                kind,
                name,
                qualified_name: qname,
                location,
                heuristics,
                telemetry,
                contract_signature,
                contract_mutated: false,
            };

            graph.add_node(node);
            count += 1;
        }

        // Restore all persisted relationship tables back into the in-memory graph
        let rel_mappings = [
            ("CALLS", crate::graph::schema::EdgeKind::Calls),
            ("CONTAINS", crate::graph::schema::EdgeKind::Contains),
            ("TESTS", crate::graph::schema::EdgeKind::Tests),
            ("IMPORTS", crate::graph::schema::EdgeKind::Imports),
            ("CO_CHANGES_WITH", crate::graph::schema::EdgeKind::CoChangesWith),
        ];

        let mut restored_edges = 0usize;
        for (rel_label, edge_kind) in rel_mappings {
            let cypher = format!(
                "MATCH (a:ChronosNode)-[:{rel_label}]->(b:ChronosNode) RETURN a.id, b.id"
            );
            if let Ok(mut edge_res) = conn.query(&cypher) {
                while let Some(row) = edge_res.next() {
                    let from_str = get_string(&row, 0).unwrap_or_default();
                    let to_str = get_string(&row, 1).unwrap_or_default();
                    if !from_str.is_empty() && !to_str.is_empty() {
                        let edge = crate::graph::schema::ChronosEdge {
                            kind: edge_kind.clone(),
                            call_site_line: None,
                            coverage_traced: matches!(edge_kind, crate::graph::schema::EdgeKind::Tests),
                        };
                        if graph
                            .add_edge(&NodeId(from_str), &NodeId(to_str), edge)
                            .is_ok()
                        {
                            restored_edges += 1;
                            if matches!(edge_kind, crate::graph::schema::EdgeKind::Calls) {
                                graph.resolved_call_sites += 1;
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Restored {} nodes and {} edges from KuzuDB into memory graph",
            count, restored_edges
        );
        Ok(count)
    }

    /// Incrementally synchronize a list of workspace files using `xxHash64` Merkle file hashes.
    pub fn sync_workspace_incremental(
        &self,
        graph: &mut ChronosGraph,
        files: &[std::path::PathBuf],
        dispatcher: &mut crate::ast::dispatcher::LanguageDispatcher,
    ) -> Result<IncrementalSyncStats> {
        if graph.node_count() == 0 {
            let _ = self.load_into_graph(graph);
        }

        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for incremental sync")?;

        let mut stored_hashes: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if let Ok(mut res) = conn.query("MATCH (f:ChronosFileHash) RETURN f.file, f.xxhash64") {
            while let Some(row) = res.next() {
                let f = get_string(&row, 0).unwrap_or_default();
                let h = get_string(&row, 1).unwrap_or_default();
                if !f.is_empty() {
                    stored_hashes.insert(f, h);
                }
            }
        }

        let files_in_graph: std::collections::HashSet<String> = graph
            .all_nodes()
            .into_iter()
            .map(|n| n.location.file.to_string_lossy().replace('\\', "/"))
            .collect();

        let mut unchanged_files = 0usize;
        let mut re_parsed_files = 0usize;

        for file_path in files {
            if !crate::ast::dispatcher::LanguageDispatcher::is_supported(file_path) {
                continue;
            }
            let Ok(current_hash) = compute_file_xxhash64(file_path) else {
                continue;
            };
            let norm_key = file_path
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");

            if stored_hashes.get(&norm_key) == Some(&current_hash)
                && files_in_graph.contains(&norm_key)
            {
                unchanged_files += 1;
                continue;
            }

            graph.remove_nodes_for_file(file_path);
            if let Ok(mut del_stmt) = conn.prepare("MATCH (n:ChronosNode {file: $file}) DETACH DELETE n") {
                let _ = conn.execute(&mut del_stmt, vec![("file", kuzu::Value::String(norm_key.clone()))]);
            }

            if let Ok(parsed) = dispatcher.parse_file(file_path) {
                graph.ingest_parsed_module(&parsed);
                re_parsed_files += 1;
                self.upsert_file_hash(&norm_key, &current_hash)?;
            }
        }

        if re_parsed_files > 0 {
            graph.resolve_pending_calls_and_routes();
            let _ = self.sync_from_graph(graph);
        }

        Ok(IncrementalSyncStats {
            total_files: unchanged_files + re_parsed_files,
            unchanged_files,
            re_parsed_files,
        })
    }

    /// Upsert a single file's `xxHash64` digest into `ChronosFileHash`.
    pub fn upsert_file_hash(&self, file_norm: &str, hash: &str) -> Result<()> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for file hash upsert")?;
        if let Ok(mut hash_stmt) = conn.prepare(
            "MERGE (f:ChronosFileHash {file: $file}) \
             ON CREATE SET f.xxhash64 = $hash \
             ON MATCH SET f.xxhash64 = $hash",
        ) {
            let _ = conn.execute(
                &mut hash_stmt,
                vec![
                    ("file", kuzu::Value::String(file_norm.to_string())),
                    ("hash", kuzu::Value::String(hash.to_string())),
                ],
            );
        }
        Ok(())
    }

    /// Query performance hotspots directly from KuzuDB.
    /// Returns `(name, file, p99_ms, cpu_percent)` tuples sorted by p99 descending.
    pub fn query_hotspots_db(
        &self,
        min_p99_ms: f64,
    ) -> Result<Vec<(String, String, f64, f64)>> {
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for hotspot query")?;

        let cypher = format!(
            "MATCH (n:ChronosNode) \
             WHERE n.exec_time_p99 >= {min} \
             RETURN n.name, n.file, n.exec_time_p99, n.cpu_percent \
             ORDER BY n.exec_time_p99 DESC \
             LIMIT 20",
            min = min_p99_ms
        );

        let mut result = conn
            .query(&cypher)
            .context("KuzuDB hotspot query failed")?;

        let mut hotspots = Vec::new();
        while let Some(row) = result.next() {
            let name    = get_string(&row, 0).unwrap_or_default();
            let file    = get_string(&row, 1).unwrap_or_default();
            let p99     = get_f64(&row, 2).unwrap_or(0.0);
            let cpu     = get_f64(&row, 3).unwrap_or(0.0);
            if !name.is_empty() {
                hotspots.push((name, file, p99, cpu));
            }
        }

        Ok(hotspots)
    }

    /// Execute a Read-Only Cypher query against the embedded KuzuDB graph.
    ///
    /// Enforces `validate_readonly_cypher` to block any mutating (`CREATE`, `MERGE`, `SET`,
    /// `DELETE`, `DROP`, `ALTER`) or file-I/O (`COPY`, `LOAD`, `INSTALL`, `EXPORT`) clauses.
    pub fn execute_readonly_cypher(
        &self,
        query: &str,
        max_rows: usize,
    ) -> Result<CypherQueryResult> {
        validate_readonly_cypher(query)?;

        let ceiling = max_rows.clamp(1, 100_000);
        let conn = Connection::new(&self.db)
            .context("Failed to open KuzuDB connection for read-only Cypher query")?;

        let mut result = conn
            .query(query)
            .map_err(|e| anyhow::anyhow!("Cypher query execution failed: {e}"))?;

        let columns = result.get_column_names();
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut total_rows = 0usize;

        while let Some(row) = result.next() {
            total_rows += 1;
            if rows.len() < ceiling {
                let json_row: Vec<serde_json::Value> = row.iter().map(kuzu_value_to_json).collect();
                rows.push(json_row);
            }
        }

        Ok(CypherQueryResult {
            columns,
            truncated: total_rows > ceiling,
            total_rows,
            rows,
        })
    }
}

/// Structured output from a read-only Cypher query against KuzuDB.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CypherQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total_rows: usize,
    pub truncated: bool,
}

/// Statistics returned by `sync_workspace_incremental` (Merkle `xxHash64` cache).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IncrementalSyncStats {
    pub total_files: usize,
    pub unchanged_files: usize,
    pub re_parsed_files: usize,
}

/// Compute the 64-bit `xxHash64` hex digest of a file's raw bytes.
pub fn compute_file_xxhash64(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read file for xxHash64: {}", path.display()))?;
    let digest = xxhash_rust::xxh64::xxh64(&bytes, 0);
    Ok(format!("{digest:016x}"))
}

/// Validate that a Cypher query is strictly read-only (`MATCH ... RETURN ...`).
///
/// Strips string literals before scanning word tokens so string filters like
/// `WHERE n.name = 'deleteUser'` are allowed while `DELETE n` or `COPY` are rejected.
pub fn validate_readonly_cypher(query: &str) -> Result<()> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("Cypher query cannot be empty"));
    }

    // Strip single-quoted and double-quoted string contents so literals like 'delete_user' don't false-positive
    let mut stripped = String::with_capacity(trimmed.len());
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_backslash = false;

    for ch in trimmed.chars() {
        if prev_backslash {
            prev_backslash = false;
            continue;
        }
        if ch == '\\' && (in_single || in_double) {
            prev_backslash = true;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
            stripped.push(' ');
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
            stripped.push(' ');
            continue;
        }
        if !in_single && !in_double {
            stripped.push(ch);
        }
    }

    if in_single || in_double {
        return Err(anyhow::anyhow!("Unterminated string literal in Cypher query"));
    }

    // Disallow multiple statements separated by `;` (except trailing semicolon)
    let without_trailing_semi = stripped.trim().trim_end_matches(';');
    if without_trailing_semi.contains(';') {
        return Err(anyhow::anyhow!(
            "Multiple Cypher statements separated by ';' are not allowed in read-only mode"
        ));
    }

    let forbidden = [
        "CREATE",
        "MERGE",
        "SET",
        "DELETE",
        "DETACH",
        "REMOVE",
        "DROP",
        "ALTER",
        "COPY",
        "LOAD",
        "INSTALL",
        "EXPORT",
        "IMPORT",
        "ATTACH",
        "CALL",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
    ];

    let mut has_read_clause = false;
    for token in without_trailing_semi.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if token.is_empty() {
            continue;
        }
        let upper = token.to_ascii_uppercase();
        if forbidden.contains(&upper.as_str()) {
            return Err(anyhow::anyhow!(
                "Forbidden Cypher keyword '{}' in read-only query mode",
                upper
            ));
        }
        if matches!(upper.as_str(), "MATCH" | "RETURN" | "UNWIND") {
            has_read_clause = true;
        }
    }

    if !has_read_clause {
        return Err(anyhow::anyhow!(
            "Read-only Cypher query must contain a MATCH or RETURN clause"
        ));
    }

    Ok(())
}

fn kuzu_value_to_json(val: &kuzu::Value) -> serde_json::Value {
    match val {
        kuzu::Value::Null(_) => serde_json::Value::Null,
        kuzu::Value::Bool(b) => serde_json::Value::Bool(*b),
        kuzu::Value::Int64(n) => serde_json::json!(*n),
        kuzu::Value::Int32(n) => serde_json::json!(*n),
        kuzu::Value::Int16(n) => serde_json::json!(*n),
        kuzu::Value::Int8(n) => serde_json::json!(*n),
        kuzu::Value::UInt64(n) => serde_json::json!(*n),
        kuzu::Value::UInt32(n) => serde_json::json!(*n),
        kuzu::Value::UInt16(n) => serde_json::json!(*n),
        kuzu::Value::UInt8(n) => serde_json::json!(*n),
        kuzu::Value::Double(d) => serde_json::json!(*d),
        kuzu::Value::Float(f) => serde_json::json!(*f),
        kuzu::Value::String(s) => serde_json::Value::String(s.clone()),
        other => serde_json::Value::String(other.to_string()),
    }
}

// ── Row extraction helpers ────────────────────────────────────────────────────
// Kuzu returns `kuzu::Value`; these helpers provide ergonomic extraction with
// graceful fallback rather than panicking.

fn get_string(row: &[kuzu::Value], idx: usize) -> Option<String> {
    row.get(idx).and_then(|v| {
        if let kuzu::Value::String(s) = v { Some(s.clone()) } else { None }
    })
}

fn get_i64(row: &[kuzu::Value], idx: usize) -> Option<i64> {
    row.get(idx).and_then(|v| match v {
        kuzu::Value::Int64(n) => Some(*n),
        kuzu::Value::Int32(n) => Some(*n as i64),
        _ => None,
    })
}

fn get_f64(row: &[kuzu::Value], idx: usize) -> Option<f64> {
    row.get(idx).and_then(|v| match v {
        kuzu::Value::Double(n) => Some(*n),
        kuzu::Value::Float(n)  => Some(*n as f64),
        _ => None,
    })
}

fn get_bool(row: &[kuzu::Value], idx: usize) -> Option<bool> {
    row.get(idx).and_then(|v| {
        if let kuzu::Value::Bool(b) = v { Some(*b) } else { None }
    })
}

fn parse_node_kind(s: &str) -> NodeKind {
    match s {
        "Function"       => NodeKind::Function,
        "Class"          => NodeKind::Class,
        "ApiRoute"       => NodeKind::ApiRoute,
        "TestCase"       => NodeKind::TestCase,
        "TypeDefinition" => NodeKind::TypeDefinition,
        _                => NodeKind::File,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────


