//! # Mutation Sandbox — Tempdir Isolation for Parallel Execution
//!
//! Creates an isolated copy of the project in a temporary directory
//! so multiple mutants can run their test suites concurrently without
//! clobbering each other's source files.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// An isolated sandbox directory containing one mutated version of a file.
/// Automatically cleaned up when dropped.
pub struct MutantSandbox {
    /// Temporary directory (auto-deleted on drop)
    _dir: TempDir,
    /// Path to the mutated source file inside the sandbox
    pub source_file: PathBuf,
    /// Working directory for test execution (sandbox root)
    pub working_dir: PathBuf,
    /// Test file paths mapped into the sandbox
    pub test_files: Vec<String>,
}

impl MutantSandbox {
    /// Create a new sandbox for a single mutant.
    ///
    /// Strategy: full copy of `working_dir` into a tempdir, then
    /// overwrite the target source file with the mutated content.
    pub fn create(
        mutated_content: &str,
        original_source: &Path,
        working_dir: &Path,
        test_files: &[&str],
    ) -> Result<Self> {
        let dir = tempfile::tempdir()
            .context("Failed to create temp sandbox directory")?;

        // Full copy of the working directory into sandbox
        copy_dir_all(working_dir, dir.path())
            .context("Failed to copy project into sandbox")?;

        // Compute relative path of the source file within working_dir
        let relative = original_source
            .strip_prefix(working_dir)
            .with_context(|| format!(
                "Source file {:?} is not inside working_dir {:?}",
                original_source, working_dir
            ))?;

        // Write mutated content to the sandboxed copy
        let sandbox_source = dir.path().join(relative);
        if let Some(parent) = sandbox_source.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&sandbox_source, mutated_content)
            .context("Failed to write mutated source into sandbox")?;

        // Remap test file paths into the sandbox
        let sandbox_tests: Vec<String> = test_files
            .iter()
            .map(|t| {
                let tp = Path::new(t);
                if let Ok(rel) = tp.strip_prefix(working_dir) {
                    dir.path().join(rel).to_string_lossy().to_string()
                } else {
                    // Test path not relative to working_dir — use as-is
                    t.to_string()
                }
            })
            .collect();

        Ok(Self {
            working_dir: dir.path().to_path_buf(),
            source_file: sandbox_source,
            test_files: sandbox_tests,
            _dir: dir,
        })
    }
}

/// Recursively copy a directory tree.
/// Skips `node_modules`, `.git`, and `target/` directories to keep copies fast.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip heavy directories that don't need copying
        if matches!(name_str.as_ref(), "node_modules" | ".git" | "target" | ".next" | "dist" | "build") {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| format!("Failed to copy {:?} → {:?}", src_path, dst_path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creates_and_cleans_up() {
        let working_dir = std::env::temp_dir().join("chronos_sandbox_test_wd");
        std::fs::create_dir_all(&working_dir).unwrap();
        let src_file = working_dir.join("test_file.ts");
        std::fs::write(&src_file, "const x = 1;").unwrap();

        let sandbox = MutantSandbox::create(
            "const x = 0; // mutated",
            &src_file,
            &working_dir,
            &[],
        ).unwrap();

        // Sandbox file should contain mutated content
        let content = std::fs::read_to_string(&sandbox.source_file).unwrap();
        assert_eq!(content, "const x = 0; // mutated");

        // Working dir is inside a tempdir
        assert!(sandbox.working_dir.exists());

        // Cleanup
        std::fs::remove_dir_all(&working_dir).ok();
    }
}
