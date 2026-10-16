//! # Rust AST Parser
//!
//! Uses tree-sitter-rust to extract symbols from Rust source files.
//! Detects functions, structs, impls, test modules, and use statements.

use crate::ast::ts_parser::{CallRef, ImportRef, ParsedModule};
use crate::graph::schema::{
    ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics,
};
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

pub struct RustParser {
    parser: Parser,
}

impl RustParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::language())
            .context("Failed to load tree-sitter-rust grammar")?;
        Ok(Self { parser })
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read Rust file: {:?}", path))?;
        self.parse_str(&source, path)
    }

    pub fn parse_str(&mut self, source: &str, path: &Path) -> Result<ParsedModule> {
        let tree = self
            .parser
            .parse(source, None)
            .context("tree-sitter-rust parse returned None")?;

        let root = tree.root_node();
        let src_bytes = source.as_bytes();
        let mut module = ParsedModule::new(path.to_path_buf());
        module.sloc = source.lines().filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        }).count();

        self.visit_node(root, src_bytes, path, &mut module, false, None);
        Ok(module)
    }

    fn make_node(
        &self,
        tree_node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        name: &str,
        kind: NodeKind,
        param_count: u32,
        _is_exported: bool, // reserved: schema has no visibility field yet
    ) -> ChronosNode {
        let id = NodeId::new(&path.to_string_lossy(), name, &kind);
        let qualified_name = format!(
            "{}::{}",
            path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
            name
        );
        let location = SourceLocation {
            file: path.to_path_buf(),
            start_line: tree_node.start_position().row + 1,
            start_col: tree_node.start_position().column,
            end_line: tree_node.end_position().row + 1,
            end_col: tree_node.end_position().column,
        };
        let fn_text = tree_node.utf8_text(src).unwrap_or("");
        let mut cyc = 1u32;
        for kw in &["if ", "else if", "for ", "while ", "loop ", "match ", "&&", "||", "?"] {
            cyc += fn_text.matches(kw).count() as u32;
        }
        let sloc = fn_text
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with("//")
            })
            .count() as u32;

        let mut heuristics = StaticHeuristics {
            cyclomatic_complexity: cyc,
            param_count,
            sloc,
            ..StaticHeuristics::default()
        };
        heuristics.compute_cognitive_load();

        let mut node = ChronosNode::new(id, kind, name, qualified_name, location);
        node.heuristics = heuristics;
        node
    }

    fn visit_node(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        module: &mut ParsedModule,
        in_test_mod: bool,
        current_function: Option<&str>,
    ) {
        let mut next_fn_owned: Option<String> = None;

        match node.kind() {
            "function_item" => {
                next_fn_owned = self.extract_function(node, src, path, module, in_test_mod);
            }
            "struct_item" => {
                self.extract_struct(node, src, path, module);
            }
            "use_declaration" => {
                self.extract_use(node, src, path, module);
            }
            "call_expression" => {
                self.extract_call(node, src, module, current_function);
            }
            "token_tree" => {
                if let Some(caller) = current_function {
                    let mut cursor = node.walk();
                    let children: Vec<_> = node.children(&mut cursor).collect();
                    for window in children.windows(2) {
                        if window[0].kind() == "identifier" && window[1].kind() == "token_tree" {
                            if let Ok(callee_name) = window[0].utf8_text(src) {
                                if !callee_name.is_empty() && caller != callee_name {
                                    module.call_refs.push(CallRef {
                                        caller_name: caller.to_string(),
                                        callee_name: callee_name.to_string(),
                                        call_site_line: window[0].start_position().row + 1,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        let is_test_mod = in_test_mod || self.is_test_module(node, src);
        let active_fn = next_fn_owned.as_deref().or(current_function);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, src, path, module, is_test_mod, active_fn);
        }
    }

    fn is_test_module(&self, node: tree_sitter::Node, src: &[u8]) -> bool {
        if node.kind() != "mod_item" {
            return false;
        }
        let parent = match node.parent() {
            Some(p) => p,
            None => return false,
        };
        let mut cursor = parent.walk();
        for sibling in parent.children(&mut cursor) {
            if sibling.id() == node.id() { break; }
            if sibling.kind() == "attribute_item" {
                let text = sibling.utf8_text(src).unwrap_or("");
                if text.contains("cfg(test)") { return true; }
            }
        }
        false
    }

    fn extract_function(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        module: &mut ParsedModule,
        in_test_mod: bool,
    ) -> Option<String> {
        let name = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())?
            .to_string();

        let is_test = in_test_mod || name.starts_with("test_");
        let is_exported = node.child_by_field_name("visibility_modifier").is_some();
        let param_count = node
            .child_by_field_name("parameters")
            .map(|p| p.child_count().saturating_sub(2) as u32)
            .unwrap_or(0);

        let kind = if is_test { NodeKind::TestCase } else { NodeKind::Function };
        let chronos_node = self.make_node(node, src, path, &name, kind, param_count, is_exported);

        if is_test {
            module.test_cases.push(chronos_node);
        } else {
            module.functions.push(chronos_node);
        }
        Some(name)
    }

    fn extract_struct(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        module: &mut ParsedModule,
    ) {
        let name = match node.child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
        {
            Some(n) => n.to_string(),
            None => return,
        };

        let is_exported = node.child_by_field_name("visibility_modifier").is_some();
        let chronos_node = self.make_node(node, src, path, &name, NodeKind::Class, 0, is_exported);
        module.classes.push(chronos_node);
    }

    fn extract_use(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        module: &mut ParsedModule,
    ) {
        let text = match node.utf8_text(src) {
            Ok(t) => t.to_string(),
            Err(_) => return,
        };

        let text_clean = text.trim_start_matches("use ").trim_end_matches(';');
        let parts: Vec<&str> = text_clean.rsplitn(2, "::").collect();
        let (imported, source_mod) = if parts.len() == 2 {
            (parts[0].trim_matches(|c| c == '{' || c == '}').to_string(), parts[1].to_string())
        } else {
            (text_clean.to_string(), text_clean.to_string())
        };

        module.import_refs.push(ImportRef {
            from_file: path.to_path_buf(),
            imported_names: imported.split(',').map(|s| s.trim().to_string()).collect(),
            source_module: source_mod,
        });
    }

    fn extract_call(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        module: &mut ParsedModule,
        current_function: Option<&str>,
    ) {
        let func_node = match node.child_by_field_name("function") {
            Some(n) => n,
            None => return,
        };

        if matches!(
            func_node.kind(),
            "index_expression" | "parenthesized_expression" | "call_expression"
        ) {
            module.unresolved_call_sites += 1;
            return;
        }

        let callee = match func_node.utf8_text(src) {
            Ok(t) => t
                .rsplit(|c| c == ':' || c == '.')
                .next()
                .unwrap_or(t)
                .trim()
                .to_string(),
            Err(_) => return,
        };

        if callee.is_empty() || callee.len() > 64 {
            return;
        }

        if let Some(caller) = current_function {
            module.call_refs.push(CallRef {
                caller_name: caller.to_string(),
                callee_name: callee,
                call_site_line: node.start_position().row + 1,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RS: &str = r#"
use std::collections::HashMap;

pub struct UserService {
    db: HashMap<u64, String>,
}

impl UserService {
    pub fn new() -> Self {
        Self { db: HashMap::new() }
    }

    pub async fn get_user(&self, id: u64) -> Option<&String> {
        self.db.get(&id)
    }
}

pub fn process_order(id: u64) -> bool {
    id > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_order() {
        assert!(process_order(1));
    }
}
"#;

    #[test]
    fn test_rust_parser_functions() {
        let mut parser = RustParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_RS, Path::new("test.rs"))
            .expect("parse");

        let fn_names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(fn_names.iter().any(|n| *n == "process_order"), "missing process_order: {:?}", fn_names);
    }

    #[test]
    fn test_rust_parser_structs() {
        let mut parser = RustParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_RS, Path::new("test.rs"))
            .expect("parse");

        assert!(module.classes.iter().any(|c| c.name == "UserService"), "missing UserService");
    }

    #[test]
    fn test_rust_parser_tests() {
        let mut parser = RustParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_RS, Path::new("test.rs"))
            .expect("parse");

        assert!(!module.test_cases.is_empty(), "should detect test functions");
    }
}
