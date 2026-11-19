//! # AtomicFileWriter — Line-Ending/BOM Preserving Transactional Disk Writer
//!
//! Provides safe, single-file and multi-file atomic writes for CHRONOS symbolic editing
//! and refactoring (`v1.8.0`):
//! - Preserves original UTF-8 BOM (`0xEF 0xBB 0xBF`) and line endings (`CRLF` vs `LF`)
//! - Writes via temporary file in the target file's directory + `sync_all` + atomic `persist`
//! - Multi-file transactions keep an in-memory rollback snapshot of original bytes and
//!   restore every previously modified file if any step fails.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Dominant line-ending convention of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
}

/// Encoding and line-ending metadata captured from disk before editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FileEncodingMetadata {
    pub line_ending: LineEnding,
    pub has_utf8_bom: bool,
}

impl FileEncodingMetadata {
    /// Detect UTF-8 BOM and CRLF/LF convention from raw disk bytes.
    pub fn detect(raw_bytes: &[u8]) -> Self {
        let has_utf8_bom = raw_bytes.starts_with(&UTF8_BOM);
        let body = if has_utf8_bom {
            &raw_bytes[UTF8_BOM.len()..]
        } else {
            raw_bytes
        };
        let line_ending = if body.windows(2).any(|w| w == b"\r\n") {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        };
        Self {
            line_ending,
            has_utf8_bom,
        }
    }

    /// Detect metadata from an existing file on disk, defaulting to `Lf` + no BOM if absent.
    pub fn detect_from_path(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => Self::detect(&bytes),
            Err(_) => Self::default(),
        }
    }

    /// Encode UTF-8 string content preserving original BOM and CRLF/LF line endings
    /// without duplicating `\r` or BOM bytes.
    pub fn encode_content(&self, content: &str) -> Vec<u8> {
        let without_bom = content.trim_start_matches('\u{FEFF}');
        let normalized_lf = without_bom.replace("\r\n", "\n");
        let body = match self.line_ending {
            LineEnding::Lf => normalized_lf,
            LineEnding::Crlf => normalized_lf.replace('\n', "\r\n"),
        };

        let mut out = Vec::with_capacity(
            body.len() + if self.has_utf8_bom { UTF8_BOM.len() } else { 0 },
        );
        if self.has_utf8_bom {
            out.extend_from_slice(&UTF8_BOM);
        }
        out.extend_from_slice(body.as_bytes());
        out
    }
}

/// Transactional atomic disk writer preserving original file encoding and line endings.
pub struct AtomicFileWriter;

impl AtomicFileWriter {
    /// Read a UTF-8 file from disk alongside its `FileEncodingMetadata`, stripping any leading BOM
    /// from the returned `String` so Tree-sitter and `TextEdit` see clean UTF-8 byte offsets.
    pub fn read_utf8_preserving_metadata(path: &Path) -> Result<(String, FileEncodingMetadata)> {
        let raw_bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read file bytes: {}", path.display()))?;
        let meta = FileEncodingMetadata::detect(&raw_bytes);
        let body_bytes = if meta.has_utf8_bom {
            &raw_bytes[UTF8_BOM.len()..]
        } else {
            &raw_bytes[..]
        };
        let text = String::from_utf8(body_bytes.to_vec())
            .with_context(|| format!("File is not valid UTF-8: {}", path.display()))?;
        Ok((text, meta))
    }

    /// Atomically write `content` to `path`, auto-detecting existing BOM & CRLF/LF metadata on disk.
    pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
        let meta = FileEncodingMetadata::detect_from_path(path);
        let encoded = meta.encode_content(content);
        Self::write_raw_atomic(path, &encoded)
    }

    /// Atomically write `content` using explicit `FileEncodingMetadata`.
    pub fn write_atomic_with_metadata(
        path: &Path,
        content: &str,
        meta: FileEncodingMetadata,
    ) -> Result<()> {
        let encoded = meta.encode_content(content);
        Self::write_raw_atomic(path, &encoded)
    }

    /// Write raw bytes via a temp file in the same directory + `sync_all` + atomic `persist`.
    pub fn write_raw_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.exists() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        let mut tmp = NamedTempFile::new_in(parent).with_context(|| {
            format!("Failed to create temporary file in {}", parent.display())
        })?;
        tmp.write_all(bytes)
            .with_context(|| format!("Failed writing temporary bytes for {}", path.display()))?;
        tmp.flush()
            .with_context(|| format!("Failed flushing temporary bytes for {}", path.display()))?;
        tmp.as_file()
            .sync_all()
            .with_context(|| format!("Failed syncing temporary file for {}", path.display()))?;

        tmp.persist(path).map_err(|e| {
            anyhow::anyhow!(
                "Failed atomic rename over '{}': {}",
                path.display(),
                e.error
            )
        })?;
        Ok(())
    }

    /// Execute an all-or-nothing multi-file atomic write transaction.
    /// Rejects duplicate target paths and rolls back every previously modified file
    /// from in-memory backups if any step fails.
    pub fn commit_transaction(writes: &[(PathBuf, String)]) -> Result<()> {
        Self::commit_transaction_with_hook(writes, |_| Ok(()))
    }

    /// Transaction core accepting a per-file post-write validation hook (used both internally
    /// and in rollback fault-injection unit tests).
    pub fn commit_transaction_with_hook<F>(
        writes: &[(PathBuf, String)],
        mut after_write_hook: F,
    ) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
    {
        if writes.is_empty() {
            return Ok(());
        }

        let mut seen = HashSet::new();
        let mut backups: Vec<(PathBuf, Option<Vec<u8>>, FileEncodingMetadata)> =
            Vec::with_capacity(writes.len());

        for (path, _) in writes {
            let norm = path.to_string_lossy().replace('\\', "/");
            if !seen.insert(norm) {
                bail!(
                    "Duplicate target path in atomic transaction: {}",
                    path.display()
                );
            }
            let backup = if path.exists() {
                Some(std::fs::read(path).with_context(|| {
                    format!("Failed to snapshot original file {}", path.display())
                })?)
            } else {
                None
            };
            let meta = backup
                .as_deref()
                .map(FileEncodingMetadata::detect)
                .unwrap_or_default();
            backups.push((path.clone(), backup, meta));
        }

        let mut applied: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::new();
        for ((path, content), (_, backup, meta)) in writes.iter().zip(backups.into_iter()) {
            let encoded = meta.encode_content(content);
            let step_res = Self::write_raw_atomic(path, &encoded)
                .and_then(|_| after_write_hook(path));

            if let Err(err) = step_res {
                // Roll back the failed file if write succeeded before hook failed, plus all prior files
                if after_write_hook_may_have_left_file(path, &backup) {
                    applied.push((path.clone(), backup));
                }
                let rollback_errors = Self::rollback(&applied);
                if rollback_errors.is_empty() {
                    return Err(err).with_context(|| {
                        format!(
                            "Atomic transaction aborted at '{}' (all modified files rolled back)",
                            path.display()
                        )
                    });
                }
                bail!(
                    "Atomic transaction failed at '{}' ({}); rollback errors: {}",
                    path.display(),
                    err,
                    rollback_errors.join("; ")
                );
            }

            applied.push((path.clone(), backup));
        }

        Ok(())
    }

    fn rollback(applied: &[(PathBuf, Option<Vec<u8>>)]) -> Vec<String> {
        let mut errors = Vec::new();
        for (path, original) in applied.iter().rev() {
            match original {
                Some(bytes) => {
                    if let Err(e) = Self::write_raw_atomic(path, bytes) {
                        errors.push(format!("{}: {}", path.display(), e));
                    }
                }
                None => {
                    if path.exists() {
                        if let Err(e) = std::fs::remove_file(path) {
                            errors.push(format!("{}: {}", path.display(), e));
                        }
                    }
                }
            }
        }
        errors
    }
}

fn after_write_hook_may_have_left_file(path: &Path, backup: &Option<Vec<u8>>) -> bool {
    // Only include current path in rollback list if it already exists or had a backup.
    path.exists() || backup.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_writer_preserves_crlf_and_bom_and_rolls_back_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("alpha.rs");
        let file_b = dir.path().join("beta.rs");

        // File A has UTF-8 BOM + CRLF line endings
        let mut raw_a = UTF8_BOM.to_vec();
        raw_a.extend_from_slice(b"fn alpha() {\r\n    let x = 1;\r\n}\r\n");
        std::fs::write(&file_a, &raw_a).unwrap();

        // File B has plain LF and no BOM
        let raw_b = b"fn beta() {\n    let y = 2;\n}\n";
        std::fs::write(&file_b, raw_b).unwrap();

        // 1. Atomic write preserving BOM + CRLF even when caller passes LF string
        AtomicFileWriter::write_atomic(&file_a, "fn alpha() {\n    let x = 99;\n}\n").unwrap();
        let disk_a = std::fs::read(&file_a).unwrap();
        assert!(disk_a.starts_with(&UTF8_BOM), "UTF-8 BOM must be preserved");
        assert!(
            disk_a.windows(2).any(|w| w == b"\r\n"),
            "CRLF line endings must be preserved"
        );
        assert!(
            !disk_a.windows(3).any(|w| w == b"\r\r\n"),
            "Must never produce double CR"
        );
        let expected_a_snapshot = disk_a.clone();

        // 2. Transactional commit that fails on the second file rolls back BOTH files cleanly
        let tx = vec![
            (file_a.clone(), "fn alpha() {\n    let x = 123;\n}\n".to_string()),
            (file_b.clone(), "fn beta() {\n    let y = 456;\n}\n".to_string()),
        ];
        let err = AtomicFileWriter::commit_transaction_with_hook(&tx, |p| {
            if p == file_b {
                anyhow::bail!("simulated failure on second file");
            }
            Ok(())
        });
        assert!(err.is_err());

        // Verify File A reverted to exact BOM + CRLF bytes and File B reverted to LF bytes
        assert_eq!(std::fs::read(&file_a).unwrap(), expected_a_snapshot);
        assert_eq!(std::fs::read(&file_b).unwrap(), raw_b);
    }
}
