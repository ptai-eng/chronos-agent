//! # Tool: chronos_contract_status
//!
//! Drain the Pre-Tool Execution Gate and return pending contract alerts.

use crate::protocol::CallToolResult;
use crate::state::SharedState;
use serde_json::Value;

pub async fn handle_contract_status(state: SharedState, _params: Option<Value>) -> CallToolResult {
    let st = state.read().await;

    // Drain all pending alerts (Pre-Tool Execution Gate)
    let alerts_text = st.broker.format_pending_alerts_for_agent().await;

    match alerts_text {
        Some(alert_msg) => {
            // There are pending breaking-change alerts
            let output = format!(
                r#"# ⚠️ CHRONOS Contract Alerts — Action Required

{}

---

> **What happened?** The CHRONOS Semantic Contract Broker detected breaking API signature changes
> while watching your workspace. These were queued and are now delivered to you before any
> further code operations.
>
> **What to do:** Review each alert above, then update all callers of the changed functions
> before proceeding with your task."#,
                alert_msg
            );
            CallToolResult::text(output)
        }
        None => {
            // No pending alerts — report clean status
            let report = st.graph.status_report();
            let output = format!(
                r#"✅ **CHRONOS Contract Broker — All Clear**

No pending contract violation alerts.

**Broker Status:**
- Graph nodes monitored: {}
- Contract signatures indexed: {}
- Hot-path functions: {}
- Bottleneck functions: {}

> The Pre-Tool Execution Gate is clear. You can proceed with your coding task safely."#,
                report.total_nodes,
                report.total_nodes, // signatures ≈ nodes indexed
                report.hot_paths,
                report.bottlenecks,
            );
            CallToolResult::text(output)
        }
    }
}
