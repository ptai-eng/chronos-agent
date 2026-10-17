//! # Telemetry Module — Runtime Measurement Engine
//!
//! The 4th Dimension of the CHRONOS Knowledge Graph.
//!
//! This module transforms a static code map into a living, breathing
//! runtime representation by capturing actual execution characteristics.
//!
//! ## Dual Backend Architecture (Ratified Spec)
//! - **Windows/macOS/Cross-platform:** CDP WebSocket (Node.js `--inspect`) via V8 Profiler
//! - **Linux/WSL2:** eBPF uprobe attachment (stub skeleton for Phase 2+)
//!
//! ## Data flow
//! ```text
//! [Test Runner] --inspect --> [V8 Inspector WebSocket]
//!                                       │
//!                               [CDP Adapter (Rust)]
//!                                       │
//!                               [TelemetryCollector]
//!                                       │
//!                              [Graph Hydration Engine]
//!                                       │
//!                             [ChronosGraph 4D Nodes]
//! ```

pub mod adapter;
pub mod cdp_client;
pub mod hydration;
pub mod ebpf_adapter;
pub mod profile_loader;

pub use adapter::{TelemetryAdapter, TelemetryData, TelemetrySession};
pub use profile_loader::{load_profile_from_file, parse_profile_str};
