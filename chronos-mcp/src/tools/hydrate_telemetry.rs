//! # `chronos_hydrate_telemetry` Tool Handler
//!
//! Hydrates the 4th Dimension (runtime telemetry) of the `ChronosGraph` from an offline
//! profiler artifact on disk or inline content:
//! - Chrome / Node.js V8 `.cpuprofile` JSON
//! - Speedscope / `py-spy` JSON
//! - Folded Stack Flamegraph text (`foo;bar;baz 42`)
//! - OpenTelemetry (OTLP) Trace JSON

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use chronos_core::telemetry::{
    hydration::hydrate_graph,
    profile_loader::{load_profile_from_file, parse_profile_str},
};
use serde_json::{json, Value};
use std::path::PathBuf;

pub async fn handle_hydrate_telemetry(state: SharedState, params: Option<Value>) -> CallToolResult {
    let profile_path = params
        .as_ref()
        .and_then(|p| p.get("profile_path"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let profile_content = params
        .as_ref()
        .and_then(|p| p.get("profile_content"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let session = if let Some(raw) = profile_content {
        match parse_profile_str(&raw) {
            Ok(s) => s,
            Err(e) => return CallToolResult::error(format!("Failed to parse profile_content: {}", e)),
        }
    } else if let Some(path_str) = profile_path {
        let abs_path = {
            let st = state.read().await;
            let p = PathBuf::from(&path_str);
            if p.is_absolute() {
                p
            } else {
                st.workspace_root.join(&p)
            }
        };
        match load_profile_from_file(&abs_path) {
            Ok(s) => s,
            Err(e) => return CallToolResult::error(format!("Failed to load profile file: {}", e)),
        }
    } else {
        return CallToolResult::error(
            "Missing required parameter: provide either `profile_path` or `profile_content`",
        );
    };

    let mut st = state.write().await;
    let report = hydrate_graph(&mut st.graph, &session);

    if let Some(ref db) = st.db {
        let _ = db.sync_from_graph(&st.graph);
    }

    let bottlenecks: Vec<Value> = session
        .bottlenecks()
        .into_iter()
        .take(15)
        .map(|b| {
            json!({
                "function_name": b.function_name,
                "cpu_percent_self": b.cpu_percent_self(),
                "p50_ms": b.p50_ms(),
                "p99_ms": b.p99_ms(),
                "invocation_count": b.invocation_count,
                "reason": b.bottleneck_reason(),
            })
        })
        .collect();

    let output = json!({
        "status": "ok",
        "backend": session.backend,
        "total_duration_ms": session.total_duration_ms,
        "total_profiled_functions": report.total_profiled,
        "hydrated_graph_nodes": report.hydrated_count,
        "cold_graph_nodes": report.cold_count,
        "bottlenecks_found": report.bottlenecks_found,
        "top_bottlenecks": bottlenecks,
    });

    CallToolResult::text(serde_json::to_string_pretty(&output).unwrap_or_default())
}
