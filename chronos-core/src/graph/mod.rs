//! # 4D Temporal Knowledge Graph
//!
//! This module defines the schema and storage engine for the CHRONOS 4D Graph.
//!
//! ## Dimensions
//! - **Dimension 1-3 (Space):** Static AST topology — Files, Classes, Functions, call relationships
//! - **Dimension 4 (Time/Energy):** Runtime telemetry overlay — CPU cycles, RAM delta, latency
//!
//! ## Design: Start Lean (Ratified Spec)
//! Using Rust HashMap + petgraph in-memory. No embedded DB until Phase 5.


pub mod query;
pub mod schema;
pub mod store;
pub mod persistent;

pub use schema::{ChronosGraph, EdgeKind, NodeId, NodeKind};
pub use persistent::PersistentGraph;
