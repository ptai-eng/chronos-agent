//! # Polyglot AST Parsers (Go, C, C++, Java) & Universal Complexity Analyzer
//!
//! Extends CHRONOS's AST engine to 7+ languages (`TS/JS`, `Python`, `Rust`, `Go`, `C`, `C++`, `Java`)
//! and computes deep AST complexity and hidden O(N^2) signals (`loop_depth`, `linear_scan_in_loop`,
//! `alloc_in_loop`, `is_recursive`) in a single Tree-sitter pass.

use crate::ast::ts_parser::{CallRef, ImportRef, ParsedModule};
use crate::graph::schema::{ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics};
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Node;

/// Universal AST complexity & hidden-bottleneck walker.
///
/// Walks any function/method AST subtree and populates:
/// - `cyclomatic_complexity`
/// - `loop_count` & `loop_depth` (max nested loops — polynomial degree proxy $O(N^k)$)
/// - `linear_scan_in_loop` (`.find`, `.contains`, `.indexOf`, `.includes`, `std::find`, `slices.Contains` inside a loop)
/// - `alloc_in_loop` (`.push`, `.append`, `new`, `vec!`, `make`, `malloc` inside a loop)
/// - `is_recursive` (direct self-call inside the body)
pub fn populate_ast_complexity_metrics(
    node: Node,
    source: &[u8],
    fn_name: &str,
    heuristics: &mut StaticHeuristics,
) {
    let mut branch_count = 1u32;
    let mut loop_count = 0u32;
    let mut max_loop_depth = 0u32;
    let mut linear_scan_in_loop = 0u32;
    let mut alloc_in_loop = 0u32;
    let mut is_recursive = false;

    let short_fn_name = fn_name.rsplit(|c| c == '.' || c == ':').next().unwrap_or(fn_name);

    walk_complexity(
        node,
        source,
        short_fn_name,
        0,
        &mut branch_count,
        &mut loop_count,
        &mut max_loop_depth,
        &mut linear_scan_in_loop,
        &mut alloc_in_loop,
        &mut is_recursive,
    );

    heuristics.cyclomatic_complexity = heuristics.cyclomatic_complexity.max(branch_count);
    heuristics.loop_count = loop_count;
    heuristics.loop_depth = max_loop_depth;
    heuristics.transitive_loop_depth = heuristics.transitive_loop_depth.max(max_loop_depth);
    heuristics.linear_scan_in_loop = linear_scan_in_loop;
    heuristics.alloc_in_loop = alloc_in_loop;
    if is_recursive {
        heuristics.is_recursive = true;
    }
    heuristics.compute_cognitive_load();
}

#[allow(clippy::too_many_arguments)]
fn walk_complexity(
    node: Node,
    source: &[u8],
    short_fn_name: &str,
    current_loop_depth: u32,
    branch_count: &mut u32,
    loop_count: &mut u32,
    max_loop_depth: &mut u32,
    linear_scan_in_loop: &mut u32,
    alloc_in_loop: &mut u32,
    is_recursive: &mut bool,
) {
    let kind = node.kind();

    let is_loop = matches!(
        kind,
        "for_statement"
            | "for_in_statement"
            | "for_range_loop"
            | "enhanced_for_statement"
            | "for_expression"
            | "while_statement"
            | "while_expression"
            | "do_statement"
            | "loop_expression"
    );

    let next_loop_depth = if is_loop {
        *loop_count += 1;
        *branch_count += 1;
        let d = current_loop_depth + 1;
        if d > *max_loop_depth {
            *max_loop_depth = d;
        }
        d
    } else {
        current_loop_depth
    };

    if !is_loop
        && matches!(
            kind,
            "if_statement"
                | "if_expression"
                | "elif_clause"
                | "switch_case"
                | "case_statement"
                | "match_arm"
                | "conditional_expression"
                | "ternary_expression"
                | "catch_clause"
                | "except_clause"
                | "&&"
                | "||"
        )
    {
        *branch_count += 1;
    }

    // Detect Python `x in collection` inside a loop (comparison_operator with `in`)
    if next_loop_depth > 0 && kind == "comparison_operator" {
        let text = node_text(node, source);
        if text.contains(" in ") {
            *linear_scan_in_loop += 1;
        }
    }

    // Detect heap allocations via `new Foo()` inside a loop
    if next_loop_depth > 0 && matches!(kind, "new_expression" | "object_creation_expression") {
        *alloc_in_loop += 1;
    }

    // Inspect call expressions / method invocations / macros
    if matches!(
        kind,
        "call_expression" | "call" | "method_invocation" | "macro_invocation"
    ) {
        let call_text = node_text(node, source);
        let head = call_text.split('(').next().unwrap_or(&call_text).trim();
        let callee_short = head
            .rsplit(|c: char| c == '.' || c == ':')
            .next()
            .unwrap_or(head)
            .trim_end_matches('!');

        if !short_fn_name.is_empty() && callee_short == short_fn_name {
            *is_recursive = true;
        }

        if next_loop_depth > 0 {
            if matches!(
                callee_short,
                "find"
                    | "findIndex"
                    | "includes"
                    | "indexOf"
                    | "lastIndexOf"
                    | "contains"
                    | "Contains"
                    | "Index"
                    | "linear_search"
            ) {
                *linear_scan_in_loop += 1;
            }
            if matches!(
                callee_short,
                "push"
                    | "append"
                    | "extend"
                    | "insert"
                    | "add"
                    | "put"
                    | "vec"
                    | "make"
                    | "malloc"
                    | "calloc"
                    | "realloc"
                    | "clone"
            ) {
                *alloc_in_loop += 1;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_complexity(
            child,
            source,
            short_fn_name,
            next_loop_depth,
            branch_count,
            loop_count,
            max_loop_depth,
            linear_scan_in_loop,
            alloc_in_loop,
            is_recursive,
        );
    }
}

// ── Go Parser (.go) ──────────────────────────────────────────────────────────

pub struct GoParser {
    parser: tree_sitter::Parser,
}

impl GoParser {
    pub fn new() -> Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_go::language())
            .context("Failed to set Go language for tree-sitter")?;
        Ok(Self { parser })
    }

    pub fn parse_file(&mut self, file_path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read Go file: {}", file_path.display()))?;
        self.parse_source(file_path, &source)
    }

    pub fn parse_source(&mut self, file_path: &Path, source: &str) -> Result<ParsedModule> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Tree-sitter failed to parse Go file: {}", file_path.display()))?;

        let mut module = ParsedModule::new(file_path.to_path_buf());
        module.sloc = count_sloc(source);
        let bytes = source.as_bytes();
        let file_str = file_path.to_string_lossy().to_string();

        self.walk_go(tree.root_node(), bytes, &mut module, file_path, &file_str, None);
        Ok(module)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_go(
        &self,
        node: Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        file_str: &str,
        current_fn: Option<&str>,
    ) {
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let raw_name = node_text(name_node, source);
                    if !raw_name.is_empty() {
                        let is_test = raw_name.starts_with("Test")
                            || raw_name.starts_with("Benchmark")
                            || raw_name.starts_with("Example")
                            || raw_name.starts_with("Fuzz");
                        let kind = if is_test { NodeKind::TestCase } else { NodeKind::Function };
                        let id = NodeId::new(file_str, &raw_name, &kind);
                        let loc = make_loc(node, file_path);
                        let mut cnode = ChronosNode::new(
                            id,
                            kind,
                            &raw_name,
                            format!("{file_str}::{raw_name}"),
                            loc,
                        );
                        cnode.heuristics.sloc =
                            (node.end_position().row.saturating_sub(node.start_position().row) + 1) as u32;
                        populate_ast_complexity_metrics(node, source, &raw_name, &mut cnode.heuristics);
                        cnode.contract_signature = Some(extract_signature_line(node, source));

                        if is_test {
                            module.test_cases.push(cnode);
                        } else {
                            module.functions.push(cnode);
                        }

                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            self.walk_go(child, source, module, file_path, file_str, Some(&raw_name));
                        }
                        return;
                    }
                }
            }
            "type_spec" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    if !name.is_empty() {
                        let id = NodeId::new(file_str, &name, &NodeKind::Class);
                        let cnode = ChronosNode::new(
                            id,
                            NodeKind::Class,
                            &name,
                            format!("{file_str}::{name}"),
                            make_loc(node, file_path),
                        );
                        module.classes.push(cnode);
                    }
                }
            }
            "import_spec" => {
                if let Some(path_node) = node.child_by_field_name("path") {
                    let pkg = node_text(path_node, source).trim_matches('"').to_string();
                    let short = pkg.rsplit('/').next().unwrap_or(&pkg).to_string();
                    module.import_refs.push(ImportRef {
                        from_file: file_path.to_path_buf(),
                        imported_names: vec![short],
                        source_module: pkg,
                    });
                }
            }
            "call_expression" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let raw = node_text(fn_node, source);
                    let callee = raw.rsplit('.').next().unwrap_or(&raw).trim().to_string();
                    if !callee.is_empty() {
                        module.call_refs.push(CallRef {
                            caller_name: current_fn.unwrap_or("<module>").to_string(),
                            callee_name: callee,
                            call_site_line: node.start_position().row + 1,
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_go(child, source, module, file_path, file_str, current_fn);
        }
    }
}

// ── C / C++ Parser (.c, .h, .cc, .cpp, .cxx, .hpp) ───────────────────────────

pub struct CppParser {
    c_parser: tree_sitter::Parser,
    cpp_parser: tree_sitter::Parser,
}

impl CppParser {
    pub fn new() -> Result<Self> {
        let mut c_parser = tree_sitter::Parser::new();
        c_parser
            .set_language(&tree_sitter_c::language())
            .context("Failed to set C language for tree-sitter")?;

        let mut cpp_parser = tree_sitter::Parser::new();
        cpp_parser
            .set_language(&tree_sitter_cpp::language())
            .context("Failed to set C++ language for tree-sitter")?;

        Ok(Self { c_parser, cpp_parser })
    }

    pub fn parse_file(&mut self, file_path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read C/C++ file: {}", file_path.display()))?;
        self.parse_source(file_path, &source)
    }

    pub fn parse_source(&mut self, file_path: &Path, source: &str) -> Result<ParsedModule> {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("cpp");
        let tree = if ext == "c" {
            self.c_parser.parse(source, None)
        } else {
            self.cpp_parser.parse(source, None)
        }
        .ok_or_else(|| anyhow::anyhow!("Tree-sitter failed to parse C/C++ file: {}", file_path.display()))?;

        let mut module = ParsedModule::new(file_path.to_path_buf());
        module.sloc = count_sloc(source);
        let bytes = source.as_bytes();
        let file_str = file_path.to_string_lossy().to_string();

        self.walk_cpp(tree.root_node(), bytes, &mut module, file_path, &file_str, None);
        Ok(module)
    }

    fn walk_cpp(
        &self,
        node: Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        file_str: &str,
        current_fn: Option<&str>,
    ) {
        match node.kind() {
            "function_definition" => {
                if let Some(decl) = node.child_by_field_name("declarator") {
                    let raw_name = extract_cpp_declarator_name(decl, source);
                    if !raw_name.is_empty() {
                        let is_gtest = raw_name == "TEST" || raw_name == "TEST_F" || raw_name.starts_with("test_");
                        let fn_name = if raw_name == "TEST" || raw_name == "TEST_F" {
                            // Extract GoogleTest TEST(Suite, Case) name from parameter list
                            extract_gtest_name(decl, source).unwrap_or(raw_name)
                        } else {
                            raw_name
                        };
                        let short_name = fn_name.rsplit("::").next().unwrap_or(&fn_name).to_string();
                        let kind = if is_gtest { NodeKind::TestCase } else { NodeKind::Function };
                        let id = NodeId::new(file_str, &short_name, &kind);
                        let mut cnode = ChronosNode::new(
                            id,
                            kind,
                            &short_name,
                            format!("{file_str}::{fn_name}"),
                            make_loc(node, file_path),
                        );
                        cnode.heuristics.sloc =
                            (node.end_position().row.saturating_sub(node.start_position().row) + 1) as u32;
                        populate_ast_complexity_metrics(node, source, &short_name, &mut cnode.heuristics);
                        cnode.contract_signature = Some(extract_signature_line(node, source));

                        if is_gtest {
                            module.test_cases.push(cnode);
                        } else {
                            module.functions.push(cnode);
                        }

                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            self.walk_cpp(child, source, module, file_path, file_str, Some(&short_name));
                        }
                        return;
                    }
                }
            }
            "class_specifier" | "struct_specifier" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    if !name.is_empty() {
                        let id = NodeId::new(file_str, &name, &NodeKind::Class);
                        module.classes.push(ChronosNode::new(
                            id,
                            NodeKind::Class,
                            &name,
                            format!("{file_str}::{name}"),
                            make_loc(node, file_path),
                        ));
                    }
                }
            }
            "preproc_include" => {
                if let Some(path_node) = node.child_by_field_name("path") {
                    let inc = node_text(path_node, source)
                        .trim_matches(|c| c == '"' || c == '<' || c == '>')
                        .to_string();
                    module.import_refs.push(ImportRef {
                        from_file: file_path.to_path_buf(),
                        imported_names: vec![inc.clone()],
                        source_module: inc,
                    });
                }
            }
            "call_expression" => {
                if let Some(fn_node) = node.child_by_field_name("function") {
                    let raw = node_text(fn_node, source);
                    let callee = raw
                        .rsplit(|c| c == '.' || c == ':' || c == '>')
                        .next()
                        .unwrap_or(&raw)
                        .trim()
                        .to_string();
                    if !callee.is_empty() {
                        module.call_refs.push(CallRef {
                            caller_name: current_fn.unwrap_or("<module>").to_string(),
                            callee_name: callee,
                            call_site_line: node.start_position().row + 1,
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_cpp(child, source, module, file_path, file_str, current_fn);
        }
    }
}

fn extract_cpp_declarator_name(node: Node, source: &[u8]) -> String {
    match node.kind() {
        "identifier" | "field_identifier" | "qualified_identifier" | "destructor_name" => {
            node_text(node, source)
        }
        _ => {
            if let Some(inner) = node.child_by_field_name("declarator") {
                return extract_cpp_declarator_name(inner, source);
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let s = extract_cpp_declarator_name(child, source);
                if !s.is_empty() {
                    return s;
                }
            }
            String::new()
        }
    }
}

fn extract_gtest_name(decl: Node, source: &[u8]) -> Option<String> {
    let text = node_text(decl, source);
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let args = text[open + 1..close]
        .split(',')
        .map(|s| s.trim())
        .collect::<Vec<_>>()
        .join("_");
    if args.is_empty() {
        None
    } else {
        Some(format!("TEST_{args}"))
    }
}

// ── Java Parser (.java) ──────────────────────────────────────────────────────

pub struct JavaParser {
    parser: tree_sitter::Parser,
}

impl JavaParser {
    pub fn new() -> Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_java::language())
            .context("Failed to set Java language for tree-sitter")?;
        Ok(Self { parser })
    }

    pub fn parse_file(&mut self, file_path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read Java file: {}", file_path.display()))?;
        self.parse_source(file_path, &source)
    }

    pub fn parse_source(&mut self, file_path: &Path, source: &str) -> Result<ParsedModule> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Tree-sitter failed to parse Java file: {}", file_path.display()))?;

        let mut module = ParsedModule::new(file_path.to_path_buf());
        module.sloc = count_sloc(source);
        let bytes = source.as_bytes();
        let file_str = file_path.to_string_lossy().to_string();

        self.walk_java(tree.root_node(), bytes, &mut module, file_path, &file_str, None);
        Ok(module)
    }

    fn walk_java(
        &self,
        node: Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        file_str: &str,
        current_fn: Option<&str>,
    ) {
        match node.kind() {
            "class_declaration" | "interface_declaration" | "record_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    if !name.is_empty() {
                        let id = NodeId::new(file_str, &name, &NodeKind::Class);
                        module.classes.push(ChronosNode::new(
                            id,
                            NodeKind::Class,
                            &name,
                            format!("{file_str}::{name}"),
                            make_loc(node, file_path),
                        ));
                    }
                }
            }
            "method_declaration" | "constructor_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    if !name.is_empty() {
                        let full_text = node_text(node, source);
                        let is_test = full_text.contains("@Test")
                            || full_text.contains("@ParameterizedTest")
                            || (file_str.ends_with("Test.java") && name.starts_with("test"));
                        let is_route = full_text.contains("@GetMapping")
                            || full_text.contains("@PostMapping")
                            || full_text.contains("@PutMapping")
                            || full_text.contains("@DeleteMapping")
                            || full_text.contains("@RequestMapping");

                        let kind = if is_test {
                            NodeKind::TestCase
                        } else {
                            NodeKind::Function
                        };
                        let id = NodeId::new(file_str, &name, &kind);
                        let mut cnode = ChronosNode::new(
                            id,
                            kind.clone(),
                            &name,
                            format!("{file_str}::{name}"),
                            make_loc(node, file_path),
                        );
                        cnode.heuristics.sloc =
                            (node.end_position().row.saturating_sub(node.start_position().row) + 1) as u32;
                        populate_ast_complexity_metrics(node, source, &name, &mut cnode.heuristics);
                        cnode.contract_signature = Some(extract_signature_line(node, source));

                        if is_route {
                            let route_id = NodeId::new(file_str, &name, &NodeKind::ApiRoute);
                            module.api_routes.push(ChronosNode::new(
                                route_id,
                                NodeKind::ApiRoute,
                                format!("ROUTE {name}"),
                                format!("{file_str}::ROUTE::{name}"),
                                make_loc(node, file_path),
                            ));
                        }

                        if is_test {
                            module.test_cases.push(cnode);
                        } else {
                            module.functions.push(cnode);
                        }

                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            self.walk_java(child, source, module, file_path, file_str, Some(&name));
                        }
                        return;
                    }
                }
            }
            "import_declaration" => {
                let raw = node_text(node, source)
                    .trim_start_matches("import")
                    .trim_start_matches("static")
                    .trim()
                    .trim_end_matches(';')
                    .to_string();
                if !raw.is_empty() {
                    let short = raw.rsplit('.').next().unwrap_or(&raw).to_string();
                    module.import_refs.push(ImportRef {
                        from_file: file_path.to_path_buf(),
                        imported_names: vec![short],
                        source_module: raw,
                    });
                }
            }
            "method_invocation" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let callee = node_text(name_node, source);
                    if !callee.is_empty() {
                        module.call_refs.push(CallRef {
                            caller_name: current_fn.unwrap_or("<module>").to_string(),
                            callee_name: callee,
                            call_site_line: node.start_position().row + 1,
                        });
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_java(child, source, module, file_path, file_str, current_fn);
        }
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn node_text(node: Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").trim().to_string()
}

fn make_loc(node: Node, file_path: &Path) -> SourceLocation {
    SourceLocation {
        file: file_path.to_path_buf(),
        start_line: node.start_position().row + 1,
        start_col: node.start_position().column,
        end_line: node.end_position().row + 1,
        end_col: node.end_position().column,
    }
}

fn extract_signature_line(node: Node, source: &[u8]) -> String {
    node_text(node, source)
        .lines()
        .next()
        .unwrap_or("")
        .trim_end_matches('{')
        .trim()
        .to_string()
}

fn count_sloc(source: &str) -> usize {
    source
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*'))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_go_parser_functions_and_loops() {
        let mut p = GoParser::new().unwrap();
        let src = r#"
package orders

import "slices"

type Order struct {
    ID string
}

func ProcessOrders(orders []string, blacklist []string) int {
    count := 0
    for _, o := range orders {
        for _, b := range blacklist {
            if slices.Contains(blacklist, o) && o == b {
                count++
            }
        }
    }
    return count
}

func TestProcessOrders(t *testing.T) {
    ProcessOrders([]string{"a"}, []string{"b"})
}
"#;
        let m = p.parse_source(Path::new("orders_test.go"), src).unwrap();
        assert_eq!(m.functions.len(), 1);
        assert_eq!(m.test_cases.len(), 1);
        assert_eq!(m.classes.len(), 1);
        let f = &m.functions[0];
        assert_eq!(f.name, "ProcessOrders");
        assert_eq!(f.heuristics.loop_depth, 2);
        assert!(f.heuristics.linear_scan_in_loop >= 1);
        assert!(m.call_refs.iter().any(|c| c.caller_name == "TestProcessOrders" && c.callee_name == "ProcessOrders"));
    }

    #[test]
    fn test_cpp_parser_and_gtest() {
        let mut p = CppParser::new().unwrap();
        let src = r#"
#include <vector>

class Engine {
public:
    int rank;
};

int computeScore(int n) {
    if (n <= 1) return 1;
    return n * computeScore(n - 1);
}

TEST(EngineSuite, ComputesFactorial) {
    int res = computeScore(5);
}
"#;
        let m = p.parse_source(Path::new("engine_test.cpp"), src).unwrap();
        assert_eq!(m.classes.len(), 1);
        assert_eq!(m.functions.len(), 1);
        assert!(m.functions[0].heuristics.is_recursive);
        assert_eq!(m.test_cases.len(), 1);
        assert!(m.call_refs.iter().any(|c| c.callee_name == "computeScore"));
    }

    #[test]
    fn test_java_parser_and_junit() {
        let mut p = JavaParser::new().unwrap();
        let src = r#"
package com.chronos.billing;
import java.util.List;

public class BillingService {
    @PostMapping("/charge")
    public boolean chargeUsers(List<String> users, List<String> blocked) {
        for (String u : users) {
            if (blocked.contains(u)) {
                return false;
            }
        }
        return true;
    }

    @Test
    public void testChargeUsers() {
        chargeUsers(List.of("u1"), List.of());
    }
}
"#;
        let m = p.parse_source(Path::new("BillingServiceTest.java"), src).unwrap();
        assert_eq!(m.classes.len(), 1);
        assert_eq!(m.functions.len(), 1);
        assert_eq!(m.api_routes.len(), 1);
        assert_eq!(m.test_cases.len(), 1);
        let f = &m.functions[0];
        assert_eq!(f.heuristics.loop_depth, 1);
        assert_eq!(f.heuristics.linear_scan_in_loop, 1);
    }
}
