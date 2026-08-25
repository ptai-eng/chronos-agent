//! # chronos-core
//!
//! The core engine of CHRONOS-AGENT: A 4-dimensional knowledge system for AI coding agents
//! that transforms them from "static text guessers" into agents with true runtime sensation.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │           CHRONOS 4D-KNOWLEDGE GRAPH             │
//! │   [Static AST] + [Runtime Telemetry] + [Memory]  │
//! └────────────────────┬─────────────────────────────┘
//!                      │
//!      ┌───────────────┼───────────────┐
//!      ▼               ▼               ▼
//! ┌─────────┐   ┌──────────────┐   ┌──────────────┐
//! │Mutation │   │  Contract    │   │ Blast-Radius │
//! │  Arena  │   │   Broker     │   │   Heatmap    │
//! └─────────┘   └──────────────┘   └──────────────┘
//! ```
//!
//! ## 4 Pillars
//! 1. **4D Knowledge Graph** — Static AST + Live Runtime Telemetry overlay
//! 2. **Mutation Arena** — Targeted fault injection with graph-guided test selection
//! 3. **Semantic Contract Broker** — Real-time multi-agent API contract synchronization
//! 4. **Blast-Radius Heatmap** — Visual impact analysis for Senior Reviewers

pub mod ast;
pub mod blast_radius;
pub mod contract;
pub mod graph;
pub mod mutation;
pub mod telemetry;

// Re-export primary types for convenience
pub use graph::{ChronosGraph, EdgeKind, NodeId, NodeKind};
pub use telemetry::TelemetryData;
