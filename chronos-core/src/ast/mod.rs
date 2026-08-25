//! # AST Parsing Module
//!
//! Phase 1: Extract static code structure from TypeScript/Python source files
//! using Tree-sitter for high-speed incremental parsing.
//!
//! ## Supported languages (MVP)
//! - TypeScript/JavaScript (primary)
//! - Python (fixture/secondary)
//!
//! ## What we extract
//! - Function declarations & arrow functions
//! - Class definitions & methods  
//! - Import/export statements
//! - Call expressions (caller → callee relationships)
//! - Express.js API route registrations
//! - Test describe/it blocks (for test node linking)

pub mod diff;
pub mod heuristics;
pub mod ts_parser;

pub use ts_parser::{ParsedModule, TypeScriptParser};
