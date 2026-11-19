//! # MCP ToolRegistry & Middleware
//!
//! Bridges `chronos_core::registry::ToolRegistry` into `chronos-mcp`:
//! - Generates `tools/list` from the canonical core specification
//! - Enforces `--readonly` mode guards
//! - Dispatches `tools/call` across all 14 tools (plus Serena compatibility aliases)
//! - Automatically drains pending `ContractEngine` alerts into `_meta.contract_alerts`
//! - Applies universal `max_answer_chars` (default 24,000) UTF-8 safe response budgeting

use crate::{
    protocol::{CallToolResult, JsonRpcError, McpTool, ToolAnnotations},
    state::SharedState,
    tools,
};
use chronos_core::registry::{
    resolve_max_answer_chars, truncate_to_budget, ToolRegistry,
};
use serde_json::{json, Value};
use tracing::warn;

/// MCP server-side tool registry & execution middleware.
pub struct McpToolRegistry;

impl McpToolRegistry {
    /// Return canonical MCP tool list (hiding mutating write tools when `readonly == true`).
    pub fn list_tools(readonly: bool) -> Vec<McpTool> {
        ToolRegistry::specs(!readonly)
            .into_iter()
            .map(|spec| McpTool {
                name: spec.name,
                description: spec.description,
                input_schema: spec.to_mcp_schema(),
                annotations: Some(ToolAnnotations {
                    title: spec.annotations.title,
                    read_only_hint: spec.annotations.read_only_hint,
                    destructive_hint: spec.annotations.destructive_hint,
                    idempotent_hint: spec.annotations.idempotent_hint,
                    open_world_hint: spec.annotations.open_world_hint,
                }),
            })
            .collect()
    }

    /// Dispatch an MCP `tools/call` request through read-only, alert, and budget middleware.
    pub async fn dispatch(
        state: SharedState,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> std::result::Result<CallToolResult, JsonRpcError> {
        let canonical_name = match tool_name {
            "chronos_replace_symbol_body" => "chronos_edit_symbol",
            "chronos_rename_symbol" | "chronos_safe_delete" => "chronos_refactor_symbol",
            other => other,
        };

        let Some(spec) = ToolRegistry::find(canonical_name) else {
            warn!("Unknown tool called: {}", tool_name);
            return Err(JsonRpcError {
                code: -32601,
                message: format!("Tool not found: {}", tool_name),
                data: None,
            });
        };

        // Inject default operation/action when invoked via Serena compatibility aliases
        let normalized_args = match tool_name {
            "chronos_replace_symbol_body" => Some(inject_default_arg(arguments, "operation", "replace_body")),
            "chronos_rename_symbol" => Some(inject_default_arg(arguments, "action", "rename")),
            "chronos_safe_delete" => Some(inject_default_arg(arguments, "action", "safe_delete")),
            _ => arguments,
        };

        let readonly = {
            let st = state.read().await;
            st.readonly
        };

        let dry_run = normalized_args
            .as_ref()
            .and_then(|a| a.get("dry_run"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let refactor_action = normalized_args
            .as_ref()
            .and_then(|a| a.get("action"))
            .and_then(|v| v.as_str())
            .unwrap_or("preview_usages");

        let readonly_allowed = spec.readonly
            || (canonical_name == "chronos_refactor_symbol"
                && (refactor_action == "preview_usages" || dry_run));

        if readonly && !readonly_allowed {
            return Ok(CallToolResult::error(format!(
                "Security: tool '{}' is disabled in --readonly mode.",
                tool_name
            )));
        }

        let max_chars = resolve_max_answer_chars(normalized_args.as_ref());

        let mut result = match canonical_name {
            "chronos_analyze_file" => tools::handle_analyze_file(state.clone(), normalized_args).await,
            "chronos_query_hotspots" => tools::handle_query_hotspots(state.clone(), normalized_args).await,
            "chronos_blast_radius" => tools::handle_blast_radius(state.clone(), normalized_args).await,
            "chronos_mutation_gate" => tools::handle_mutation_gate(state.clone(), normalized_args).await,
            "chronos_contract_status" => tools::handle_contract_status(state.clone(), normalized_args).await,
            "chronos_query_graph" => tools::handle_query_graph(state.clone(), normalized_args).await,
            "chronos_search_graph" => tools::handle_search_graph(state.clone(), normalized_args).await,
            "chronos_get_snippet" => tools::handle_get_snippet(state.clone(), normalized_args).await,
            "chronos_get_architecture" => tools::handle_get_architecture(state.clone(), normalized_args).await,
            "chronos_hydrate_telemetry" => tools::handle_hydrate_telemetry(state.clone(), normalized_args).await,
            "chronos_generate_agents_md" => tools::handle_generate_agents_md(state.clone(), normalized_args).await,
            "chronos_explore" => tools::handle_explore(state.clone(), normalized_args).await,
            "chronos_edit_symbol" => tools::handle_edit_symbol(state.clone(), normalized_args).await,
            "chronos_refactor_symbol" => tools::handle_refactor_symbol(state.clone(), normalized_args).await,
            _ => {
                return Err(JsonRpcError {
                    code: -32601,
                    message: format!("Tool not found: {}", tool_name),
                    data: None,
                });
            }
        };

        // Pre-Tool / Post-Call Contract Alert Gate: attach any pending breaking-change alerts
        // detected by background watcher / previous edits onto `_meta.contract_alerts`.
        if tool_name != "chronos_contract_status" {
            let alerts = {
                let st = state.read().await;
                st.contracts.drain_pending_alerts().await
            };
            if !alerts.is_empty() {
                let alert_summaries: Vec<String> = alerts
                    .iter()
                    .map(|a| a.notification_message.clone())
                    .collect();
                let mut meta_obj = match result.meta.take() {
                    Some(Value::Object(m)) => m,
                    _ => serde_json::Map::new(),
                };
                meta_obj.insert("contract_alerts".to_string(), json!(alert_summaries));
                result.meta = Some(Value::Object(meta_obj));

                if let Some(first_item) = result.content.first_mut() {
                    let banner = format!(
                        "\n\n⚠️ **CONTRACT ALERTS ({}):**\n{}",
                        alert_summaries.len(),
                        alert_summaries.join("\n")
                    );
                    first_item.text.push_str(&banner);
                }
            }
        }

        // Apply universal `max_answer_chars` character budget
        if let Some(limit) = max_chars {
            for item in &mut result.content {
                if item.text.chars().count() > limit {
                    item.text = truncate_to_budget(&item.text, limit);
                }
            }
        }

        Ok(result)
    }
}

fn inject_default_arg(args: Option<Value>, key: &str, value: &str) -> Value {
    let mut map = match args {
        Some(Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    if !map.contains_key(key) {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::new_shared_state_with_options;
    use std::path::PathBuf;

    struct TempWorkspace(PathBuf);
    impl TempWorkspace {
        fn new(suffix: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("chronos_mcp_reg_{suffix}_{nanos}"));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn test_readonly_blocks_disk_write_and_allows_preview() {
        let ws = TempWorkspace::new("ro");
        std::fs::write(
            ws.0.join("sample.ts"),
            "export function greet(name: string): string { return name; }\n",
        )
        .unwrap();

        let state = new_shared_state_with_options(ws.0.clone(), true);
        let denied = McpToolRegistry::dispatch(
            state.clone(),
            "chronos_generate_agents_md",
            Some(json!({ "target": "codex", "write_to_disk": true, "auto_index": true })),
        )
        .await
        .unwrap();
        assert_eq!(denied.is_error, Some(true));
        assert!(denied.content[0].text.contains("--readonly"));

        let preview = McpToolRegistry::dispatch(
            state,
            "chronos_generate_agents_md",
            Some(json!({ "target": "codex", "write_to_disk": false, "auto_index": true })),
        )
        .await
        .unwrap();
        assert_ne!(preview.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_max_answer_chars_budget_truncates_large_output() {
        let ws = TempWorkspace::new("budget");
        std::fs::write(
            ws.0.join("app.ts"),
            "export function alpha(x: number) { return x + 1; }\nexport function beta(y: number) { return alpha(y); }\n",
        )
        .unwrap();

        let state = new_shared_state_with_options(ws.0.clone(), true);
        let res = McpToolRegistry::dispatch(
            state,
            "chronos_explore",
            Some(json!({ "query": "alpha", "max_answer_chars": 360 })),
        )
        .await
        .unwrap();
        assert!(res.content[0].text.chars().count() <= 360);
        assert!(res.content[0].text.contains("TRUNCATED by CHRONOS ToolRegistry"));
    }

    #[tokio::test]
    async fn test_contract_alert_middleware_attaches_to_meta() {
        let ws = TempWorkspace::new("alerts");
        let file = ws.0.join("api.ts");
        std::fs::write(&file, "export function calcPrice(a: number): number { return a; }\n").unwrap();

        let state = new_shared_state_with_options(ws.0.clone(), true);
        McpToolRegistry::dispatch(
            state.clone(),
            "chronos_analyze_file",
            Some(json!({ "file_path": "api.ts" })),
        )
        .await
        .unwrap();

        // Breaking parameter addition triggers ContractEngine alert on second index
        std::fs::write(
            &file,
            "export function calcPrice(a: number, taxRate: number): number { return a * taxRate; }\n",
        )
        .unwrap();
        let after_break = McpToolRegistry::dispatch(
            state.clone(),
            "chronos_analyze_file",
            Some(json!({ "file_path": "api.ts" })),
        )
        .await
        .unwrap();
        let meta = after_break.meta.expect("breaking change must populate _meta");
        assert!(meta["contract_alerts"].as_array().unwrap().len() >= 1);
    }

    #[tokio::test]
    async fn test_v18_14_tools_readonly_guards_and_explore_script() {
        assert_eq!(McpToolRegistry::list_tools(false).len(), 14);
        assert_eq!(McpToolRegistry::list_tools(true).len(), 12);
        assert!(McpToolRegistry::list_tools(false).iter().all(|t| t.annotations.is_some()));

        let ws = TempWorkspace::new("v18_guards");
        let file = ws.0.join("svc.rs");
        std::fs::write(
            &file,
            "pub fn greet(x: i32) -> i32 {\n    x + 1\n}\n",
        )
        .unwrap();

        let state_ro = new_shared_state_with_options(ws.0.clone(), true);

        // 1. --readonly blocks chronos_edit_symbol & aliases and mutating refactor actions
        let edit_blocked = McpToolRegistry::dispatch(
            state_ro.clone(),
            "chronos_edit_symbol",
            Some(json!({
                "symbol": "greet",
                "file": "svc.rs",
                "operation": "replace_body",
                "new_code": "pub fn greet(x: i32) -> i32 { x + 2 }"
            })),
        )
        .await
        .unwrap();
        assert_eq!(edit_blocked.is_error, Some(true));
        assert!(edit_blocked.content[0].text.contains("--readonly"));

        let alias_blocked = McpToolRegistry::dispatch(
            state_ro.clone(),
            "chronos_replace_symbol_body",
            Some(json!({
                "symbol": "greet",
                "file": "svc.rs",
                "new_code": "pub fn greet(x: i32) -> i32 { x + 2 }"
            })),
        )
        .await
        .unwrap();
        assert_eq!(alias_blocked.is_error, Some(true));

        let rename_blocked = McpToolRegistry::dispatch(
            state_ro.clone(),
            "chronos_rename_symbol",
            Some(json!({ "symbol": "greet", "new_name": "welcome", "file": "svc.rs" })),
        )
        .await
        .unwrap();
        assert_eq!(rename_blocked.is_error, Some(true));

        // 2. --readonly allows chronos_refactor_symbol(preview_usages)
        let preview_ok = McpToolRegistry::dispatch(
            state_ro.clone(),
            "chronos_refactor_symbol",
            Some(json!({ "symbol": "greet", "action": "preview_usages", "file": "svc.rs" })),
        )
        .await
        .unwrap();
        assert_ne!(preview_ok.is_error, Some(true));
        assert!(preview_ok.content[0].text.contains("\"status\": \"preview\""));

        // 3. ReadWrite MCP allows chronos_explore.script read queries, but STILL blocks write functions
        let state_rw = new_shared_state_with_options(ws.0.clone(), false);
        let script_ok = McpToolRegistry::dispatch(
            state_rw.clone(),
            "chronos_explore",
            Some(json!({
                "script": "explore(\"greet\").symbols_returned"
            })),
        )
        .await
        .unwrap();
        assert_ne!(script_ok.is_error, Some(true));
        let script_json: Value = serde_json::from_str(&script_ok.content[0].text).unwrap();
        assert_eq!(script_json["script_result"], 1);

        let script_inject_blocked = McpToolRegistry::dispatch(
            state_rw,
            "chronos_explore",
            Some(json!({
                "script": "edit_symbol(\"greet\", \"replace_body\", \"pub fn greet(x: i32) -> i32 { 0 }\", \"svc.rs\")"
            })),
        )
        .await
        .unwrap();
        assert_eq!(script_inject_blocked.is_error, Some(true));
    }
}
