//! # MCP ToolRegistry & Middleware
//!
//! Bridges `chronos_core::registry::ToolRegistry` into `chronos-mcp`:
//! - Generates `tools/list` from the canonical core specification
//! - Enforces `--readonly` mode guards
//! - Dispatches `tools/call` across all 12 tools
//! - Automatically drains pending `ContractEngine` alerts into `_meta.contract_alerts`
//! - Applies universal `max_answer_chars` (default 24,000) UTF-8 safe response budgeting

use crate::{
    protocol::{CallToolResult, JsonRpcError, McpTool},
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
            })
            .collect()
    }

    /// Dispatch an MCP `tools/call` request through read-only, alert, and budget middleware.
    pub async fn dispatch(
        state: SharedState,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> std::result::Result<CallToolResult, JsonRpcError> {
        let Some(spec) = ToolRegistry::find(tool_name) else {
            warn!("Unknown tool called: {}", tool_name);
            return Err(JsonRpcError {
                code: -32601,
                message: format!("Tool not found: {}", tool_name),
                data: None,
            });
        };

        let readonly = {
            let st = state.read().await;
            st.readonly
        };
        if readonly && !spec.readonly {
            return Ok(CallToolResult::error(format!(
                "Security: tool '{}' is disabled in --readonly mode.",
                tool_name
            )));
        }

        let max_chars = resolve_max_answer_chars(arguments.as_ref());

        let mut result = match tool_name {
            "chronos_analyze_file" => tools::handle_analyze_file(state.clone(), arguments).await,
            "chronos_query_hotspots" => tools::handle_query_hotspots(state.clone(), arguments).await,
            "chronos_blast_radius" => tools::handle_blast_radius(state.clone(), arguments).await,
            "chronos_mutation_gate" => tools::handle_mutation_gate(state.clone(), arguments).await,
            "chronos_contract_status" => tools::handle_contract_status(state.clone(), arguments).await,
            "chronos_query_graph" => tools::handle_query_graph(state.clone(), arguments).await,
            "chronos_search_graph" => tools::handle_search_graph(state.clone(), arguments).await,
            "chronos_get_snippet" => tools::handle_get_snippet(state.clone(), arguments).await,
            "chronos_get_architecture" => tools::handle_get_architecture(state.clone(), arguments).await,
            "chronos_hydrate_telemetry" => tools::handle_hydrate_telemetry(state.clone(), arguments).await,
            "chronos_generate_agents_md" => tools::handle_generate_agents_md(state.clone(), arguments).await,
            "chronos_explore" => tools::handle_explore(state.clone(), arguments).await,
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
}
