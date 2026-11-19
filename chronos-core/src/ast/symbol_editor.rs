//! # Symbol-Boundary AST Editing Engine (`chronos_edit_symbol`)
//!
//! Performs surgical AST-bounded source edits without trusting stale graph offsets:
//! 1. Freshly reads and re-parses the target file from disk (`WorkspaceSession::parse_fresh_file`)
//! 2. Separates `declaration_span` vs. `full_symbol_span` to deduplicate attributes/decorators/docs
//! 3. Normalizes indentation to the target column and preserves CRLF/LF + UTF-8 BOM
//! 4. Stage 1 In-Memory Tree-sitter Syntax Gate
//! 5. Stage 2 Contract & Caller Blast-Radius Gate (`blocked_requires_force` unless `force: true`)
//! 6. Atomic disk write + immediate graph delta sync.

use crate::ast::atomic_writer::AtomicFileWriter;
use crate::ast::edit::{
    apply_edits_in_memory, generate_unified_diff, FreshParsedFile, FreshSymbolSpan, TextEdit,
};
use crate::graph::query::{GraphQueryEngine, VerificationSurface};
use crate::session::WorkspaceSession;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Symbolic edit operation supported by `chronos_edit_symbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOperation {
    ReplaceBody,
    InsertBefore,
    InsertAfter,
}

impl EditOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            EditOperation::ReplaceBody => "replace_body",
            EditOperation::InsertBefore => "insert_before",
            EditOperation::InsertAfter => "insert_after",
        }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "replace_body" | "replace" => Ok(EditOperation::ReplaceBody),
            "insert_before" | "before" => Ok(EditOperation::InsertBefore),
            "insert_after" | "after" => Ok(EditOperation::InsertAfter),
            other => Err(anyhow!(
                "Unsupported edit operation '{}'. Expected 'replace_body', 'insert_before', or 'insert_after'.",
                other
            )),
        }
    }
}

/// Request payload for `WorkspaceSession::edit_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditSymbolRequest {
    pub symbol: String,
    pub file: PathBuf,
    pub operation: EditOperation,
    pub new_code: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub force: bool,
}

/// Structured result of `WorkspaceSession::edit_symbol` including 2-Stage Gate diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditSymbolOutcome {
    /// `"applied"` | `"dry_run"` | `"syntax_error"` | `"blocked_requires_force"`
    pub status: String,
    pub is_error: bool,
    pub symbol: String,
    pub file: String,
    pub operation: String,
    pub old_signature: Option<String>,
    pub new_signature: Option<String>,
    pub signature_changed: bool,
    pub syntax_error: Option<(usize, usize)>,
    pub fan_in: usize,
    pub affected_callers: Vec<String>,
    pub verification_surfaces: Vec<VerificationSurface>,
    pub unified_diff: String,
    pub summary: String,
}

/// Execute a 2-stage gated symbolic edit against a fresh disk parse inside `session`.
pub async fn execute_edit_symbol(
    session: &mut WorkspaceSession,
    req: EditSymbolRequest,
) -> Result<EditSymbolOutcome> {
    if !req.dry_run {
        session.ensure_writable()?;
    }

    let fresh = session.parse_fresh_file(&req.file)?;
    let target = fresh
        .resolve_symbol(&req.symbol)
        .ok_or_else(|| {
            anyhow!(
                "Symbol '{}' not found in freshly parsed '{}'",
                req.symbol,
                fresh.abs_path.display()
            )
        })?
        .clone();

    let edit = build_symbol_text_edit(&fresh, &target, req.operation, &req.new_code)?;
    let candidate_source = apply_edits_in_memory(&fresh.source, std::slice::from_ref(&edit))?;

    let file_rel = fresh
        .abs_path
        .strip_prefix(&session.workspace_root)
        .unwrap_or(&fresh.abs_path)
        .to_string_lossy()
        .replace('\\', "/");
    let unified_diff = generate_unified_diff(&file_rel, &fresh.source, &candidate_source);

    // Stage 1: In-Memory Tree-sitter Syntax Gate (never writes broken syntax to disk)
    if let Some((line, col)) = WorkspaceSession::validate_syntax(&fresh.abs_path, &candidate_source)? {
        return Ok(EditSymbolOutcome {
            status: "syntax_error".to_string(),
            is_error: true,
            symbol: target.name.clone(),
            file: file_rel,
            operation: req.operation.as_str().to_string(),
            old_signature: None,
            new_signature: None,
            signature_changed: false,
            syntax_error: Some((line, col)),
            fan_in: 0,
            affected_callers: Vec::new(),
            verification_surfaces: Vec::new(),
            unified_diff,
            summary: format!(
                "Syntax validation failed at {}:{} — disk untouched.",
                line, col
            ),
        });
    }

    // Stage 2: Contract & Caller Blast-Radius Gate (directly compares the target symbol's own
    // declaration signature from the fresh parse so multiple impl blocks / classes never collide)
    let old_fn_sig = Some(extract_symbol_declaration_signature(&fresh.source, &target));
    let new_fresh = FreshParsedFile::from_source_with_encoding(
        &fresh.abs_path,
        candidate_source.clone(),
        fresh.encoding,
    )?;
    let new_fn_sig = new_fresh
        .resolve_symbol(&req.symbol)
        .or_else(|| new_fresh.resolve_symbol(&target.name))
        .map(|s| extract_symbol_declaration_signature(&new_fresh.source, s));

    let signature_changed = match req.operation {
        EditOperation::ReplaceBody => match (&old_fn_sig, &new_fn_sig) {
            (Some(old_s), Some(new_s)) => old_s != new_s,
            (Some(_), None) => true, // symbol renamed or removed during ReplaceBody
            _ => false,
        },
        EditOperation::InsertBefore | EditOperation::InsertAfter => false,
    };

    let (affected_callers, verification_surfaces) =
        resolve_graph_dependents(session, &fresh, &target, &req.symbol);
    let fan_in = affected_callers.len();

    if req.dry_run {
        return Ok(EditSymbolOutcome {
            status: "dry_run".to_string(),
            is_error: false,
            symbol: target.name,
            file: file_rel,
            operation: req.operation.as_str().to_string(),
            old_signature: old_fn_sig,
            new_signature: new_fn_sig,
            signature_changed,
            syntax_error: None,
            fan_in,
            affected_callers,
            verification_surfaces,
            unified_diff,
            summary: if signature_changed && fan_in > 0 {
                format!(
                    "Dry-run valid syntax; contract signature changed with {} caller(s) — would require force=true.",
                    fan_in
                )
            } else {
                "Dry-run valid syntax; ready to apply.".to_string()
            },
        });
    }

    if signature_changed && fan_in > 0 && !req.force {
        return Ok(EditSymbolOutcome {
            status: "blocked_requires_force".to_string(),
            is_error: true,
            symbol: target.name,
            file: file_rel,
            operation: req.operation.as_str().to_string(),
            old_signature: old_fn_sig,
            new_signature: new_fn_sig,
            signature_changed: true,
            syntax_error: None,
            fan_in,
            affected_callers: affected_callers.clone(),
            verification_surfaces,
            unified_diff,
            summary: format!(
                "Blocked: contract signature changed for '{}' which has {} active caller(s) ({}). Inspect callers or pass force=true.",
                req.symbol,
                fan_in,
                affected_callers.join(", ")
            ),
        });
    }

    // Atomic write preserving BOM/CRLF + immediate graph delta re-index
    AtomicFileWriter::write_atomic_with_metadata(&fresh.abs_path, &candidate_source, fresh.encoding)?;
    let _report = session.index_file(&fresh.abs_path).await?;

    Ok(EditSymbolOutcome {
        status: "applied".to_string(),
        is_error: false,
        symbol: target.name,
        file: file_rel,
        operation: req.operation.as_str().to_string(),
        old_signature: old_fn_sig,
        new_signature: new_fn_sig,
        signature_changed,
        syntax_error: None,
        fan_in,
        affected_callers,
        verification_surfaces,
        unified_diff,
        summary: format!(
            "Applied '{}' on '{}' in {} and synced graph delta.",
            req.operation.as_str(),
            req.symbol,
            fresh.abs_path.display()
        ),
    })
}

fn build_symbol_text_edit(
    fresh: &FreshParsedFile,
    target: &FreshSymbolSpan,
    op: EditOperation,
    new_code: &str,
) -> Result<TextEdit> {
    let source = &fresh.source;
    let file = &fresh.abs_path;

    match op {
        EditOperation::ReplaceBody => {
            let new_has_trivia = code_starts_with_trivia(new_code);
            let new_has_export = new_code.trim_start().starts_with("export ");

            let replace_start = if new_has_trivia {
                target.full_symbol_span.start_byte
            } else if new_has_export {
                target
                    .export_prefix_span
                    .map(|s| s.start_byte)
                    .unwrap_or(target.declaration_span.start_byte)
            } else {
                target.declaration_span.start_byte
            };
            let replace_end = if new_has_trivia || new_has_export {
                target.full_symbol_span.end_byte
            } else {
                target.declaration_span.end_byte
            };

            let replacement = if new_has_trivia && target.export_prefix_span.is_some() && !new_has_export {
                // Re-insert `export ` right before the declaration part if replacement supplied leading JSDoc/decorators without `export`
                let prefix_already_present = line_prefix_is_whitespace_only(source, replace_start);
                let normalized = normalize_block_for_inline_start(
                    new_code,
                    &target.base_indent,
                    !prefix_already_present,
                );
                preserve_ts_export_keyword_if_split(&normalized)
            } else {
                let prefix_already_present = line_prefix_is_whitespace_only(source, replace_start);
                normalize_block_for_inline_start(
                    new_code,
                    &target.base_indent,
                    !prefix_already_present,
                )
            };

            Ok(TextEdit::new(
                file,
                replace_start,
                replace_end,
                target.full_symbol_span.start_line,
                target.full_symbol_span.end_line,
                replacement,
            ))
        }
        EditOperation::InsertBefore => {
            let anchor = target.full_symbol_span.start_byte;
            let line_start = source[..anchor].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let indented = indent_full_block(new_code, &target.base_indent);
            let insertion = format!("{}\n", indented);
            Ok(TextEdit::new(
                file,
                line_start,
                line_start,
                target.full_symbol_span.start_line,
                target.full_symbol_span.start_line,
                insertion,
            ))
        }
        EditOperation::InsertAfter => {
            let anchor = target.full_symbol_span.end_byte;
            let indented = indent_full_block(new_code, &target.base_indent);
            let insertion = format!("\n\n{}", indented);
            Ok(TextEdit::new(
                file,
                anchor,
                anchor,
                target.full_symbol_span.end_line,
                target.full_symbol_span.end_line,
                insertion,
            ))
        }
    }
}

fn preserve_ts_export_keyword_if_split(block: &str) -> String {
    let mut lines: Vec<String> = block.lines().map(|l| l.to_string()).collect();
    for line in &mut lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("function ")
            || trimmed.starts_with("async function ")
            || trimmed.starts_with("class ")
        {
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];
            *line = format!("{}export {}", indent, trimmed);
            break;
        }
    }
    lines.join("\n")
}

fn code_starts_with_trivia(code: &str) -> bool {
    let trimmed = code.trim_start();
    trimmed.starts_with("#[")
        || trimmed.starts_with('@')
        || trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/**")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("//")
        || trimmed.starts_with("# ")
}

fn line_prefix_is_whitespace_only(source: &str, byte_pos: usize) -> bool {
    let pos = byte_pos.min(source.len());
    let line_start = source[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..pos]
        .chars()
        .all(|c| c == ' ' || c == '\t')
}

fn dedent_trimmed_lines(raw_code: &str) -> Vec<String> {
    let normalized = raw_code.replace("\r\n", "\n");
    let trimmed = normalized.trim_matches('\n');
    let lines: Vec<&str> = trimmed.lines().collect();
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.bytes().take_while(|&b| b == b' ' || b == b'\t').count())
        .min()
        .unwrap_or(0);

    lines
        .into_iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                let byte_cut = l
                    .char_indices()
                    .take(min_indent)
                    .last()
                    .map(|(idx, ch)| idx + ch.len_utf8())
                    .unwrap_or(0);
                l[byte_cut.min(l.len())..].to_string()
            }
        })
        .collect()
}

fn indent_full_block(raw_code: &str, base_indent: &str) -> String {
    dedent_trimmed_lines(raw_code)
        .into_iter()
        .map(|l| {
            if l.is_empty() {
                l
            } else {
                format!("{base_indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_block_for_inline_start(
    raw_code: &str,
    base_indent: &str,
    indent_first_line: bool,
) -> String {
    let lines = dedent_trimmed_lines(raw_code);
    lines
        .into_iter()
        .enumerate()
        .map(|(idx, l)| {
            if l.is_empty() || (idx == 0 && !indent_first_line) {
                l
            } else {
                format!("{base_indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_symbol_declaration_signature(source: &str, sym: &FreshSymbolSpan) -> String {
    let raw = sym.declaration_span.slice(source).trim();
    let mut header = raw;
    if let Some(brace_idx) = header.find('{') {
        header = &header[..brace_idx];
    }
    if let Some(arrow_idx) = header.find("=>") {
        header = &header[..arrow_idx];
    }
    if raw.starts_with("def ") || raw.starts_with("async def ") || raw.starts_with("class ") {
        if let Some(colon_nl) = header.find(":\n").or_else(|| header.find(":\r\n")) {
            header = &header[..colon_nl];
        } else if let Some(line_end) = header.find('\n') {
            header = &header[..line_end];
        }
        header = header.trim_end_matches(':');
    }
    header.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn resolve_graph_dependents(
    session: &WorkspaceSession,
    fresh: &FreshParsedFile,
    target: &FreshSymbolSpan,
    requested_symbol: &str,
) -> (Vec<String>, Vec<VerificationSurface>) {
    let file_hint = fresh.abs_path.to_string_lossy().replace('\\', "/");
    let graph_node = session
        .graph
        .resolve_first_symbol(requested_symbol, Some(&file_hint))
        .or_else(|| {
            session
                .graph
                .resolve_first_symbol(&target.qualified_name, Some(&file_hint))
        })
        .or_else(|| {
            session
                .graph
                .resolve_first_symbol(&target.name, Some(&file_hint))
        });

    let Some(node) = graph_node else {
        return (Vec::new(), Vec::new());
    };

    let callers = session
        .graph
        .callers_of(&node.id)
        .into_iter()
        .map(|c| c.name.clone())
        .collect();
    let surfaces = GraphQueryEngine::new(&session.graph).trace_verification_surfaces(&node.id, 5);
    (callers, surfaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replace_body_internal_change_writes_and_syncs_graph() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("calc.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            "pub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n\npub fn caller() -> i32 {\n    multiply(2, 3)\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let outcome = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "multiply".to_string(),
                file: file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "pub fn multiply(a: i32, b: i32) -> i32 {\n    (a * b) + 0\n}".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.status, "applied");
        assert!(!outcome.is_error);
        assert!(!outcome.signature_changed);
        let disk = std::fs::read_to_string(&file).unwrap();
        assert!(disk.contains("(a * b) + 0"));
        assert!(session.graph.resolve_first_symbol("multiply", None).is_some());
    }

    #[tokio::test]
    async fn test_replace_body_deduplicates_rust_attributes_and_python_decorators() {
        let dir = tempfile::tempdir().unwrap();
        let rs_file = dir.path().join("svc.rs");
        let py_file = dir.path().join("svc.py");

        std::fs::write(
            &rs_file,
            "#[inline]\n/// Old Rust doc\npub fn ping(x: i32) -> i32 {\n    x\n}\n",
        )
        .unwrap();
        std::fs::write(
            &py_file,
            "class Worker:\n    @staticmethod\n    def compute(x):\n        return x + 1\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        // Rust replacement WITH attributes updates existing attribute/doc without duplication
        let rs_res = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "ping".to_string(),
                file: rs_file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "#[inline]\n/// New Rust doc\npub fn ping(x: i32) -> i32 {\n    x + 1\n}".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(rs_res.status, "applied");
        let rs_disk = std::fs::read_to_string(&rs_file).unwrap();
        assert_eq!(rs_disk.matches("#[inline]").count(), 1);
        assert!(rs_disk.contains("New Rust doc"));
        assert!(!rs_disk.contains("Old Rust doc"));

        // Python replacement WITHOUT decorator preserves existing @staticmethod and preserves 4-space class indent
        let py_res = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "Worker/compute".to_string(),
                file: py_file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "def compute(x):\n    return x * 10".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(py_res.status, "applied");
        let py_disk = std::fs::read_to_string(&py_file).unwrap();
        assert_eq!(py_disk.matches("@staticmethod").count(), 1);
        assert!(py_disk.contains("    def compute(x):\n        return x * 10"));
    }

    #[tokio::test]
    async fn test_replace_body_rejects_syntax_error_without_touching_disk() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("broken.rs");
        let orig = "pub fn safe(x: i32) -> i32 {\n    x + 1\n}\n";
        std::fs::write(&file, orig).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        let outcome = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "safe".to_string(),
                file: file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "pub fn safe(x: i32) -> i32 {\n    let broken = ;\n".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.status, "syntax_error");
        assert!(outcome.is_error);
        assert!(outcome.syntax_error.is_some());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), orig);
    }

    #[tokio::test]
    async fn test_replace_body_blocks_breaking_signature_when_callers_exist_until_forced() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("api.rs");
        let orig = "pub fn charge(amount: i32) -> i32 {\n    amount * 2\n}\n\npub fn checkout() -> i32 {\n    charge(50)\n}\n";
        std::fs::write(&file, orig).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let blocked = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "charge".to_string(),
                file: file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "pub fn charge(amount: i32, currency: &str) -> i32 {\n    let _ = currency;\n    amount * 2\n}".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(blocked.status, "blocked_requires_force");
        assert!(blocked.is_error);
        assert!(blocked.signature_changed);
        assert_eq!(blocked.fan_in, 1);
        assert!(blocked.affected_callers.contains(&"checkout".to_string()));
        assert!(!blocked.unified_diff.is_empty());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), orig);

        let forced = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "charge".to_string(),
                file: file.clone(),
                operation: EditOperation::ReplaceBody,
                new_code: "pub fn charge(amount: i32, currency: &str) -> i32 {\n    let _ = currency;\n    amount * 2\n}".to_string(),
                dry_run: false,
                force: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(forced.status, "applied");
        assert!(!forced.is_error);
        assert!(std::fs::read_to_string(&file).unwrap().contains("currency: &str"));
    }

    #[tokio::test]
    async fn test_insert_after_and_insert_before_preserve_indentation() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("service.py");
        std::fs::write(
            &file,
            "class Service:\n    def target(self):\n        return 1\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let after_res = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "Service/target".to_string(),
                file: file.clone(),
                operation: EditOperation::InsertAfter,
                new_code: "def helper_after(self):\n    return 2".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(after_res.status, "applied");

        let before_res = execute_edit_symbol(
            &mut session,
            EditSymbolRequest {
                symbol: "Service/target".to_string(),
                file: file.clone(),
                operation: EditOperation::InsertBefore,
                new_code: "def helper_before(self):\n    return 0".to_string(),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(before_res.status, "applied");

        let disk = std::fs::read_to_string(&file).unwrap();
        assert!(disk.contains("    def helper_before(self):\n        return 0\n    def target(self):"));
        assert!(disk.contains("    def target(self):\n        return 1\n\n    def helper_after(self):\n        return 2"));
    }
}
