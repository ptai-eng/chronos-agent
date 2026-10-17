//! # Language Dispatcher
//!
//! Routes a file to the correct language parser based on its extension.
//! Produces a unified `ParsedModule` regardless of language.

use crate::ast::polyglot_parser::{CppParser, GoParser, JavaParser};
use crate::ast::py_parser::PythonParser;
use crate::ast::rs_parser::RustParser;
use crate::ast::ts_parser::{ParsedModule, TypeScriptParser};
use anyhow::{anyhow, Result};
use std::path::Path;

/// Unified multi-language parser dispatcher (7 language families: TS/JS, Python, Rust, Go, C, C++, Java).
pub struct LanguageDispatcher {
    ts: TypeScriptParser,
    py: PythonParser,
    rs: RustParser,
    go: GoParser,
    cpp: CppParser,
    java: JavaParser,
}

impl LanguageDispatcher {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ts: TypeScriptParser::new()?,
            py: PythonParser::new()?,
            rs: RustParser::new()?,
            go: GoParser::new()?,
            cpp: CppParser::new()?,
            java: JavaParser::new()?,
        })
    }

    /// Parse any supported source file into a `ParsedModule`.
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "ts" | "tsx" | "js" | "mjs" | "cjs" | "jsx" => self.ts.parse_file(path),
            "py" => self.py.parse_file(path),
            "rs" => self.rs.parse_file(path),
            "go" => self.go.parse_file(path),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => self.cpp.parse_file(path),
            "java" => self.java.parse_file(path),
            other => Err(anyhow!(
                "Unsupported file extension '.{}'. Supported: .ts .tsx .js .py .rs .go .c .h .cpp .hpp .java",
                other
            )),
        }
    }

    /// Check if a path is a supported source file
    pub fn is_supported(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some(
                "ts"
                    | "tsx"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "jsx"
                    | "py"
                    | "rs"
                    | "go"
                    | "c"
                    | "h"
                    | "cc"
                    | "cpp"
                    | "cxx"
                    | "hpp"
                    | "java"
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        assert!(LanguageDispatcher::is_supported(Path::new("foo.ts")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.py")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.rs")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.go")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.cpp")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.c")));
        assert!(LanguageDispatcher::is_supported(Path::new("foo.java")));
        assert!(!LanguageDispatcher::is_supported(Path::new("foo.rb")));
    }
}
