//! # Semantic Contract Broker
//!
//! Pillar 3: Real-time multi-agent API contract synchronization.
//!
//! ## Architecture (Ratified Spec)
//! - **Pre-Tool Execution Gate:** Never interrupt LLM stream. Inject alerts
//!   into agent context at the next tool call boundary.
//! - **AST Contract Differ:** Distinguish public contract changes from
//!   internal implementation changes.
//! - **File Watcher:** notify-rs with 30ms debounce.

//! # Semantic Contract Broker & Polyglot ContractEngine
//!
//! Pillar 3: Real-time 7-language AST contract tracking across agents & edits.

pub mod broker;
pub mod signature;

pub use broker::{ContractBroker, ContractEngine, ContractEvent};
