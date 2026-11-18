//! # Language Dispatcher
//!
//! Routes a file to the correct language parser based on its extension.
//! Produces a unified `ParsedModule` regardless of language.

use crate::ast::polyglot_parser::{CppParser, GoParser, JavaParser};
use crate::ast::py_parser::PythonParser;
use crate::ast::rs_parser::RustParser;
use crate::ast::ts_parser::{ParsedModule, TypeScriptParser};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tree_sitter::{Language, Parser};

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

    /// Return the Tree-sitter `Language` grammar for `path`'s extension.
    pub fn language_for_path(path: &Path) -> Result<Language> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "ts" | "js" | "mjs" | "cjs" => Ok(tree_sitter_typescript::language_typescript()),
            "tsx" | "jsx" => Ok(tree_sitter_typescript::language_tsx()),
            "py" => Ok(tree_sitter_python::language()),
            "rs" => Ok(tree_sitter_rust::language()),
            "go" => Ok(tree_sitter_go::language()),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => Ok(tree_sitter_cpp::language()),
            "java" => Ok(tree_sitter_java::language()),
            other => Err(anyhow!(
                "Unsupported file extension '.{}'. Supported: .ts .tsx .js .py .rs .go .c .h .cpp .hpp .java",
                other
            )),
        }
    }

    /// Check in-memory source code for syntax errors before touching disk (returns 1-indexed `(line, col)`).
    pub fn check_syntax_errors(path: &Path, source: &str) -> Result<Option<(usize, usize)>> {
        let lang = Self::language_for_path(path)?;
        let mut parser = Parser::new();
        parser
            .set_language(&lang)
            .context("Failed to initialize Tree-sitter language for syntax check")?;
        let tree = parser
            .parse(source, None)
            .context("Tree-sitter parse returned None")?;
        let root = tree.root_node();
        if !root.has_error() {
            return Ok(None);
        }
        Ok(Some(find_first_error_pos(root)))
    }

    /// Parse any supported in-memory source buffer into a `ParsedModule` across all 7 language families.
    pub fn parse_source(&mut self, path: &Path, source: &str) -> Result<ParsedModule> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "ts" | "tsx" | "js" | "mjs" | "cjs" | "jsx" => self.ts.parse_source(path, source),
            "py" => self.py.parse_str(source, path),
            "rs" => self.rs.parse_str(source, path),
            "go" => self.go.parse_source(path, source),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" => self.cpp.parse_source(path, source),
            "java" => self.java.parse_source(path, source),
            other => Err(anyhow!(
                "Unsupported file extension '.{}'. Supported: .ts .tsx .js .py .rs .go .c .h .cpp .hpp .java",
                other
            )),
        }
    }

    /// Parse any supported source file from disk into a `ParsedModule`.
    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read source file: {:?}", path))?;
        self.parse_source(path, &source)
    }

    pub fn is_supported(path: &Path) -> bool {
        Self::language_for_path(path).is_ok()
    }
}

fn find_first_error_pos(node: tree_sitter::Node<'_>) -> (usize, usize) {
    if node.is_error() || node.is_missing() {
        let p = node.start_position();
        return (p.row + 1, p.column + 1);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() {
            return find_first_error_pos(child);
        }
    }
    let p = node.start_position();
    (p.row + 1, p.column + 1)
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
