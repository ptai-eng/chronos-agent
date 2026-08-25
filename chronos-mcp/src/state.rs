//! # Shared Server State
//!
//! The global in-memory state shared across all MCP tool calls.
//! Wrapped in Arc<RwLock<>> for safe concurrent access.

use chronos_core::{
    contract::broker::ContractBroker,
    graph::{schema::ChronosGraph, persistent::PersistentGraph},
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

/// The shared CHRONOS runtime state
pub struct ChronosState {
    /// The 4D Knowledge Graph (in-memory)
    pub graph: ChronosGraph,
    /// The Semantic Contract Broker
    pub broker: ContractBroker,
    /// The workspace root being analyzed
    pub workspace_root: PathBuf,
    /// The optional persistent graph database
    pub db: Option<PersistentGraph>,
}

impl ChronosState {
    pub fn new(workspace_root: PathBuf) -> Self {
        let db_path = workspace_root.join(".chronos").join("graph.kuzu");
        std::fs::create_dir_all(db_path.parent().unwrap()).ok();
        let db = PersistentGraph::open(&db_path)
            .map_err(|e| tracing::warn!("KuzuDB unavailable: {} (using in-memory only)", e))
            .ok();
            
        let mut state = Self {
            graph: ChronosGraph::new(),
            broker: ContractBroker::new(),
            workspace_root,
            db,
        };
        
        if let Some(ref db) = state.db {
            if let Ok(count) = db.load_into_graph(&mut state.graph) {
                tracing::info!("Restored {} nodes from persisted graph", count);
            }
        }
        state
    }
}

/// Thread-safe handle to the shared state
pub type SharedState = Arc<RwLock<ChronosState>>;

pub fn new_shared_state(workspace_root: std::path::PathBuf) -> SharedState {
    Arc::new(RwLock::new(ChronosState::new(workspace_root)))
}
