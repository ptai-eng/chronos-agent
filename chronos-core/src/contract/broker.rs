//! # Contract Broker — Multi-Agent Coordination Hub
//!
//! The central coordinator for the Semantic Contract Broker system.
//!
//! ## Pre-Tool Execution Gate (Ratified Spec)
//! Alerts are NOT pushed mid-stream. Instead they are queued and
//! returned as the FIRST thing in the next MCP tool call response.
//! This ensures LLM context is never corrupted by abrupt interruption.

use crate::contract::signature::ModuleSignature;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

/// A contract violation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEvent {
    pub file_path: String,
    pub detected_at: DateTime<Utc>,
    pub is_breaking: bool,
    pub notification_message: String,
    pub diff_summary: String,
}

/// The state of the contract broker
struct BrokerState {
    /// Last known signatures for each file
    signatures: HashMap<PathBuf, ModuleSignature>,
    /// Pending alerts for agents (Pre-Tool Execution Gate)
    pending_alerts: Vec<ContractEvent>,
}

/// The main Semantic Contract Broker.
///
/// Monitors workspace files, detects contract changes, and queues
/// alerts for injection at the next agent tool call boundary.
#[derive(Clone)]
pub struct ContractBroker {
    state: Arc<RwLock<BrokerState>>,
    event_sender: broadcast::Sender<ContractEvent>,
}

impl ContractBroker {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            state: Arc::new(RwLock::new(BrokerState {
                signatures: HashMap::new(),
                pending_alerts: vec![],
            })),
            event_sender: sender,
        }
    }

    /// Subscribe to contract violation events
    pub fn subscribe(&self) -> broadcast::Receiver<ContractEvent> {
        self.event_sender.subscribe()
    }

    /// Index a file's current signature (call this during initial parse)
    pub async fn index_file(&self, file_path: &Path) -> Result<()> {
        let source = tokio::fs::read_to_string(file_path).await?;
        let sig = ModuleSignature::from_source(&file_path.to_string_lossy(), &source);

        let mut state = self.state.write().await;
        state.signatures.insert(file_path.to_path_buf(), sig);
        debug!("Indexed signature for {}", file_path.display());
        Ok(())
    }

    /// Process a file change: compute signature diff and queue alert if needed.
    pub async fn on_file_changed(&self, file_path: &Path) -> Option<ContractEvent> {
        let source = match tokio::fs::read_to_string(file_path).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Cannot read changed file {}: {}", file_path.display(), e);
                return None;
            }
        };

        let new_sig = ModuleSignature::from_source(&file_path.to_string_lossy(), &source);

        let mut state = self.state.write().await;

        if let Some(old_sig) = state.signatures.get(file_path) {
            if let Some(diff) = old_sig.diff(&new_sig) {
                let event = ContractEvent {
                    file_path: file_path.to_string_lossy().to_string(),
                    detected_at: Utc::now(),
                    is_breaking: diff.is_breaking,
                    notification_message: diff.agent_notification(),
                    diff_summary: format!(
                        "{} changed, {} removed, {} added",
                        diff.changed_exports.len(),
                        diff.removed_exports.len(),
                        diff.added_exports.len()
                    ),
                };

                info!(
                    "Contract event: {} (breaking={})",
                    file_path.display(),
                    event.is_breaking
                );

                // Queue for Pre-Tool Gate injection
                state.pending_alerts.push(event.clone());

                // Also broadcast to all subscribers
                let _ = self.event_sender.send(event.clone());

                state.signatures.insert(file_path.to_path_buf(), new_sig);
                return Some(event);
            }
        } else {
            // New file — just index it
            state.signatures.insert(file_path.to_path_buf(), new_sig);
        }

        None
    }

    /// Pre-Tool Execution Gate: drain all pending alerts.
    ///
    /// Call this at the START of every MCP tool handler.
    /// If alerts are present, prepend them to the response context.
    pub async fn drain_pending_alerts(&self) -> Vec<ContractEvent> {
        let mut state = self.state.write().await;
        std::mem::take(&mut state.pending_alerts)
    }

    /// Format pending alerts for injection into an MCP tool response
    pub async fn format_pending_alerts_for_agent(&self) -> Option<String> {
        let alerts = self.drain_pending_alerts().await;
        if alerts.is_empty() { return None; }

        let messages: Vec<String> = alerts.iter()
            .map(|a| a.notification_message.clone())
            .collect();

        Some(format!(
            "⚠️⚠️  CHRONOS CONTRACT ALERTS (READ BEFORE PROCEEDING) ⚠️⚠️\n\n{}\n\n{}",
            messages.join("\n\n---\n\n"),
            "═══ End of Contract Alerts — Resolve before continuing ═══"
        ))
    }

    /// Get current signature for a file (for direct comparison)
    pub async fn get_signature(&self, file_path: &Path) -> Option<ModuleSignature> {
        let state = self.state.read().await;
        state.signatures.get(file_path).cloned()
    }

    /// Check for contract conflicts between two files
    pub async fn check_conflict(&self, file_a: &Path, file_b: &Path) -> Option<String> {
        let state = self.state.read().await;
        let sig_a = state.signatures.get(file_a)?;
        let sig_b = state.signatures.get(file_b)?;

        // Check if B calls anything from A's exports
        // This is a simplified check — full analysis uses the 4D graph
        let _a_exports: std::collections::HashSet<&str> = sig_a.exports.keys()
            .map(|s| s.as_str())
            .collect();

        let _b_source_path = file_b.with_extension("").to_string_lossy().to_string();

        // Simplified: report if both files have exports that might conflict
        let conflicts: Vec<String> = sig_a.exports.keys()
            .filter(|name| sig_b.exports.contains_key(*name))
            .map(|name| format!("Duplicate export `{}`", name))
            .collect();

        if conflicts.is_empty() { return None; }

        Some(format!(
            "⚠️ Potential naming conflicts between {} and {}:\n{}",
            file_a.display(),
            file_b.display(),
            conflicts.join("\n")
        ))
    }
}

impl Default for ContractBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    const SOURCE_V1: &str = r#"
export async function getUser(id: string): Promise<User> {
    return db.find(id);
}
"#;

    const SOURCE_V2_BREAKING: &str = r#"
export async function getUser(id: string, tenantId: string): Promise<User> {
    return db.find(id, tenantId);
}
"#;

    #[tokio::test]
    async fn test_index_and_detect_change() {
        let broker = ContractBroker::new();

        // Write v1 to temp file
        let mut temp = NamedTempFile::with_suffix(".ts").unwrap();
        temp.write_all(SOURCE_V1.as_bytes()).unwrap();
        temp.flush().unwrap();

        // Index v1
        broker.index_file(temp.path()).await.unwrap();

        // Write v2 (breaking change)
        std::fs::write(temp.path(), SOURCE_V2_BREAKING).unwrap();

        // Process the change
        let event = broker.on_file_changed(temp.path()).await;
        assert!(event.is_some(), "Should detect contract change");
        let event = event.unwrap();
        assert!(event.is_breaking, "Should flag as breaking");
    }

    #[tokio::test]
    async fn test_pre_tool_gate_drains_alerts() {
        let broker = ContractBroker::new();

        let mut temp = NamedTempFile::with_suffix(".ts").unwrap();
        temp.write_all(SOURCE_V1.as_bytes()).unwrap();
        temp.flush().unwrap();
        broker.index_file(temp.path()).await.unwrap();

        // Trigger a breaking change
        std::fs::write(temp.path(), SOURCE_V2_BREAKING).unwrap();
        broker.on_file_changed(temp.path()).await;

        // First gate check should return alerts
        let alerts = broker.drain_pending_alerts().await;
        assert_eq!(alerts.len(), 1, "Should have 1 pending alert");

        // Second check should be empty (drained)
        let alerts2 = broker.drain_pending_alerts().await;
        assert_eq!(alerts2.len(), 0, "Should be empty after drain");
    }

    #[tokio::test]
    async fn test_no_event_for_internal_change() {
        let broker = ContractBroker::new();

        let mut temp = NamedTempFile::with_suffix(".ts").unwrap();
        temp.write_all(SOURCE_V1.as_bytes()).unwrap();
        temp.flush().unwrap();
        broker.index_file(temp.path()).await.unwrap();

        // Internal change: same signature, different implementation
        let internal_v2 = r#"
export async function getUser(id: string): Promise<User> {
    // Optimized with caching
    const cached = cache.get(id);
    if (cached) return cached;
    return db.find(id);
}
"#;

        std::fs::write(temp.path(), internal_v2).unwrap();
        let event = broker.on_file_changed(temp.path()).await;
        assert!(event.is_none(), "Should NOT generate event for internal change");
    }
}
