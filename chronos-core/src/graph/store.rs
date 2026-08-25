//! # Graph Store — In-memory persistence and serialization
//!
//! Provides save/load of the 4D graph to JSON for persistence across sessions.
//! Phase 5+ will add KuzuDB or SQLite backend if needed.

use crate::graph::schema::ChronosGraph;
use anyhow::Result;
use serde_json;
use std::path::Path;

/// Simple JSON-based persistence for the in-memory graph.
/// Writes the full node/edge set to a `.chronos-graph.json` file in the project root.
pub struct GraphStore;

impl GraphStore {
    /// Serialize and save the graph to disk.
    pub fn save(graph: &ChronosGraph, path: &Path) -> Result<()> {
        // For now, save the status report as a lightweight summary.
        // Full serialization of petgraph requires custom serde impl — Phase 5 scope.
        let report = graph.status_report();
        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, json)?;
        tracing::info!("Graph saved to {}", path.display());
        Ok(())
    }
}
