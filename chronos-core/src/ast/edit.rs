//! # TextEdit Primitive — Shared AST & Mutation Editing Value Type
//!
//! Provides byte-span `TextEdit` operations shared across:
//! - Pillar 2 (`MutationArena` / Super-Mutant source synthesis)
//! - v1.8 Symbolic AST Editing & Atomic Refactoring (`chronos_edit_symbol`, `chronos_refactor_symbol`)

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    sorted.sort_by(|a, b| b.start_byte.cmp(&a.start_byte).then_with(|| b.end_byte.cmp(&a.end_byte)));

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
}
