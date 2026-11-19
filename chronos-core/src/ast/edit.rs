//! # TextEdit Primitive & Fresh-Parse Symbol Span Resolver
//!
//! Provides byte-span `TextEdit` operations and fresh-from-disk AST symbol span resolution
//! shared across:
//! - Pillar 2 (`MutationArena` / Super-Mutant source synthesis)
//! - v1.8 Symbolic AST Editing & Atomic Refactoring (`chronos_edit_symbol`, `chronos_refactor_symbol`)

use crate::ast::atomic_writer::{AtomicFileWriter, FileEncodingMetadata};
use crate::ast::dispatcher::LanguageDispatcher;
use crate::ast::ts_parser::ParsedModule;
use crate::graph::schema::{ChronosNode, NodeKind};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A single byte-span replacement in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub file: PathBuf,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub replacement: String,
}

impl TextEdit {
    pub fn new(
        file: impl Into<PathBuf>,
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        end_line: usize,
        replacement: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            start_byte,
            end_byte,
            start_line,
            end_line,
            replacement: replacement.into(),
        }
    }

    /// Apply a single `TextEdit` to an in-memory source string, checking UTF-8 char boundaries.
    pub fn apply_in_memory(&self, source: &str) -> Result<String> {
        apply_edits_in_memory(source, std::slice::from_ref(self))
    }
}

/// Apply non-overlapping `TextEdit`s in reverse byte order to preserve earlier byte offsets.
pub fn apply_edits_in_memory(source: &str, edits: &[TextEdit]) -> Result<String> {
    if edits.is_empty() {
        return Ok(source.to_string());
    }
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.start_byte
            .cmp(&a.start_byte)
            .then_with(|| b.end_byte.cmp(&a.end_byte))
    });

    let mut out = source.to_string();
    let mut last_start = usize::MAX;

    for edit in sorted {
        if edit.start_byte > edit.end_byte || edit.end_byte > source.len() {
            bail!(
                "TextEdit byte range {}..{} out of bounds for source length {}",
                edit.start_byte,
                edit.end_byte,
                source.len()
            );
        }
        if edit.end_byte > last_start {
            bail!(
                "Overlapping TextEdit detected ending at {} (next started at {})",
                edit.end_byte,
                last_start
            );
        }
        if !source.is_char_boundary(edit.start_byte) || !source.is_char_boundary(edit.end_byte) {
            bail!(
                "TextEdit byte range {}..{} is not on UTF-8 char boundaries",
                edit.start_byte,
                edit.end_byte
            );
        }
        out.replace_range(edit.start_byte..edit.end_byte, &edit.replacement);
        last_start = edit.start_byte;
    }

    Ok(out)
}

/// Contiguous UTF-8 byte & 1-indexed line span in a fresh source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl ByteSpan {
    pub fn from_ts_node(node: tree_sitter::Node<'_>) -> Self {
        let s = node.start_position();
        let e = node.end_position();
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: s.row + 1,
            end_line: e.row + 1,
            start_col: s.column,
            end_col: e.column,
        }
    }

    pub fn from_byte_range(source: &str, start_byte: usize, end_byte: usize) -> Self {
        let clamped_start = start_byte.min(source.len());
        let clamped_end = end_byte.max(clamped_start).min(source.len());
        let (start_line, start_col) = byte_to_line_col(source, clamped_start);
        let (end_line, end_col) = byte_to_line_col(source, clamped_end);
        Self {
            start_byte: clamped_start,
            end_byte: clamped_end,
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        let start = self.start_byte.min(source.len());
        let end = self.end_byte.max(start).min(source.len());
        &source[start..end]
    }
}

/// Exact fresh-parsed CST spans for a single symbol (declaration vs. attached leading trivia).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshSymbolSpan {
    pub name: String,
    pub qualified_name: String,
    pub kind: NodeKind,
    pub enclosing_type: Option<String>,
    /// Span of the symbol declaration node itself (`fn ...`, `def ...`, `function ...`, `class ...`).
    pub declaration_span: ByteSpan,
    /// Full span including immediately attached leading doc-comments, attributes (`#[...]`),
    /// decorators (`@...`), and any wrapping `export` statement.
    pub full_symbol_span: ByteSpan,
    /// Attached leading doc-comments / attributes / decorators prior to `declaration_or_export_start`.
    pub leading_trivia_span: Option<ByteSpan>,
    /// Exact byte span of the declared identifier name token inside `declaration_span`.
    pub name_ident_span: Option<ByteSpan>,
    /// Wrapping `export ` prefix span in TypeScript/JavaScript, when applicable.
    pub export_prefix_span: Option<ByteSpan>,
    /// Whitespace indentation prefix on the declaration's starting line.
    pub base_indent: String,
    pub is_exported: bool,
    pub has_entrypoint_attr: bool,
    pub is_trait_impl: bool,
    #[serde(default)]
    pub interface_impl_reason: Option<String>,
}

/// Fresh-from-disk parse result combining Tree-sitter CST spans and `ParsedModule`.
#[derive(Debug, Clone)]
pub struct FreshParsedFile {
    pub abs_path: PathBuf,
    pub source: String,
    pub encoding: FileEncodingMetadata,
    pub parsed: ParsedModule,
    pub symbols: Vec<FreshSymbolSpan>,
    pub has_syntax_errors: bool,
}

impl FreshParsedFile {
    /// Read and freshly parse `abs_path` directly from disk.
    pub fn from_disk(abs_path: &Path) -> Result<Self> {
        let (source, encoding) = AtomicFileWriter::read_utf8_preserving_metadata(abs_path)?;
        Self::from_source_with_encoding(abs_path, source, encoding)
    }

    /// Freshly parse an in-memory source buffer with known file extension and encoding metadata.
    pub fn from_source_with_encoding(
        abs_path: &Path,
        source: String,
        encoding: FileEncodingMetadata,
    ) -> Result<Self> {
        let lang = LanguageDispatcher::language_for_path(abs_path)?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&lang)
            .context("Failed to configure Tree-sitter language for fresh parse")?;
        let tree = parser
            .parse(&source, None)
            .context("Tree-sitter fresh parse returned None")?;
        let has_syntax_errors = tree.root_node().has_error();

        let mut dispatcher = LanguageDispatcher::new()?;
        let parsed = dispatcher.parse_source(abs_path, &source)?;

        let mut symbols = Vec::new();
        for node in parsed
            .classes
            .iter()
            .chain(parsed.functions.iter())
            .chain(parsed.test_cases.iter())
            .chain(parsed.api_routes.iter())
        {
            symbols.push(locate_symbol_with_trivia(&tree, &source, node));
        }

        Ok(Self {
            abs_path: abs_path.to_path_buf(),
            source,
            encoding,
            parsed,
            symbols,
            has_syntax_errors,
        })
    }

    /// Resolve a target symbol by exact name, `qualified_name`, `Class/method`, or `Class::method`
    /// purely against the freshly parsed CST symbols (never cached graph byte offsets).
    pub fn resolve_symbol(&self, query: &str) -> Option<&FreshSymbolSpan> {
        let matches = self.resolve_symbols(query);
        matches.into_iter().next()
    }

    /// Resolve all matching symbols in priority order.
    pub fn resolve_symbols(&self, query: &str) -> Vec<&FreshSymbolSpan> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let (parent_filter, leaf) = if let Some((p, l)) = q.rsplit_once('/') {
            (Some(p), l)
        } else if let Some((p, l)) = q.rsplit_once("::") {
            (Some(p), l)
        } else if let Some((p, l)) = q.rsplit_once('.') {
            (Some(p), l)
        } else {
            (None, q)
        };

        let mut scored: Vec<(u8, &FreshSymbolSpan)> = Vec::new();
        for sym in &self.symbols {
            let tier = if sym.name == q || sym.qualified_name == q {
                0
            } else if let Some(parent) = parent_filter {
                let leaf_matches = sym.name == leaf
                    || sym.name.ends_with(&format!(".{leaf}"))
                    || sym.name.ends_with(&format!("::{leaf}"));
                let parent_matches = sym
                    .enclosing_type
                    .as_deref()
                    .map(|t| t == parent)
                    .unwrap_or(false)
                    || sym.qualified_name.contains(parent)
                    || sym.name.starts_with(&format!("{parent}."))
                    || sym.name.starts_with(&format!("{parent}::"));
                if leaf_matches && parent_matches {
                    1
                } else {
                    continue;
                }
            } else if sym.name.ends_with(&format!(".{q}"))
                || sym.name.ends_with(&format!("::{q}"))
            {
                2
            } else {
                continue;
            };
            scored.push((tier, sym));
        }
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.declaration_span.start_byte.cmp(&b.1.declaration_span.start_byte))
        });
        scored.into_iter().map(|(_, s)| s).collect()
    }
}

/// Compute `declaration_span` vs. `full_symbol_span` (including immediately attached leading
/// doc-comments, attributes `#[...]`, decorators `@...`, and `export` wrappers) from a fresh CST.
pub fn locate_symbol_with_trivia(
    tree: &tree_sitter::Tree,
    source: &str,
    node: &ChronosNode,
) -> FreshSymbolSpan {
    let raw_start = node.location.start_byte;
    let raw_end = node.location.end_byte;

    let Some(decl_node) = find_cst_node_for_chronos(tree, source, node, raw_start, raw_end) else {
        let fallback_span = if raw_end > raw_start && raw_end <= source.len() {
            ByteSpan::from_byte_range(source, raw_start, raw_end)
        } else {
            line_range_to_byte_span(source, node.location.start_line, node.location.end_line)
        };
        let base_indent = extract_line_indent(source, fallback_span.start_byte);
        let lower_name = node.name.to_lowercase();
        let has_entrypoint_attr =
            lower_name == "main" || node.kind == NodeKind::ApiRoute || node.kind == NodeKind::TestCase;
        return FreshSymbolSpan {
            name: node.name.clone(),
            qualified_name: node.qualified_name.clone(),
            kind: node.kind.clone(),
            enclosing_type: None,
            declaration_span: fallback_span,
            full_symbol_span: fallback_span,
            leading_trivia_span: None,
            name_ident_span: None,
            export_prefix_span: None,
            base_indent,
            is_exported: false,
            has_entrypoint_attr,
            is_trait_impl: false,
            interface_impl_reason: None,
        };
    };

    let declaration_span = ByteSpan::from_ts_node(decl_node);
    let enclosing_type = find_enclosing_type_name(decl_node, source.as_bytes());
    let name_ident_span = find_name_ident_span(decl_node).map(ByteSpan::from_ts_node);
    let interface_impl_reason = detect_interface_or_override_guard(tree.root_node(), decl_node, source);
    let is_trait_impl = interface_impl_reason.is_some();

    // Handle wrapping syntax nodes (`decorated_definition`, single `lexical_declaration`, `export_statement`)
    let mut anchor = decl_node;
    let mut outer_end = declaration_span.end_byte;
    let mut decl_or_export_start = declaration_span.start_byte;
    let mut export_prefix_span: Option<ByteSpan> = None;
    let mut inner_decorator_start: Option<usize> = None;

    if let Some(parent) = anchor.parent() {
        if parent.kind() == "decorated_definition" {
            // Python decorators live inside `decorated_definition` before `function_definition` / `class_definition`
            let p_start = parent.start_byte();
            if p_start < decl_or_export_start {
                inner_decorator_start = Some(p_start);
            }
            anchor = parent;
            outer_end = outer_end.max(parent.end_byte());
        } else if matches!(parent.kind(), "lexical_declaration" | "variable_declaration")
            && decl_node.kind() == "variable_declarator"
            && parent.named_child_count() == 1
        {
            anchor = parent;
            decl_or_export_start = parent.start_byte();
            outer_end = outer_end.max(parent.end_byte());
        }
    }

    let mut is_exported_wrapper = false;
    if let Some(parent) = anchor.parent() {
        if parent.kind() == "export_statement" {
            is_exported_wrapper = true;
            // Collect decorators inside `export_statement` before the `export` token if any
            let mut cursor = parent.walk();
            let mut export_kw_start: Option<usize> = None;
            let mut pre_export_decorator_start: Option<usize> = None;
            for child in parent.children(&mut cursor) {
                if child.id() == anchor.id() {
                    break;
                }
                if child.kind() == "decorator" && export_kw_start.is_none() {
                    pre_export_decorator_start = Some(
                        pre_export_decorator_start
                            .map(|prev| prev.min(child.start_byte()))
                            .unwrap_or_else(|| child.start_byte()),
                    );
                } else if !child.is_named() && child.utf8_text(source.as_bytes()).ok() == Some("export") {
                    export_kw_start = Some(child.start_byte());
                }
            }

            let export_start = export_kw_start.unwrap_or_else(|| parent.start_byte());
            if export_start < decl_or_export_start {
                export_prefix_span = Some(ByteSpan::from_byte_range(
                    source,
                    export_start,
                    decl_or_export_start,
                ));
                decl_or_export_start = export_start;
            }
            if let Some(dec_start) = pre_export_decorator_start {
                inner_decorator_start = Some(
                    inner_decorator_start
                        .map(|p| p.min(dec_start))
                        .unwrap_or(dec_start),
                );
            }
            anchor = parent;
            outer_end = outer_end.max(parent.end_byte());
        }
    }

    let seed_trivia_start = inner_decorator_start.unwrap_or(decl_or_export_start);
    let trivia_start = walk_leading_trivia_start(anchor, source, seed_trivia_start);
    let trivia_end = if inner_decorator_start.is_some() {
        declaration_span.start_byte
    } else {
        decl_or_export_start
    };

    let leading_trivia_span = if trivia_start < trivia_end {
        Some(ByteSpan::from_byte_range(source, trivia_start, trivia_end))
    } else {
        None
    };

    let full_symbol_span = ByteSpan::from_byte_range(source, trivia_start, outer_end);
    let base_indent = extract_line_indent(source, declaration_span.start_byte);

    let full_text = full_symbol_span.slice(source);
    let is_exported = is_exported_wrapper
        || has_visibility_export(decl_node, source, &node.name);
    let short_name = node
        .name
        .rsplit(|c| c == '.' || c == ':')
        .next()
        .unwrap_or(&node.name);
    let has_entrypoint_attr = short_name == "main"
        || node.kind == NodeKind::ApiRoute
        || node.kind == NodeKind::TestCase
        || full_text.contains("#[test]")
        || full_text.contains("#[tokio::main]")
        || full_text.contains("#[async_std::main]")
        || full_text.contains("#[actix_web::main]")
        || full_text.contains("#[rocket::main]")
        || full_text.contains(".route(")
        || full_text.contains(".get(")
        || full_text.contains(".post(")
        || full_text.contains(".put(")
        || full_text.contains(".delete(");

    FreshSymbolSpan {
        name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
        kind: node.kind.clone(),
        enclosing_type,
        declaration_span,
        full_symbol_span,
        leading_trivia_span,
        name_ident_span,
        export_prefix_span,
        base_indent,
        is_exported,
        has_entrypoint_attr,
        is_trait_impl,
        interface_impl_reason,
    }
}

fn find_cst_node_for_chronos<'a>(
    tree: &'a tree_sitter::Tree,
    source: &str,
    node: &ChronosNode,
    start_byte: usize,
    end_byte: usize,
) -> Option<tree_sitter::Node<'a>> {
    if end_byte <= start_byte || end_byte > source.len() {
        return None;
    }
    let mut curr = tree
        .root_node()
        .descendant_for_byte_range(start_byte, end_byte)?;
    while let Some(parent) = curr.parent() {
        if parent.start_byte() == start_byte
            && parent.end_byte() == end_byte
            && parent.kind() != "source_file"
            && parent.kind() != "program"
            && parent.kind() != "module"
            && parent.kind() != "translation_unit"
        {
            curr = parent;
        } else {
            break;
        }
    }
    let _ = node;
    Some(curr)
}

fn walk_leading_trivia_start(
    mut curr: tree_sitter::Node<'_>,
    source: &str,
    mut earliest_byte: usize,
) -> usize {
    while let Some(prev) = curr.prev_sibling() {
        if !is_attached_trivia_kind(prev.kind()) {
            break;
        }
        let gap_start = prev.end_byte();
        let gap_end = curr.start_byte().min(earliest_byte);
        if gap_start > gap_end || gap_end > source.len() {
            break;
        }
        let gap = &source[gap_start..gap_end];
        if !gap.trim().is_empty() {
            break;
        }
        // A blank line (2+ newlines in the gap) detaches the comment/block from this symbol.
        let newline_count = gap.bytes().filter(|&b| b == b'\n').count();
        if newline_count > 1 {
            break;
        }
        earliest_byte = earliest_byte.min(prev.start_byte());
        curr = prev;
    }
    earliest_byte
}

fn is_attached_trivia_kind(kind: &str) -> bool {
    matches!(
        kind,
        "attribute_item"
            | "inner_attribute_item"
            | "decorator"
            | "line_comment"
            | "block_comment"
            | "comment"
            | "doc_comment"
            | "marker_annotation"
            | "annotation"
    )
}

fn find_enclosing_type_name(mut node: tree_sitter::Node<'_>, src: &[u8]) -> Option<String> {
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "class_declaration" | "class_definition" | "struct_item" | "interface_declaration" => {
                if let Some(name) = parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(src).ok())
                {
                    return Some(name.to_string());
                }
            }
            "impl_item" => {
                if let Some(ty) = parent
                    .child_by_field_name("type")
                    .and_then(|n| n.utf8_text(src).ok())
                {
                    let clean = ty.split('<').next().unwrap_or(ty).trim();
                    if !clean.is_empty() {
                        return Some(clean.to_string());
                    }
                }
            }
            _ => {}
        }
        node = parent;
    }
    None
}

fn find_name_ident_span(decl_node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if let Some(name_node) = decl_node.child_by_field_name("name") {
        return Some(name_node);
    }
    // C/C++ declarators nest `declarator` -> `identifier` / `field_identifier`
    let mut cursor_node = decl_node;
    while let Some(inner) = cursor_node.child_by_field_name("declarator") {
        if matches!(
            inner.kind(),
            "identifier" | "field_identifier" | "property_identifier" | "type_identifier"
        ) {
            return Some(inner);
        }
        cursor_node = inner;
    }
    let mut cursor = cursor_node.walk();
    for child in cursor_node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier" | "field_identifier" | "property_identifier" | "type_identifier"
        ) {
            return Some(child);
        }
    }
    None
}

fn detect_interface_or_override_guard(
    file_root: tree_sitter::Node<'_>,
    decl_node: tree_sitter::Node<'_>,
    source: &str,
) -> Option<String> {
    let bytes = source.as_bytes();

    // 1. Rust trait `impl Trait for Type`
    if let Some(trait_name) = find_rust_trait_impl_name(decl_node, bytes) {
        return Some(format!("Rust trait implementation method ('{trait_name}')"));
    }

    // 2. TypeScript `override` modifier or enclosing class `implements` clause
    if decl_node.kind() == "method_definition" {
        if has_child_kind(decl_node, "override_modifier") {
            return Some("TypeScript method carries 'override' modifier".to_string());
        }
        let method_name = decl_node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(bytes).ok())
            .unwrap_or("");
        let is_private_or_static = method_name.starts_with('#')
            || has_ts_private_or_static(decl_node, bytes);
        if !is_private_or_static && !method_name.is_empty() {
            if let Some(implements_names) = find_ts_implements_names(decl_node, bytes) {
                let local_decl_matches =
                    check_local_ts_interface_methods(file_root, bytes, &implements_names, method_name);
                if local_decl_matches {
                    return Some(format!(
                        "TypeScript class method '{method_name}' implements interface '{}'",
                        implements_names.join(", ")
                    ));
                }
            }
        }
    }

    // 3. Java `@Override` annotation or enclosing class `implements` clause
    if decl_node.kind() == "method_declaration" && decl_node.child_by_field_name("receiver").is_none()
    {
        if has_java_override_annotation(decl_node, bytes) {
            return Some("Java method carries @Override annotation".to_string());
        }
        if !has_java_private_or_static(decl_node, bytes) {
            if let Some(ifaces) = find_java_super_interfaces(decl_node, bytes) {
                return Some(format!("Java class implements interface(s) '{ifaces}'"));
            }
        }
    }

    // 4. Go receiver `method_declaration` matching a same-file `interface_type` (`method_elem`)
    if decl_node.kind() == "method_declaration" && decl_node.child_by_field_name("receiver").is_some()
    {
        if let Some((method_name, param_count)) = extract_go_method_sig(decl_node, bytes) {
            if let Some(iface) =
                find_go_interface_match_in_tree(file_root, bytes, &method_name, param_count)
            {
                return Some(iface);
            }
        }
    }

    None
}

fn find_rust_trait_impl_name(mut node: tree_sitter::Node<'_>, bytes: &[u8]) -> Option<String> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "impl_item" {
            if let Some(trait_node) = parent.child_by_field_name("trait") {
                return Some(
                    trait_node
                        .utf8_text(bytes)
                        .unwrap_or("Trait")
                        .trim()
                        .to_string(),
                );
            }
        }
        node = parent;
    }
    None
}

fn has_child_kind(node: tree_sitter::Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).any(|c| c.kind() == kind);
    found
}

fn has_ts_private_or_static(method_node: tree_sitter::Node<'_>, bytes: &[u8]) -> bool {
    let mut cursor = method_node.walk();
    for child in method_node.children(&mut cursor) {
        if child.kind() == "private_property_identifier" {
            return true;
        }
        if child.kind() == "accessibility_modifier" {
            if child.utf8_text(bytes).map(|t| t.trim() == "private").unwrap_or(false) {
                return true;
            }
        }
        if child.kind() == "static" {
            return true;
        }
    }
    false
}

fn find_ts_implements_names(mut node: tree_sitter::Node<'_>, bytes: &[u8]) -> Option<Vec<String>> {
    while let Some(parent) = node.parent() {
        if matches!(parent.kind(), "class_declaration" | "abstract_class_declaration") {
            let mut c_cur = parent.walk();
            for child in parent.children(&mut c_cur) {
                if child.kind() == "class_heritage" {
                    let mut h_cur = child.walk();
                    for clause in child.children(&mut h_cur) {
                        if clause.kind() == "implements_clause" {
                            let text = clause.utf8_text(bytes).unwrap_or("");
                            let raw = text.trim().strip_prefix("implements").unwrap_or(text);
                            let names: Vec<String> = raw
                                .split(',')
                                .map(|s| s.split('<').next().unwrap_or(s).trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if !names.is_empty() {
                                return Some(names);
                            }
                        }
                    }
                }
            }
        }
        node = parent;
    }
    None
}

/// Returns `true` if either an implemented interface in this file explicitly declares `method_name`,
/// OR none of the implemented interfaces are defined in this file (imported interface).
fn check_local_ts_interface_methods(
    root: tree_sitter::Node<'_>,
    bytes: &[u8],
    implements_names: &[String],
    method_name: &str,
) -> bool {
    let mut found_any_local_iface = false;
    let mut matched = false;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let iface_node = if child.kind() == "interface_declaration" {
            Some(child)
        } else if child.kind() == "export_statement" {
            child
                .child_by_field_name("declaration")
                .filter(|d| d.kind() == "interface_declaration")
        } else {
            None
        };
        let Some(iface) = iface_node else { continue };
        let Some(iface_name) = iface
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(bytes).ok())
        else {
            continue;
        };
        if !implements_names
            .iter()
            .any(|imp| imp == iface_name || imp.ends_with(&format!(".{iface_name}")))
        {
            continue;
        }
        found_any_local_iface = true;
        if let Some(body) = iface.child_by_field_name("body") {
            let mut b_cur = body.walk();
            for member in body.children(&mut b_cur) {
                if member
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(bytes).ok())
                    .map(|n| n == method_name)
                    .unwrap_or(false)
                {
                    matched = true;
                }
            }
        }
    }
    !found_any_local_iface || matched
}

fn has_java_override_annotation(method_node: tree_sitter::Node<'_>, bytes: &[u8]) -> bool {
    let mut cursor = method_node.walk();
    for child in method_node.children(&mut cursor) {
        if matches!(child.kind(), "marker_annotation" | "annotation")
            && annotation_is_override(child, bytes)
        {
            return true;
        }
        if child.kind() == "modifiers" {
            let mut m_cur = child.walk();
            for m in child.children(&mut m_cur) {
                if matches!(m.kind(), "marker_annotation" | "annotation")
                    && annotation_is_override(m, bytes)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn annotation_is_override(node: tree_sitter::Node<'_>, bytes: &[u8]) -> bool {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(bytes).ok())
        .map(|name| name == "Override" || name.ends_with(".Override"))
        .unwrap_or(false)
}

fn has_java_private_or_static(method_node: tree_sitter::Node<'_>, bytes: &[u8]) -> bool {
    let mut cursor = method_node.walk();
    for child in method_node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let text = child.utf8_text(bytes).unwrap_or("");
            if text.split_whitespace().any(|tok| tok == "private" || tok == "static") {
                return true;
            }
        }
    }
    false
}

fn find_java_super_interfaces(mut node: tree_sitter::Node<'_>, bytes: &[u8]) -> Option<String> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "class_declaration" | "enum_declaration" | "record_declaration"
        ) {
            if let Some(ifaces) = parent.child_by_field_name("interfaces") {
                let text = ifaces.utf8_text(bytes).unwrap_or("").trim();
                let cleaned = text.strip_prefix("implements").unwrap_or(text).trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
        node = parent;
    }
    None
}

/// Extract `(method_name, parameter_count)` from a Go `method_declaration` (receiver method).
pub(crate) fn extract_go_method_sig(
    method_node: tree_sitter::Node<'_>,
    bytes: &[u8],
) -> Option<(String, usize)> {
    if method_node.kind() != "method_declaration"
        || method_node.child_by_field_name("receiver").is_none()
    {
        return None;
    }
    let name = method_node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(bytes).ok())?
        .trim()
        .to_string();
    let params = method_node.child_by_field_name("parameters")?;
    Some((name, count_go_parameter_list(params)))
}

fn count_go_parameter_list(param_list: tree_sitter::Node<'_>) -> usize {
    let mut count = 0usize;
    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        match child.kind() {
            "parameter_declaration" => {
                // `a, b int` has 2 `identifier` children (`name` fields); `int` alone has 0 `identifier` children -> 1 param.
                let mut idents = 0usize;
                let mut p_cur = child.walk();
                for sub in child.children(&mut p_cur) {
                    if sub.kind() == "identifier" {
                        idents += 1;
                    }
                }
                count += idents.max(1);
            }
            "variadic_parameter_declaration" => {
                count += 1;
            }
            _ => {}
        }
    }
    count
}

/// Search `tree_root` for any Go `interface_type` whose `method_elem` matches `(method_name, param_count)`.
pub(crate) fn find_go_interface_match_in_tree(
    node: tree_sitter::Node<'_>,
    bytes: &[u8],
    method_name: &str,
    param_count: usize,
) -> Option<String> {
    if node.kind() == "interface_type" {
        let iface_name = node
            .parent()
            .filter(|p| p.kind() == "type_spec")
            .and_then(|p| p.child_by_field_name("name"))
            .and_then(|n| n.utf8_text(bytes).ok())
            .unwrap_or("interface");

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "method_elem" {
                let elem_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(bytes).ok())
                    .unwrap_or("");
                let elem_params = child
                    .child_by_field_name("parameters")
                    .map(count_go_parameter_list)
                    .unwrap_or(0);
                if elem_name == method_name && elem_params == param_count {
                    return Some(format!(
                        "Go receiver method '{method_name}' matches interface '{iface_name}.{method_name}'"
                    ));
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_go_interface_match_in_tree(child, bytes, method_name, param_count) {
            return Some(found);
        }
    }
    None
}

fn has_visibility_export(decl_node: tree_sitter::Node<'_>, source: &str, name: &str) -> bool {
    if decl_node.child_by_field_name("visibility_modifier").is_some() {
        return true;
    }
    let mut cursor = decl_node.walk();
    for child in decl_node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
        if child.kind() == "modifiers"
            && child
                .utf8_text(source.as_bytes())
                .map(|t| t.contains("public"))
                .unwrap_or(false)
        {
            return true;
        }
    }
    // Go exported identifiers start with an uppercase ASCII character in `func Foo(...)`
    if decl_node.kind() == "function_declaration" || decl_node.kind() == "type_declaration" {
        let short = name.rsplit('.').next().unwrap_or(name);
        if decl_node
            .utf8_text(source.as_bytes())
            .map(|t| t.starts_with("func ") || t.starts_with("type "))
            .unwrap_or(false)
            && short.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn extract_line_indent(source: &str, byte_pos: usize) -> String {
    let pos = byte_pos.min(source.len());
    let line_start = source[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

fn line_range_to_byte_span(source: &str, start_line: usize, end_line: usize) -> ByteSpan {
    let mut line_starts = vec![0usize];
    for (idx, b) in source.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(idx + 1);
        }
    }
    let s_idx = start_line.saturating_sub(1).min(line_starts.len().saturating_sub(1));
    let e_idx = end_line.max(start_line);
    let start_byte = line_starts[s_idx];
    let end_byte = if e_idx < line_starts.len() {
        line_starts[e_idx].saturating_sub(1)
    } else {
        source.len()
    };
    ByteSpan::from_byte_range(source, start_byte, end_byte)
}

fn byte_to_line_col(source: &str, byte_pos: usize) -> (usize, usize) {
    let prefix = &source[..byte_pos.min(source.len())];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = prefix
        .rfind('\n')
        .map(|nl| prefix.len() - nl - 1)
        .unwrap_or(prefix.len());
    (line, col)
}

/// Generate a compact human-readable unified diff between `old_src` and `new_src`.
pub fn generate_unified_diff(file_label: &str, old_src: &str, new_src: &str) -> String {
    if old_src == new_src {
        return String::new();
    }
    let old_lines: Vec<&str> = old_src.lines().collect();
    let new_lines: Vec<&str> = new_src.lines().collect();

    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut old_end = old_lines.len();
    let mut new_end = new_lines.len();
    while old_end > prefix
        && new_end > prefix
        && old_lines[old_end - 1] == new_lines[new_end - 1]
    {
        old_end -= 1;
        new_end -= 1;
    }

    let ctx_before = prefix.saturating_sub(2);
    let ctx_after_old = (old_end + 2).min(old_lines.len());
    let ctx_after_new = (new_end + 2).min(new_lines.len());

    let mut out = format!("--- a/{}\n+++ b/{}\n", file_label, file_label);
    out.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        ctx_before + 1,
        ctx_after_old.saturating_sub(ctx_before),
        ctx_before + 1,
        ctx_after_new.saturating_sub(ctx_before)
    ));
    for line in &old_lines[ctx_before..prefix] {
        out.push_str(&format!(" {line}\n"));
    }
    for line in &old_lines[prefix..old_end] {
        out.push_str(&format!("-{line}\n"));
    }
    for line in &new_lines[prefix..new_end] {
        out.push_str(&format!("+{line}\n"));
    }
    for line in &old_lines[old_end..ctx_after_old] {
        out.push_str(&format!(" {line}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_edits_in_memory_reverse_order() {
        let src = "fn alpha() { 1 } fn beta() { 2 }";
        let e1 = TextEdit::new("a.rs", 13, 14, 1, 1, "100");
        let e2 = TextEdit::new("a.rs", 29, 30, 1, 1, "200");
        let updated = apply_edits_in_memory(src, &[e1, e2]).unwrap();
        assert_eq!(updated, "fn alpha() { 100 } fn beta() { 200 }");
    }

    #[test]
    fn test_apply_edits_rejects_overlap_and_bad_bounds() {
        let src = "hello world";
        let overlap = [
            TextEdit::new("a.rs", 0, 7, 1, 1, "x"),
            TextEdit::new("a.rs", 5, 11, 1, 1, "y"),
        ];
        assert!(apply_edits_in_memory(src, &overlap).is_err());

        let oob = TextEdit::new("a.rs", 0, 999, 1, 1, "z");
        assert!(oob.apply_in_memory(src).is_err());
    }

    #[test]
    fn test_fresh_parse_locates_leading_attributes_and_doc_comments_across_rust_python_ts() {
        // 1. Rust: attached #[inline] + /// doc vs. detached header comment across blank line
        let rs_src = "// Section banner\n\n#[inline]\n/// Computes sum\npub fn compute_sum(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let rs_fresh = FreshParsedFile::from_source_with_encoding(
            Path::new("lib.rs"),
            rs_src.to_string(),
            FileEncodingMetadata::default(),
        )
        .unwrap();
        let rs_sym = rs_fresh.resolve_symbol("compute_sum").unwrap();
        assert!(rs_sym.declaration_span.slice(rs_src).starts_with("pub fn compute_sum"));
        let rs_full = rs_sym.full_symbol_span.slice(rs_src);
        assert!(rs_full.starts_with("#[inline]\n/// Computes sum\npub fn compute_sum"));
        assert!(!rs_full.contains("Section banner"), "Blank-line separated comment must not attach");
        assert!(rs_sym.is_exported);

        // 2. Python: indented method with @staticmethod decorator + leading comment
        let py_src = "class MathSvc:\n    @staticmethod\n    # helper comment\n    def multiply(a, b):\n        return a * b\n";
        let py_fresh = FreshParsedFile::from_source_with_encoding(
            Path::new("svc.py"),
            py_src.to_string(),
            FileEncodingMetadata::default(),
        )
        .unwrap();
        let py_sym = py_fresh.resolve_symbol("MathSvc/multiply").unwrap();
        assert!(py_sym.declaration_span.slice(py_src).starts_with("def multiply(a, b):"));
        let py_full = py_sym.full_symbol_span.slice(py_src);
        assert!(py_full.starts_with("@staticmethod"));
        assert!(py_full.contains("def multiply(a, b):"));
        assert_eq!(py_sym.base_indent, "    ");

        // 3. TypeScript: JSDoc comment + export keyword wrapping function declaration
        let ts_src = "// header\n\n/** Greets a user */\nexport function greet(name: string): string {\n  return `Hi ${name}`;\n}\n";
        let ts_fresh = FreshParsedFile::from_source_with_encoding(
            Path::new("greet.ts"),
            ts_src.to_string(),
            FileEncodingMetadata::default(),
        )
        .unwrap();
        let ts_sym = ts_fresh.resolve_symbol("greet").unwrap();
        assert!(ts_sym.declaration_span.slice(ts_src).starts_with("function greet("));
        let ts_full = ts_sym.full_symbol_span.slice(ts_src);
        assert!(ts_full.starts_with("/** Greets a user */\nexport function greet("));
        assert!(ts_sym.is_exported);
        assert!(ts_sym.export_prefix_span.is_some());
    }
}
