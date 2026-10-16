//! # TypeScript AST Parser
//!
//! Uses tree-sitter-typescript to parse TypeScript/JavaScript source files
//! and extract symbols into a form ready for graph ingestion.
//!
//! ## Design decisions
//! - Uses tree-sitter S-expression queries for robust pattern matching
//! - Extracts both declaration sites AND call sites in one pass
//! - Handles arrow functions, regular functions, and class methods uniformly
//! - Express.js route patterns detected via call chain analysis

use crate::graph::schema::{
    ChronosNode, NodeId, NodeKind, SourceLocation, StaticHeuristics,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::debug;

/// A call reference discovered during AST parsing
#[derive(Debug, Clone)]
pub struct CallRef {
    pub caller_name: String,
    pub callee_name: String,
    pub call_site_line: usize,
}

/// An import reference discovered during AST parsing
#[derive(Debug, Clone)]
pub struct ImportRef {
    pub from_file: PathBuf,
    pub imported_names: Vec<String>,
    pub source_module: String,
}

/// All symbols and relationships extracted from a single source file
#[derive(Debug)]
pub struct ParsedModule {
    pub file_path: PathBuf,
    pub functions: Vec<ChronosNode>,
    pub classes: Vec<ChronosNode>,
    pub api_routes: Vec<ChronosNode>,
    pub test_cases: Vec<ChronosNode>,
    pub call_refs: Vec<CallRef>,
    pub import_refs: Vec<ImportRef>,
    /// Number of dynamic/computed call sites whose target could not be statically named
    pub unresolved_call_sites: usize,
    /// Total non-blank, non-comment lines (for complexity scoring)
    pub sloc: usize,
}

impl ParsedModule {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            functions: vec![],
            classes: vec![],
            api_routes: vec![],
            test_cases: vec![],
            call_refs: vec![],
            import_refs: vec![],
            unresolved_call_sites: 0,
            sloc: 0,
        }
    }

    /// All nodes from this module (for bulk graph insertion)
    pub fn all_nodes(&self) -> Vec<&ChronosNode> {
        let mut nodes: Vec<&ChronosNode> = vec![];
        nodes.extend(self.functions.iter());
        nodes.extend(self.classes.iter());
        nodes.extend(self.api_routes.iter());
        nodes.extend(self.test_cases.iter());
        nodes
    }
}

/// Tree-sitter based TypeScript/JavaScript parser.
///
/// Parse a file → extract all symbols → ready for graph ingestion.
pub struct TypeScriptParser {
    parser: tree_sitter::Parser,
}

impl TypeScriptParser {
    /// Create a new parser configured for TypeScript.
    pub fn new() -> Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_typescript::language_typescript())
            .context("Failed to set TypeScript language for tree-sitter")?;
        Ok(Self { parser })
    }

    /// Parse a TypeScript/JavaScript file and extract all symbols.
    ///
    /// Returns a `ParsedModule` ready to be inserted into the 4D graph.
    pub fn parse_file(&mut self, file_path: &Path) -> Result<ParsedModule> {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
        self.parse_source(file_path, &source)
    }

    /// Parse from source string (useful for testing without filesystem).
    pub fn parse_source(&mut self, file_path: &Path, source: &str) -> Result<ParsedModule> {
        let tree = self.parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Tree-sitter failed to parse: {}", file_path.display()))?;

        let mut module = ParsedModule::new(file_path.to_path_buf());
        module.sloc = count_sloc(source);

        let root_node = tree.root_node();
        let source_bytes = source.as_bytes();

        // Walk the AST and extract symbols
        self.extract_symbols(root_node, source_bytes, &mut module, file_path, None, None);

        debug!(
            "Parsed {}: {} functions, {} classes, {} routes, {} tests, {} calls ({} unresolved)",
            file_path.display(),
            module.functions.len(),
            module.classes.len(),
            module.api_routes.len(),
            module.test_cases.len(),
            module.call_refs.len(),
            module.unresolved_call_sites,
        );

        Ok(module)
    }

    fn extract_symbols(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        current_class: Option<&str>,
        current_function: Option<&str>,
    ) {
        match node.kind() {
            // function foo() {} / async function foo() {}
            "function_declaration" => {
                let mut fn_scope = current_function.map(|s| s.to_string());
                if let Some(fn_node) = self.extract_function(node, source, file_path, current_class) {
                    fn_scope = Some(fn_node.name.clone());
                    module.functions.push(fn_node);
                }
                // Recurse with updated current_function scope
                self.recurse_children(node, source, module, file_path, current_class, fn_scope.as_deref());
            }

            // class Foo { ... }
            "class_declaration" => {
                let class_name = get_child_text(node, "name", source).unwrap_or_default();
                if let Some(class_node) = self.extract_class(node, source, file_path, &class_name) {
                    module.classes.push(class_node);
                }
                // Recurse into class body to extract methods
                self.recurse_children(node, source, module, file_path, Some(class_name.as_str()), current_function);
            }

            // const foo = () => {} or const foo = function() {}
            "lexical_declaration" | "variable_declaration" => {
                let declared_fn = self.extract_variable_declarations(node, source, module, file_path, current_class);
                let next_fn = declared_fn.as_deref().or(current_function);
                self.recurse_children(node, source, module, file_path, current_class, next_fn);
            }

            // class method definitions
            "method_definition" => {
                let mut fn_scope = current_function.map(|s| s.to_string());
                if let Some(fn_node) = self.extract_method(node, source, file_path, current_class) {
                    fn_scope = Some(fn_node.name.clone());
                    module.functions.push(fn_node);
                }
                self.recurse_children(node, source, module, file_path, current_class, fn_scope.as_deref());
            }

            // Call expressions: foo(), obj.method(), app.get('/path', handler)
            "call_expression" => {
                let test_scope = self.try_extract_test_scope(node, source);
                let active_fn = test_scope.as_deref().or(current_function);
                if test_scope.is_none() {
                    self.extract_call(node, source, module, file_path, active_fn);
                }
                self.recurse_children(node, source, module, file_path, current_class, active_fn);
            }

            // import { foo } from './bar'
            "import_statement" => {
                if let Some(import_ref) = self.extract_import(node, source, file_path) {
                    module.import_refs.push(import_ref);
                }
            }

            // describe('...', () => { it('...', () => { ... }) })
            "expression_statement" => {
                self.extract_test_blocks(node, source, module, file_path);
                self.recurse_children(node, source, module, file_path, current_class, current_function);
            }

            _ => {
                self.recurse_children(node, source, module, file_path, current_class, current_function);
            }
        }
    }

    fn recurse_children(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        current_class: Option<&str>,
        current_function: Option<&str>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols(child, source, module, file_path, current_class, current_function);
        }
    }

    fn extract_function(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        parent_class: Option<&str>,
    ) -> Option<ChronosNode> {
        let name = get_child_text(node, "name", source)?;
        let qualified_name = if let Some(class) = parent_class {
            format!("{}::{}", class, name)
        } else {
            let file_stem = file_path.file_stem()?.to_string_lossy().to_string();
            format!("{}::{}", file_stem, name)
        };

        let location = node_to_location(node, file_path);
        let id = NodeId::new(&file_path.to_string_lossy(), &name, &NodeKind::Function);

        let mut chronos_node = ChronosNode::new(id, NodeKind::Function, name, qualified_name, location);
        chronos_node.heuristics = compute_function_heuristics(node, source);

        Some(chronos_node)
    }

    fn extract_method(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
        parent_class: Option<&str>,
    ) -> Option<ChronosNode> {
        let name = get_child_text(node, "name", source)?;
        
        // Skip constructor for now, or handle specially
        let qualified_name = if let Some(class) = parent_class {
            format!("{}::{}", class, name)
        } else {
            name.clone()
        };

        let location = node_to_location(node, file_path);
        let id = NodeId::new(&file_path.to_string_lossy(), &qualified_name, &NodeKind::Function);

        let mut chronos_node = ChronosNode::new(id, NodeKind::Function, name, qualified_name, location);
        chronos_node.heuristics = compute_function_heuristics(node, source);
        Some(chronos_node)
    }

    fn extract_class(
        &self,
        node: tree_sitter::Node,
        _source: &[u8],
        file_path: &Path,
        name: &str,
    ) -> Option<ChronosNode> {
        let qualified_name = format!(
            "{}::{}",
            file_path.file_stem()?.to_string_lossy(),
            name
        );
        let location = node_to_location(node, file_path);
        let id = NodeId::new(&file_path.to_string_lossy(), name, &NodeKind::Class);
        Some(ChronosNode::new(id, NodeKind::Class, name, qualified_name, location))
    }

    fn extract_variable_declarations(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        current_class: Option<&str>,
    ) -> Option<String> {
        let mut declared_name = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                // Look for: const foo = () => {} or const foo = function() {}
                let name_node = child.child_by_field_name("name");
                let value_node = child.child_by_field_name("value");

                if let (Some(name_n), Some(val_n)) = (name_node, value_node) {
                    let name = node_text(name_n, source);
                    match val_n.kind() {
                        "arrow_function" | "function_expression" => {
                            let qualified_name = if let Some(class) = current_class {
                                format!("{}::{}", class, name)
                            } else {
                                let stem = file_path.file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                format!("{}::{}", stem, name)
                            };
                            let location = node_to_location(child, file_path);
                            let id = NodeId::new(&file_path.to_string_lossy(), &name, &NodeKind::Function);
                            let mut fn_node = ChronosNode::new(
                                id, NodeKind::Function, name.clone(), qualified_name, location,
                            );
                            fn_node.heuristics = compute_function_heuristics(val_n, source);
                            module.functions.push(fn_node);
                            declared_name = Some(name);
                        }
                        _ => {}
                    }
                }
            }
        }
        declared_name
    }

    fn try_extract_test_scope(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
    ) -> Option<String> {
        let fn_node = node.child_by_field_name("function")?;
        let fn_name = node_text(fn_node, source);
        if matches!(fn_name.as_str(), "it" | "test" | "describe") {
            let args_node = node.child_by_field_name("arguments")?;
            let mut arg_cursor = args_node.walk();
            for arg in args_node.children(&mut arg_cursor) {
                if matches!(arg.kind(), "string" | "template_string") {
                    let test_label = node_text(arg, source)
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim_matches('`')
                        .to_string();
                    return Some(format!("{}({})", fn_name, test_label));
                }
            }
        }
        None
    }

    fn extract_call(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
        current_function: Option<&str>,
    ) {
        // Detect Express.js routes: app.get(...), router.post(...), etc.
        if let Some(route_node) = try_extract_express_route(node, source, file_path) {
            module.api_routes.push(route_node);
            return;
        }

        // Extract callee name for call graph edges
        let function_node = node.child_by_field_name("function");
        if let Some(fn_node) = function_node {
            // Dynamic / computed call sites (subscript_expression, parenthesized_expression, etc.)
            if matches!(
                fn_node.kind(),
                "subscript_expression" | "parenthesized_expression" | "call_expression"
            ) {
                module.unresolved_call_sites += 1;
                return;
            }

            let raw_callee = node_text(fn_node, source);
            let callee_name = raw_callee
                .rsplit('.')
                .next()
                .unwrap_or(&raw_callee)
                .trim()
                .to_string();

            if matches!(callee_name.as_str(), "apply" | "call" | "bind" | "eval") {
                module.unresolved_call_sites += 1;
            }

            // Only record meaningful function calls (not built-ins, not too long)
            if !callee_name.is_empty() && callee_name.len() < 100 && !callee_name.contains('\n') {
                if let Some(caller) = current_function {
                    module.call_refs.push(CallRef {
                        caller_name: caller.to_string(),
                        callee_name,
                        call_site_line: node.start_position().row + 1,
                    });
                }
            }
        }
    }

    fn extract_import(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        file_path: &Path,
    ) -> Option<ImportRef> {
        // import { foo, bar } from './module'
        let _source_text = node_text(node, source);
        
        // Find the from source (string literal at end)
        let mut cursor = node.walk();
        let mut source_module = String::new();
        let mut imported_names = vec![];

        for child in node.children(&mut cursor) {
            match child.kind() {
                "string" => {
                    source_module = node_text(child, source)
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                }
                "import_clause" => {
                    // Extract named imports
                    let _clause_text = node_text(child, source);
                    // Simple extraction: get identifiers from { foo, bar as baz }
                    let mut child_cursor = child.walk();
                    for import_child in child.children(&mut child_cursor) {
                        if import_child.kind() == "named_imports" {
                            let mut named_cursor = import_child.walk();
                            for named in import_child.children(&mut named_cursor) {
                                if named.kind() == "import_specifier" {
                                    let name = get_child_text(named, "name", source)
                                        .unwrap_or_else(|| node_text(named, source));
                                    imported_names.push(name);
                                }
                            }
                        } else if import_child.kind() == "identifier" {
                            // default import
                            imported_names.push(node_text(import_child, source));
                        }
                    }
                }
                _ => {}
            }
        }

        if source_module.is_empty() { return None; }

        Some(ImportRef {
            from_file: file_path.to_path_buf(),
            imported_names,
            source_module,
        })
    }

    fn extract_test_blocks(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        module: &mut ParsedModule,
        file_path: &Path,
    ) {
        // Detect: describe('...', ...) / it('...', ...) / test('...', ...)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "call_expression" {
                let fn_node = child.child_by_field_name("function");
                if let Some(fn_node) = fn_node {
                    let fn_name = node_text(fn_node, source);
                    if matches!(fn_name.as_str(), "it" | "test" | "describe") {
                        // Get the test name from first argument (string literal)
                        let args = child.child_by_field_name("arguments");
                        if let Some(args_node) = args {
                            let mut arg_cursor = args_node.walk();
                            for arg in args_node.children(&mut arg_cursor) {
                                if matches!(arg.kind(), "string" | "template_string") {
                                    let test_label = node_text(arg, source)
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .trim_matches('`')
                                        .to_string();
                                    let display_name = format!("{}({})", fn_name, test_label);
                                    let location = node_to_location(child, file_path);
                                    let id = NodeId::new(
                                        &file_path.to_string_lossy(),
                                        &display_name,
                                        &NodeKind::TestCase,
                                    );
                                    module.test_cases.push(ChronosNode::new(
                                        id,
                                        NodeKind::TestCase,
                                        display_name.clone(),
                                        display_name,
                                        location,
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Detect Express.js route registration patterns:
/// `app.get('/path', handler)`, `router.post('/path', handler)`, etc.
fn try_extract_express_route(
    node: tree_sitter::Node,
    source: &[u8],
    file_path: &Path,
) -> Option<ChronosNode> {
    let fn_node = node.child_by_field_name("function")?;

    if fn_node.kind() != "member_expression" { return None; }

    let method_node = fn_node.child_by_field_name("property")?;
    let method_name = node_text(method_node, source).to_uppercase();

    if !matches!(method_name.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "USE") {
        return None;
    }

    // Extract the route path from first argument
    let args_node = node.child_by_field_name("arguments")?;
    let mut cursor = args_node.walk();
    for arg in args_node.children(&mut cursor) {
        if matches!(arg.kind(), "string" | "template_string") {
            let path = node_text(arg, source)
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            let route_name = format!("{} {}", method_name, path);
            let location = node_to_location(node, file_path);
            let id = NodeId::new(
                &file_path.to_string_lossy(),
                &route_name,
                &NodeKind::ApiRoute,
            );
            let mut route_node = ChronosNode::new(
                id, NodeKind::ApiRoute, route_name.clone(), route_name, location,
            );
            route_node.contract_signature = Some(format!("{}:{}", method_name, path));
            return Some(route_node);
        }
    }
    None
}

/// Compute static complexity heuristics from a function/method AST node.
fn compute_function_heuristics(node: tree_sitter::Node, source: &[u8]) -> StaticHeuristics {
    let mut h = StaticHeuristics::default();
    let fn_text = node_text(node, source);

    // Cyclomatic complexity: count branching keywords
    h.cyclomatic_complexity = 1; // base case
    let keywords = ["if ", "else if", "for ", "while ", "switch ", " ? ", "&&", "||", "?."];
    for kw in &keywords {
        h.cyclomatic_complexity += fn_text.matches(kw).count() as u32;
    }

    // SLOC approximation
    h.sloc = fn_text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//") && !l.trim().starts_with("*"))
        .count() as u32;

    // Count parameters (rough heuristic: count commas in param list + 1)
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let params_text = node_text(params_node, source);
        if params_text.len() > 2 {
            h.param_count = (params_text.matches(',').count() + 1) as u32;
        }
    }

    h.compute_cognitive_load();
    h
}

/// Count source lines of code (excluding blank lines and comment-only lines)
fn count_sloc(source: &str) -> usize {
    source.lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("*") && !trimmed.starts_with("/*")
        })
        .count()
}

/// Get the text content of a named child field from a tree-sitter node.
fn get_child_text(node: tree_sitter::Node, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

/// Get the text content of a tree-sitter node from the source bytes.
fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte().min(source.len());
    String::from_utf8_lossy(&source[start..end]).to_string()
}

/// Convert a tree-sitter Node position to a CHRONOS SourceLocation.
fn node_to_location(node: tree_sitter::Node, file_path: &Path) -> SourceLocation {
    let start = node.start_position();
    let end = node.end_position();
    SourceLocation {
        file: file_path.to_path_buf(),
        start_line: start.row + 1,  // 1-indexed
        start_col: start.column,
        end_line: end.row + 1,
        end_col: end.column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SAMPLE_TS: &str = r#"
import { UserRepository } from './repository';
import { hashPassword } from './utils';

class UserService {
    private repo: UserRepository;

    async getUser(id: string): Promise<User> {
        return this.repo.findById(id);
    }

    async createUser(email: string, password: string): Promise<User> {
        const hashed = await hashPassword(password);
        return this.repo.create({ email, password: hashed });
    }
}

const processOrders = async (orders: Order[]) => {
    // N+1 query bug: for loop calling DB each iteration
    for (const order of orders) {
        const user = await getUser(order.userId);  // bug: should batch
        if (user.isActive) {
            await sendEmail(user.email);
        }
    }
};

export { UserService, processOrders };
"#;

    const SAMPLE_EXPRESS: &str = r#"
import express from 'express';
const router = express.Router();

router.get('/users/:id', async (req, res) => {
    const user = await userService.getUser(req.params.id);
    res.json(user);
});

router.post('/users', async (req, res) => {
    const user = await userService.createUser(req.body.email, req.body.password);
    res.status(201).json(user);
});
"#;

    const SAMPLE_TEST: &str = r#"
import { UserService } from '../src/services/user';

describe('UserService', () => {
    it('should return a user by id', async () => {
        const service = new UserService();
        const user = await service.getUser('123');
        expect(user).toBeDefined();
    });

    test('should create a new user', async () => {
        const service = new UserService();
        const user = await service.createUser('test@example.com', 'pass');
        expect(user.email).toBe('test@example.com');
    });
});
"#;

    #[test]
    fn test_parse_class_and_methods() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let path = Path::new("src/services/user.ts");
        let module = parser.parse_source(path, SAMPLE_TS).expect("Parse failed");

        // Should find UserService class
        assert!(!module.classes.is_empty(), "Should find at least one class");
        let class = &module.classes[0];
        assert_eq!(class.name, "UserService");

        // Should find functions: getUser, createUser, processOrders
        let fn_names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
        println!("Found functions: {:?}", fn_names);
        assert!(fn_names.contains(&"getUser") || fn_names.contains(&"processOrders"),
            "Should find key function names, got: {:?}", fn_names);
    }

    #[test]
    fn test_parse_express_routes() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let path = Path::new("src/routes/users.ts");
        let module = parser.parse_source(path, SAMPLE_EXPRESS).expect("Parse failed");

        println!("API routes found: {:?}", module.api_routes.iter().map(|r| &r.name).collect::<Vec<_>>());
        // Should detect Express routes
        assert!(!module.api_routes.is_empty(), "Should detect Express routes");
    }

    #[test]
    fn test_parse_test_cases() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let path = Path::new("tests/user.test.ts");
        let module = parser.parse_source(path, SAMPLE_TEST).expect("Parse failed");

        let test_names: Vec<&str> = module.test_cases.iter().map(|t| t.name.as_str()).collect();
        println!("Test cases found: {:?}", test_names);
        assert!(!module.test_cases.is_empty(), "Should find test cases (it/test/describe blocks)");
    }

    #[test]
    fn test_parse_imports() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let path = Path::new("src/services/user.ts");
        let module = parser.parse_source(path, SAMPLE_TS).expect("Parse failed");

        assert!(!module.import_refs.is_empty(), "Should find import statements");
        let sources: Vec<&str> = module.import_refs.iter().map(|i| i.source_module.as_str()).collect();
        assert!(sources.contains(&"./repository") || sources.contains(&"./utils"),
            "Should find correct import sources, got: {:?}", sources);
    }

    #[test]
    fn test_heuristics_computation() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let path = Path::new("src/complex.ts");
        let complex_fn = r#"
async function complexFunction(a: string, b: number, c: boolean): Promise<void> {
    if (a.length > 0) {
        for (let i = 0; i < b; i++) {
            if (c && a.includes('x')) {
                try {
                    await doSomething(a, i);
                } catch (e) {
                    console.error(e);
                }
            } else if (!c || b > 100) {
                await doOther(a);
            }
        }
    }
}
"#;
        let module = parser.parse_source(path, complex_fn).expect("Parse failed");
        assert!(!module.functions.is_empty(), "Should find function");
        let fn_node = &module.functions[0];
        // Complex function should have high cyclomatic complexity
        assert!(fn_node.heuristics.cyclomatic_complexity > 3,
            "Complex function should have cyclomatic > 3, got: {}", fn_node.heuristics.cyclomatic_complexity);
        assert!(fn_node.heuristics.param_count >= 3,
            "Should count 3 params, got: {}", fn_node.heuristics.param_count);
    }

    #[test]
    fn test_sloc_counting() {
        let source = "// comment\n\nconst x = 1;\n// another comment\nconst y = 2;";
        assert_eq!(count_sloc(source), 2); // Only the 2 const lines
    }

    #[test]
    fn test_nonexistent_file_returns_error() {
        let mut parser = TypeScriptParser::new().expect("Parser init failed");
        let result = parser.parse_file(Path::new("/nonexistent/file.ts"));
        assert!(result.is_err());
    }
}
