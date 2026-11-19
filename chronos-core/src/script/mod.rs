//! # Embedded `rhai` 5D Script Engine (`chronos_explore.script`, `chronos eval`, `chronos repl`)
//!
//! Embeds a strict sandbox (`Engine::new_raw()` + pure-data core packages, zero filesystem/network/env I/O)
//! bounded by execution quotas (`max_operations = 50_000`, `max_call_levels = 32`,
//! `max_expr_depths = (32, 32)`, `max_string_size = 256 KiB`, `max_array_size = 4096`)
//! and enforces a strict dual-capability split:
//! - `ScriptProfile::ReadOnly` (MCP `chronos_explore.script` and `--readonly` CLI): ONLY read-only
//!   5D graph functions are registered; `edit_symbol` / `refactor_symbol` do not exist in the engine.
//! - `ScriptProfile::FullCli` (CLI `chronos eval` / `chronos repl`): additionally exposes gated
//!   `edit_symbol` and `refactor_symbol` subject to the 2-stage syntax & blast-radius gate.

use crate::ast::refactor::{execute_refactor_symbol, RefactorAction, RefactorSymbolRequest};
use crate::ast::symbol_editor::{execute_edit_symbol, EditOperation, EditSymbolRequest};
use crate::blast_radius::ImpactAnalyzer;
use crate::graph::persistent::validate_readonly_cypher;
use crate::graph::query::{ExploreOptions, GraphQueryEngine, GraphSearchRequest, NodeSummary};
use crate::graph::schema::NodeKind;
use crate::session::WorkspaceSession;
use anyhow::{anyhow, Result};
use rhai::packages::{
    ArithmeticPackage, BasicArrayPackage, BasicIteratorPackage, BasicMapPackage,
    BasicStringPackage, CorePackage, LogicPackage, Package,
};
use rhai::{Dynamic, Engine, EvalAltResult};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const DEFAULT_MAX_OPERATIONS: u64 = 50_000;
pub const DEFAULT_MAX_CALL_LEVELS: usize = 32;
pub const DEFAULT_MAX_EXPR_DEPTH: usize = 32;
pub const DEFAULT_MAX_STRING_SIZE: usize = 256 * 1024;
pub const DEFAULT_MAX_ARRAY_SIZE: usize = 4096;

/// Strict capability boundary separating MCP read-only queries from gated CLI mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptProfile {
    ReadOnly,
    FullCli,
}

/// Structured evaluation outcome returned by `ScriptEngine::eval`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEvalOutcome {
    pub profile: String,
    pub result: serde_json::Value,
    pub printed_logs: Vec<String>,
}

/// RAII restore guard guaranteeing `WorkspaceSession` is always returned even if a closure unwinds.
struct SessionSwapGuard<'a> {
    target: &'a mut WorkspaceSession,
    shared: Arc<Mutex<WorkspaceSession>>,
}

impl<'a> SessionSwapGuard<'a> {
    fn swap(session: &'a mut WorkspaceSession) -> Self {
        let placeholder = WorkspaceSession::open_in_memory(PathBuf::new(), session.readonly);
        let owned = std::mem::replace(session, placeholder);
        let shared = Arc::new(Mutex::new(owned));
        Self {
            target: session,
            shared,
        }
    }
}

impl Drop for SessionSwapGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.shared.lock() {
            let placeholder = WorkspaceSession::open_in_memory(PathBuf::new(), guard.readonly);
            let restored = std::mem::replace(&mut *guard, placeholder);
            *self.target = restored;
        }
    }
}

/// Stateless orchestrator compiling and executing bounded `rhai` scripts against a `WorkspaceSession`.
pub struct ScriptEngine;

impl ScriptEngine {
    /// Evaluate `script` inside `session` enforcing `profile`.
    /// Note: if `session.readonly` is true, `FullCli` automatically downgrades to `ScriptProfile::ReadOnly`.
    pub fn eval(
        session: &mut WorkspaceSession,
        profile: ScriptProfile,
        script: &str,
    ) -> Result<ScriptEvalOutcome> {
        let effective_profile = if session.readonly {
            ScriptProfile::ReadOnly
        } else {
            profile
        };

        let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let guard = SessionSwapGuard::swap(session);
        let shared = Arc::clone(&guard.shared);

        let result_dynamic = {
            let engine = build_sandboxed_engine(
                Arc::clone(&shared),
                effective_profile,
                Arc::clone(&logs),
            );
            engine.eval::<Dynamic>(script)
        };

        // Ensure all closures registered on `engine` have dropped before SessionSwapGuard restores `session`
        drop(guard);

        let dynamic = result_dynamic.map_err(|e| anyhow!("Rhai script error: {}", e))?;
        let json_val = rhai::serde::from_dynamic::<serde_json::Value>(&dynamic)
            .unwrap_or_else(|_| serde_json::Value::String(dynamic.to_string()));
        let printed_logs = logs.lock().map(|g| g.clone()).unwrap_or_default();

        Ok(ScriptEvalOutcome {
            profile: match effective_profile {
                ScriptProfile::ReadOnly => "ReadOnly".to_string(),
                ScriptProfile::FullCli => "FullCli".to_string(),
            },
            result: json_val,
            printed_logs,
        })
    }
}

fn build_sandboxed_engine(
    shared: Arc<Mutex<WorkspaceSession>>,
    profile: ScriptProfile,
    logs: Arc<Mutex<Vec<String>>>,
) -> Engine {
    let mut engine = Engine::new_raw();

    // Pure data/control-flow standard packages only (zero filesystem, shell, network, or env access)
    engine.register_global_module(CorePackage::new().as_shared_module());
    engine.register_global_module(ArithmeticPackage::new().as_shared_module());
    engine.register_global_module(LogicPackage::new().as_shared_module());
    engine.register_global_module(BasicStringPackage::new().as_shared_module());
    engine.register_global_module(BasicArrayPackage::new().as_shared_module());
    engine.register_global_module(BasicMapPackage::new().as_shared_module());
    engine.register_global_module(BasicIteratorPackage::new().as_shared_module());

    // Strict bounded resource quotas
    engine.set_max_operations(DEFAULT_MAX_OPERATIONS);
    engine.set_max_call_levels(DEFAULT_MAX_CALL_LEVELS);
    engine.set_max_expr_depths(DEFAULT_MAX_EXPR_DEPTH, DEFAULT_MAX_EXPR_DEPTH);
    engine.set_max_string_size(DEFAULT_MAX_STRING_SIZE);
    engine.set_max_array_size(DEFAULT_MAX_ARRAY_SIZE);
    engine.set_max_map_size(DEFAULT_MAX_ARRAY_SIZE);

    let log_print = Arc::clone(&logs);
    engine.on_print(move |text| {
        if let Ok(mut buf) = log_print.lock() {
            if buf.len() < 128 {
                buf.push(text.to_string());
            }
        }
    });
    engine.on_debug(move |text, _, _| {
        if let Ok(mut buf) = logs.lock() {
            if buf.len() < 128 {
                buf.push(text.to_string());
            }
        }
    });

    register_readonly_functions(&mut engine, Arc::clone(&shared));

    if profile == ScriptProfile::FullCli {
        register_cli_mutation_functions(&mut engine, shared);
    }

    engine
}

fn lock_session<'a>(
    shared: &'a Arc<Mutex<WorkspaceSession>>,
) -> Result<std::sync::MutexGuard<'a, WorkspaceSession>, Box<EvalAltResult>> {
    shared
        .lock()
        .map_err(|_| Box::new(EvalAltResult::from("Internal mutex poison in ScriptEngine")))
}

fn to_dyn<T: Serialize>(val: &T) -> Result<Dynamic, Box<EvalAltResult>> {
    rhai::serde::to_dynamic(val)
        .map_err(|e| Box::new(EvalAltResult::from(format!("Serialization error: {e}"))))
}

fn register_readonly_functions(engine: &mut Engine, shared: Arc<Mutex<WorkspaceSession>>) {
    // explore(query)
    let s1 = Arc::clone(&shared);
    engine.register_fn(
        "explore",
        move |query: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s1)?;
            let resp = GraphQueryEngine::new(&guard.graph).explore_graph(query, None, 3);
            to_dyn(&resp)
        },
    );

    // explore(query, max_symbols)
    let s2 = Arc::clone(&shared);
    engine.register_fn(
        "explore",
        move |query: &str, max_symbols: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s2)?;
            let cap = max_symbols.clamp(1, 20) as usize;
            let resp = GraphQueryEngine::new(&guard.graph).explore_graph_with_options(
                query,
                &ExploreOptions {
                    max_symbols: cap,
                    ..Default::default()
                },
            );
            to_dyn(&resp)
        },
    );

    // search(query)
    let s3 = Arc::clone(&shared);
    engine.register_fn(
        "search",
        move |query: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s3)?;
            let resp = GraphQueryEngine::new(&guard.graph).search_graph(&GraphSearchRequest {
                query: if query.trim().is_empty() {
                    None
                } else {
                    Some(query.to_string())
                },
                limit: 15,
                ..Default::default()
            });
            to_dyn(&resp)
        },
    );

    // search_kind(kind, min_loop_depth)
    let s4 = Arc::clone(&shared);
    engine.register_fn(
        "search_kind",
        move |kind: &str, min_loop_depth: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s4)?;
            let resp = GraphQueryEngine::new(&guard.graph).search_graph(&GraphSearchRequest {
                kind: if kind.trim().is_empty() {
                    None
                } else {
                    Some(kind.to_string())
                },
                min_loop_depth: if min_loop_depth > 0 {
                    Some(min_loop_depth as u32)
                } else {
                    None
                },
                limit: 20,
                ..Default::default()
            });
            to_dyn(&resp)
        },
    );

    // blast_radius(symbol)
    let s5 = Arc::clone(&shared);
    engine.register_fn(
        "blast_radius",
        move |symbol: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s5)?;
            let Some(node) = guard.graph.resolve_first_symbol(symbol, None) else {
                return Err(Box::new(EvalAltResult::from(format!(
                    "Symbol '{}' not found in graph",
                    symbol
                ))));
            };
            let report = ImpactAnalyzer::new(&guard.graph).analyze(&node.id, false);
            to_dyn(&report)
        },
    );

    // hotspots(top_n)
    let s6 = Arc::clone(&shared);
    engine.register_fn(
        "hotspots",
        move |top_n: i64| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s6)?;
            let cap = top_n.clamp(1, 50) as usize;
            let mut nodes: Vec<_> = guard
                .graph
                .all_nodes()
                .into_iter()
                .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::ApiRoute))
                .collect();
            nodes.sort_by(|a, b| {
                let a_bot = a.telemetry.as_ref().map(|t| t.is_bottleneck).unwrap_or(false);
                let b_bot = b.telemetry.as_ref().map(|t| t.is_bottleneck).unwrap_or(false);
                b_bot
                    .cmp(&a_bot)
                    .then_with(|| {
                        b.heuristics
                            .transitive_loop_depth
                            .cmp(&a.heuristics.transitive_loop_depth)
                    })
                    .then_with(|| {
                        b.heuristics
                            .linear_scan_in_loop
                            .cmp(&a.heuristics.linear_scan_in_loop)
                    })
                    .then_with(|| {
                        b.heuristics
                            .cognitive_load_score
                            .partial_cmp(&a.heuristics.cognitive_load_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
            let out: Vec<NodeSummary> = nodes.into_iter().take(cap).map(NodeSummary::from_node).collect();
            to_dyn(&out)
        },
    );

    // arch()
    let s7 = Arc::clone(&shared);
    engine.register_fn("arch", move || -> Result<Dynamic, Box<EvalAltResult>> {
        let guard = lock_session(&s7)?;
        let report = GraphQueryEngine::new(&guard.graph).get_architecture(None);
        to_dyn(&report)
    });

    // arch(path_prefix)
    let s8 = Arc::clone(&shared);
    engine.register_fn(
        "arch",
        move |path_prefix: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s8)?;
            let pref = if path_prefix.trim().is_empty() {
                None
            } else {
                Some(path_prefix)
            };
            let report = GraphQueryEngine::new(&guard.graph).get_architecture(pref);
            to_dyn(&report)
        },
    );

    // snippet(symbol)
    let s9 = Arc::clone(&shared);
    engine.register_fn(
        "snippet",
        move |symbol: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let guard = lock_session(&s9)?;
            let Some(snip) = GraphQueryEngine::new(&guard.graph).get_code_snippet(symbol, None)
            else {
                return Err(Box::new(EvalAltResult::from(format!(
                    "Snippet for symbol '{}' not found",
                    symbol
                ))));
            };
            to_dyn(&snip)
        },
    );

    // cypher(query)
    let s10 = Arc::clone(&shared);
    engine.register_fn(
        "cypher",
        move |query: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            validate_readonly_cypher(query)
                .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
            let guard = lock_session(&s10)?;
            let Some(repo) = guard.repo() else {
                return Err(Box::new(EvalAltResult::from(
                    "KuzuDB repository unavailable in in-memory mode; use explore/search/blast_radius instead",
                )));
            };
            let rows = repo
                .execute_readonly_cypher(query, 50)
                .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
            to_dyn(&rows)
        },
    );
}

fn register_cli_mutation_functions(engine: &mut Engine, shared: Arc<Mutex<WorkspaceSession>>) {
    // edit_symbol(symbol, op, new_code, file)
    let s_edit = Arc::clone(&shared);
    engine.register_fn(
        "edit_symbol",
        move |symbol: &str,
              op: &str,
              new_code: &str,
              file: &str|
              -> Result<Dynamic, Box<EvalAltResult>> {
            run_edit_from_script(&s_edit, symbol, op, new_code, file, false, false)
        },
    );

    // edit_symbol(symbol, op, new_code, file, dry_run, force)
    let s_edit_opts = Arc::clone(&shared);
    engine.register_fn(
        "edit_symbol",
        move |symbol: &str,
              op: &str,
              new_code: &str,
              file: &str,
              dry_run: bool,
              force: bool|
              -> Result<Dynamic, Box<EvalAltResult>> {
            run_edit_from_script(&s_edit_opts, symbol, op, new_code, file, dry_run, force)
        },
    );

    // refactor_symbol(symbol, action, new_name_or_empty, file)
    let s_ref = Arc::clone(&shared);
    engine.register_fn(
        "refactor_symbol",
        move |symbol: &str,
              action: &str,
              new_name_or_empty: &str,
              file: &str|
              -> Result<Dynamic, Box<EvalAltResult>> {
            run_refactor_from_script(
                &s_ref,
                symbol,
                action,
                new_name_or_empty,
                file,
                false,
                false,
            )
        },
    );

    // refactor_symbol(symbol, action, new_name_or_empty, file, dry_run, force)
    let s_ref_opts = Arc::clone(&shared);
    engine.register_fn(
        "refactor_symbol",
        move |symbol: &str,
              action: &str,
              new_name_or_empty: &str,
              file: &str,
              dry_run: bool,
              force: bool|
              -> Result<Dynamic, Box<EvalAltResult>> {
            run_refactor_from_script(
                &s_ref_opts,
                symbol,
                action,
                new_name_or_empty,
                file,
                dry_run,
                force,
            )
        },
    );
}

fn run_edit_from_script(
    shared: &Arc<Mutex<WorkspaceSession>>,
    symbol: &str,
    op: &str,
    new_code: &str,
    file: &str,
    dry_run: bool,
    force: bool,
) -> Result<Dynamic, Box<EvalAltResult>> {
    let parsed_op = EditOperation::parse(op)
        .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
    let mut guard = lock_session(shared)?;
    let outcome = block_on_scoped(execute_edit_symbol(
        &mut guard,
        EditSymbolRequest {
            symbol: symbol.to_string(),
            file: PathBuf::from(file),
            operation: parsed_op,
            new_code: new_code.to_string(),
            dry_run,
            force,
        },
    ))
    .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
    to_dyn(&outcome)
}

fn run_refactor_from_script(
    shared: &Arc<Mutex<WorkspaceSession>>,
    symbol: &str,
    action: &str,
    new_name_or_empty: &str,
    file: &str,
    dry_run: bool,
    force: bool,
) -> Result<Dynamic, Box<EvalAltResult>> {
    let parsed_action = RefactorAction::parse(action)
        .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
    let mut guard = lock_session(shared)?;
    let outcome = block_on_scoped(execute_refactor_symbol(
        &mut guard,
        RefactorSymbolRequest {
            symbol: symbol.to_string(),
            action: parsed_action,
            new_name: if new_name_or_empty.trim().is_empty() {
                None
            } else {
                Some(new_name_or_empty.trim().to_string())
            },
            file: if file.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(file))
            },
            dry_run,
            force,
            use_lsp: false,
        },
    ))
    .map_err(|e| Box::new(EvalAltResult::from(e.to_string())))?;
    to_dyn(&outcome)
}

/// Safely drive an async session mutation from synchronous Rhai closures regardless of
/// whether the caller already runs inside a `current_thread` or `multi_thread` Tokio runtime.
fn block_on_scoped<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send,
    T: Send,
{
    std::thread::scope(|scope| {
        let handle = scope.spawn(move || -> Result<T> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(fut)
        });
        match handle.join() {
            Ok(res) => res,
            Err(_) => Err(anyhow!("Scoped async worker thread panicked")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rhai_readonly_profile_executes_5d_queries_and_blocks_edit_functions() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("svc.rs");
        std::fs::write(
            &file,
            "pub fn helper(x: i32) -> i32 { x + 1 }\npub fn entry(n: i32) -> i32 { helper(n) }\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let ok_res = ScriptEngine::eval(
            &mut session,
            ScriptProfile::ReadOnly,
            r#"
                let b = blast_radius("helper");
                let a = arch();
                #{ callers: b.direct_callers_count, nodes: a.total_nodes }
            "#,
        )
        .unwrap();

        assert_eq!(ok_res.profile, "ReadOnly");
        assert_eq!(ok_res.result["callers"], 1);
        assert!(ok_res.result["nodes"].as_u64().unwrap() >= 2);

        // ReadOnly profile must NEVER register edit_symbol or refactor_symbol
        let blocked_edit = ScriptEngine::eval(
            &mut session,
            ScriptProfile::ReadOnly,
            r#"edit_symbol("helper", "replace_body", "pub fn helper(x: i32) -> i32 { x }", "svc.rs")"#,
        );
        assert!(blocked_edit.is_err());

        let blocked_ref = ScriptEngine::eval(
            &mut session,
            ScriptProfile::ReadOnly,
            r#"refactor_symbol("helper", "safe_delete", "", "svc.rs")"#,
        );
        assert!(blocked_ref.is_err());
    }

    #[test]
    fn test_rhai_operation_limit_aborts_infinite_loops_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), true);

        let err = ScriptEngine::eval(
            &mut session,
            ScriptProfile::ReadOnly,
            "let i = 0; loop { i += 1; } i",
        );
        assert!(err.is_err(), "Infinite loop must be aborted by max_operations budget");
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.to_lowercase().contains("operations") || msg.to_lowercase().contains("limit") || msg.to_lowercase().contains("terminated"),
            "Unexpected error message: {msg}"
        );
    }

    #[test]
    fn test_rhai_cli_profile_chains_search_and_edit_with_gate() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("math.rs");
        std::fs::write(
            &file,
            "pub fn price(x: i32) -> i32 {\n    x + 1\n}\n\npub fn checkout() -> i32 {\n    price(10)\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        // 1. Chain search + internal edit -> succeeds
        let applied = ScriptEngine::eval(
            &mut session,
            ScriptProfile::FullCli,
            r#"
                let hits = search("price");
                let target_file = hits.results[0].node.file;
                edit_symbol("price", "replace_body", "pub fn price(x: i32) -> i32 {\n    x + 5\n}", target_file)
            "#,
        )
        .unwrap();
        assert_eq!(applied.result["status"], "applied");
        assert!(std::fs::read_to_string(&file).unwrap().contains("x + 5"));

        // 2. Signature-breaking edit inside FullCli script is STILL stopped by Stage 2 blast-radius gate
        let gated = ScriptEngine::eval(
            &mut session,
            ScriptProfile::FullCli,
            r#"
                edit_symbol("price", "replace_body", "pub fn price(x: i32, tax: i32) -> i32 {\n    x + tax\n}", "math.rs")
            "#,
        )
        .unwrap();
        assert_eq!(gated.result["status"], "blocked_requires_force");
        assert_eq!(gated.result["is_error"], true);
        assert!(!std::fs::read_to_string(&file).unwrap().contains("tax: i32"));
    }
}
