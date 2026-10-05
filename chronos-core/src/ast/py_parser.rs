//! # Python AST Parser
//!
//! Uses tree-sitter-python to extract symbols from Python source files.
//! Mirrors the interface of `TypeScriptParser` and produces the same
//! `ParsedModule` output so the rest of the pipeline is language-agnostic.

use crate::ast::ts_parser::{CallRef, ImportRef, ParsedModule};
use crate::graph::schema::{
    ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics,
};
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

pub struct PythonParser {
    parser: Parser,
}

impl PythonParser {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::language())
            .context("Failed to load tree-sitter-python grammar")?;
        Ok(Self { parser })
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read Python file: {:?}", path))?;
        self.parse_str(&source, path)
    }

    pub fn parse_str(&mut self, source: &str, path: &Path) -> Result<ParsedModule> {
        let tree = self
            .parser
            .parse(source, None)
            .context("tree-sitter-python parse returned None")?;

        let root = tree.root_node();
        let src_bytes = source.as_bytes();
        let mut module = ParsedModule::new(path.to_path_buf());
        module.sloc = source.lines().filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        }).count();

        let mut cursor = root.walk();
        self.visit_node(root, src_bytes, path, source, &mut module, &mut cursor, None);

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
    ) -> ChronosNode {
        let id = NodeId::new(
            &path.to_string_lossy(),
            name,
            &kind,
        );
        let qualified_name = format!("{}::{}", path.file_stem()
            .and_then(|s| s.to_str()).unwrap_or(""), name);

        let location = SourceLocation {
            file: path.to_path_buf(),
            start_line: tree_node.start_position().row + 1,
            start_col: tree_node.start_position().column,
            end_line: tree_node.end_position().row + 1,
            end_col: tree_node.end_position().column,
        };

        let mut node = ChronosNode::new(id, kind, name, qualified_name, location);
        node.heuristics = StaticHeuristics {
            param_count,
            fan_out: self.count_calls_in_node(tree_node, src) as u32,
            ..StaticHeuristics::default()
        };
        node
    }

    fn count_calls_in_node(&self, node: tree_sitter::Node, _src: &[u8]) -> usize {
        let mut count = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call" { count += 1; }
        }
        count
    }

    fn visit_node(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        source: &str,
        module: &mut ParsedModule,
        _cursor: &mut tree_sitter::TreeCursor,
        parent_class: Option<&str>,
    ) {
        match node.kind() {
            "function_definition" => {
                self.extract_function(node, src, path, module, parent_class);
            }
            "class_definition" => {
                self.extract_class(node, src, path, module);
            }
            "decorated_definition" => {
                self.extract_decorated(node, src, path, source, module, parent_class);
                return; // children handled inside extract_decorated
            }
            "import_statement" | "import_from_statement" => {
                self.extract_import(node, src, path, module);
            }
            "call" => {
                self.extract_call(node, src, module);
            }
            _ => {}
        }

        let class_name = if node.kind() == "class_definition" {
            node.child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .map(|s| s.to_string())
        } else {
            None
        };

        let mut child_cursor = node.walk();
        for child in node.children(&mut child_cursor) {
            self.visit_node(
                child, src, path, source, module, _cursor,
                class_name.as_deref().or(parent_class),
            );
        }
    }

    fn extract_function(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        module: &mut ParsedModule,
        parent_class: Option<&str>,
    ) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = match name_node.utf8_text(src) {
            Ok(n) => n.to_string(),
            Err(_) => return,
        };

        let full_name = if let Some(class) = parent_class {
            format!("{}.{}", class, name)
        } else {
            name.clone()
        };

        let is_test = name.starts_with("test_") || name.starts_with("Test");
        let param_count = node
            .child_by_field_name("parameters")
            .map(|p| p.child_count().saturating_sub(2) as u32)
            .unwrap_or(0);

        let kind = if is_test { NodeKind::TestCase } else { NodeKind::Function };
        let chronos_node = self.make_node(node, src, path, &full_name, kind, param_count);

        if is_test {
            module.test_cases.push(chronos_node);
        } else {
            module.functions.push(chronos_node);
        }
    }

    fn extract_class(
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
        let chronos_node = self.make_node(node, src, path, &name, NodeKind::Class, 0);
        module.classes.push(chronos_node);
    }

    fn extract_decorated(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        path: &Path,
        source: &str,
        module: &mut ParsedModule,
        parent_class: Option<&str>,
    ) {
        let mut is_route = false;
        let mut route_path = String::new();
        let mut http_method = "GET".to_string();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let decorator_text = child.utf8_text(src).unwrap_or("").to_string();
                if decorator_text.contains(".route(")
                    || decorator_text.contains(".get(")
                    || decorator_text.contains(".post(")
                    || decorator_text.contains(".put(")
                    || decorator_text.contains(".delete(")
                    || decorator_text.contains(".patch(")
                {
                    is_route = true;
                    for m in &["get", "post", "put", "delete", "patch"] {
                        if decorator_text.contains(&format!(".{}(", m)) {
                            http_method = m.to_uppercase();
                            break;
                        }
                    }
                    if let Some(start) = decorator_text.find('"') {
                        if let Some(end) = decorator_text[start+1..].find('"') {
                            route_path = decorator_text[start+1..start+1+end].to_string();
                        }
                    } else if let Some(start) = decorator_text.find('\'') {
                        if let Some(end) = decorator_text[start+1..].find('\'') {
                            route_path = decorator_text[start+1..start+1+end].to_string();
                        }
                    }
                }
            }

            if child.kind() == "function_definition" {
                let fn_name = child.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(src).ok())
                    .unwrap_or("")
                    .to_string();
                let full_name = match parent_class {
                    Some(class) => format!("{}.{}", class, fn_name),
                    None => fn_name.clone(),
                };
                let param_count = child.child_by_field_name("parameters")
                    .map(|p| p.child_count().saturating_sub(2) as u32)
                    .unwrap_or(0);

                if is_route {
                    let route_name = format!("{} {}", http_method, route_path);
                    let chronos_node = self.make_node(child, src, path, &route_name, NodeKind::ApiRoute, param_count);
                    module.api_routes.push(chronos_node);
                } else {
                    let is_test = fn_name.starts_with("test_");
                    let kind = if is_test { NodeKind::TestCase } else { NodeKind::Function };
                    let chronos_node = self.make_node(child, src, path, &full_name, kind, param_count);
                    if is_test { module.test_cases.push(chronos_node); }
                    else { module.functions.push(chronos_node); }
                }

                // Keep call refs / nested defs inside the decorated function body
                if let Some(body) = child.child_by_field_name("body") {
                    let mut c = body.walk();
                    self.visit_node(body, src, path, source, module, &mut c, parent_class);
                }
            } else if child.kind() == "class_definition" {
                // Decorated class (e.g. @dataclass) — visit normally so the class
                // and its methods are extracted
                let mut c = child.walk();
                self.visit_node(child, src, path, source, module, &mut c, parent_class);
            }
        }
    }

    fn extract_import(
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

        let (source_module, imported_names) = if text.starts_with("from ") {
            let parts: Vec<&str> = text.splitn(3, ' ').collect();
            let module_name = parts.get(1).copied().unwrap_or("").to_string();
            let names_part = parts.get(2).and_then(|p| p.strip_prefix("import ")).unwrap_or("");
            let names: Vec<String> = names_part
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (module_name, names)
        } else {
            let module_name = text.strip_prefix("import ").unwrap_or("").trim().to_string();
            (module_name.clone(), vec![module_name])
        };

        module.import_refs.push(ImportRef {
            from_file: path.to_path_buf(),
            imported_names,
            source_module,
        });
    }

    fn extract_call(
        &self,
        node: tree_sitter::Node,
        src: &[u8],
        module: &mut ParsedModule,
    ) {
        let func_node = match node.child_by_field_name("function") {
            Some(n) => n,
            None => return,
        };

        let callee = match func_node.utf8_text(src) {
            Ok(t) => t.split('.').last().unwrap_or(t).to_string(),
            Err(_) => return,
        };

        if callee.is_empty() || callee.len() > 64 {
            return;
        }

        module.call_refs.push(CallRef {
            caller_name: String::new(),
            callee_name: callee,
            call_site_line: node.start_position().row + 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PY: &str = r#"
from flask import Flask, request
import json

app = Flask(__name__)

class UserService:
    def get_user(self, user_id):
        return {"id": user_id}

    def test_get_user(self):
        result = self.get_user(1)
        assert result["id"] == 1

@app.route("/users")
def list_users():
    return json.dumps([])

def test_create_user():
    pass
"#;

    #[test]
    fn test_python_parser_functions() {
        let mut parser = PythonParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_PY, Path::new("test.py"))
            .expect("parse");

        let fn_names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(!fn_names.is_empty(), "should find functions, got: {:?}", fn_names);
    }

    #[test]
    fn test_python_parser_tests() {
        let mut parser = PythonParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_PY, Path::new("test.py"))
            .expect("parse");

        assert!(!module.test_cases.is_empty(), "should detect test functions");
    }

    #[test]
    fn test_python_parser_imports() {
        let mut parser = PythonParser::new().expect("parser init");
        let module = parser
            .parse_str(SAMPLE_PY, Path::new("test.py"))
            .expect("parse");

        assert!(!module.import_refs.is_empty(), "should detect imports");
        assert!(
            module.import_refs.iter().any(|i| i.source_module == "flask"),
            "should detect flask import"
        );
    }
}
