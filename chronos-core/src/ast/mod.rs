//! # AST Parsing Module
//!
//! Phase 1: Extract static code structure from TypeScript/Python/Rust source files
//! using Tree-sitter for high-speed incremental parsing.
//!
//! ## Supported languages
//! - TypeScript/JavaScript (.ts, .tsx, .js, .mjs, .jsx)
//! - Python (.py) — Flask, FastAPI route detection
//! - Rust (.rs) — struct, fn, impl, test module detection
//!
//! ## What we extract
//! - Function declarations & arrow functions
//! - Class/struct definitions & methods
//! - Import/use statements
//! - Call expressions (caller → callee relationships)
//! - API route registrations (Express.js, Flask, FastAPI)
//! - Test describe/it blocks & #[test] / def test_* (for test node linking)

pub mod diff;
pub mod dispatcher;
pub mod heuristics;
pub mod polyglot_parser;
pub mod py_parser;
pub mod rs_parser;
pub mod ts_parser;

pub use dispatcher::LanguageDispatcher;
pub use polyglot_parser::{CppParser, GoParser, JavaParser};
pub use ts_parser::{ParsedModule, TypeScriptParser};
