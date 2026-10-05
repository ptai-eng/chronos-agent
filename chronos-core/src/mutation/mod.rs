//! # Mutation Testing Arena
//!
//! Pillar 2: Eradicates "TDD hallucination" — AI-written tests that pass
//! but detect zero real bugs.
//!
//! ## Architecture (Ratified Spec)
//! - **Safe AST Mutators:** Only mutate safe operator patterns (no loop body changes)
//! - **Graph-Guided Test Selector:** Use 4D graph to find the 2-3 tests that actually
//!   exercise the mutated code path
//! - **In-process isolation:** tokio::time::timeout (not child process per mutant)
//! - **Mutation Score:** Kill rate must be >= 90% to pass quality gate

pub mod arena;
pub mod operators;
pub mod runner;
pub mod sandbox;
pub mod selector;
pub mod watchdog;

pub use arena::{MutationArena, MutationReport};
pub use operators::{Mutant, MutantKind};
