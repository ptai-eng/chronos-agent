//! # Git Diff Blast-Radius
//!
//! Runs `git diff` to discover changed symbols in the current workspace,
//! then runs targeted blast-radius analysis only on those symbols.
//!
//! Usage:
//!   `chronos audit --diff`           — unstaged changes
//!   `chronos audit --diff --staged`  — staged (pre-commit)
//!   `chronos audit --diff --commit <sha>` — specific commit

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A symbol that changed in the git diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedSymbol {
    pub file: PathBuf,
    pub symbol_name: String,
    pub start_line: usize,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// Result of a diff blast-radius analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct DiffBlastRadiusReport {
    pub changed_symbols: Vec<ChangedSymbol>,
    pub symbol_reports: Vec<SymbolImpact>,
    pub overall_risk: String,
    pub exit_code_for_ci: i32, // 0 = safe, 1 = high risk detected
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolImpact {
    pub symbol: String,
    pub file: String,
    pub risk_level: String,     // "LOW" | "AMBER" | "HIGH" | "CRITICAL"
    pub direct_callers: usize,
    pub transitive_callers: usize,
    pub recommended_test_modules: Vec<String>,
}

/// Get changed symbols from git diff
pub fn get_changed_symbols(
    working_dir: &Path,
    diff_mode: DiffMode,
) -> Result<Vec<ChangedSymbol>> {
    let git_args = diff_mode.to_git_args();
    let output = Command::new("git")
        .args(&git_args)
        .current_dir(working_dir)
        .output()
        .context("Failed to run git diff — is git installed and this a git repo?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {}", stderr);
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    parse_unified_diff(&diff_text, working_dir)
}

/// How to run git diff
#[derive(Debug, Clone)]
pub enum DiffMode {
    /// `git diff HEAD` — unstaged changes
    Unstaged,
    /// `git diff --cached HEAD` — staged changes (pre-commit hook)
    Staged,
    /// `git diff <sha>` — specific commit
    Commit(String),
    /// `git diff <base>..<head>` — PR range
    Range(String, String),
}

impl DiffMode {
    fn to_git_args(&self) -> Vec<String> {
        match self {
            DiffMode::Unstaged => vec![
                "diff".to_string(), "HEAD".to_string(), "--unified=3".to_string(),
            ],
            DiffMode::Staged => vec![
                "diff".to_string(), "--cached".to_string(), "HEAD".to_string(), "--unified=3".to_string(),
            ],
            DiffMode::Commit(sha) => vec![
                "diff".to_string(), format!("{}^..{}", sha, sha), "--unified=3".to_string(),
            ],
            DiffMode::Range(base, head) => vec![
                "diff".to_string(), format!("{}..{}", base, head), "--unified=3".to_string(),
            ],
        }
    }
}

/// Parse unified diff output to extract changed files + line ranges
fn parse_unified_diff(diff: &str, working_dir: &Path) -> Result<Vec<ChangedSymbol>> {
    let mut symbols: Vec<ChangedSymbol> = Vec::new();
    let mut current_file: Option<PathBuf> = None;
    let mut current_line: usize = 0;

    for line in diff.lines() {
        if line.starts_with("+++ b/") {
            // New file being diffed
            let rel_path = line.trim_start_matches("+++ b/");
            current_file = Some(working_dir.join(rel_path));
        } else if line.starts_with("@@ ") {
            // @@ -old_start,old_count +new_start,new_count @@
            if let Some(new_range) = parse_hunk_header(line) {
                current_line = new_range;
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            // Added line — check if it's a function/class definition
            let content = &line[1..];
            if let Some(sym) = extract_symbol_from_line(content, current_line) {
                if let Some(ref file) = current_file {
                    symbols.push(ChangedSymbol {
                        file: file.clone(),
                        symbol_name: sym,
                        start_line: current_line,
                        change_type: ChangeType::Modified,
                    });
                }
            }
            current_line += 1;
        } else if !line.starts_with('-') {
            current_line += 1;
        }
    }

    Ok(symbols)
}

fn parse_hunk_header(line: &str) -> Option<usize> {
    // "@@ -1,5 +10,20 @@" → extract 10 (new file start line)
    let plus_part = line.split('+').nth(1)?;
    let num_str = plus_part.split(',').next()?.split(' ').next()?;
    num_str.parse::<usize>().ok()
}

fn extract_symbol_from_line(line: &str, _line_num: usize) -> Option<String> {
    let trimmed = line.trim();

    // TypeScript / JavaScript: function declarations, arrow functions, methods
    if trimmed.starts_with("function ") || trimmed.starts_with("async function ") {
        let name = trimmed
            .trim_start_matches("async ")
            .trim_start_matches("function ")
            .split('(').next()?
            .trim()
            .to_string();
        if !name.is_empty() { return Some(name); }
    }

    // TypeScript: export const/let fn = () =>
    if (trimmed.starts_with("export const ") || trimmed.starts_with("const "))
        && trimmed.contains("= (") || (trimmed.contains("=>") && trimmed.contains("("))
    {
        let name = trimmed
            .trim_start_matches("export ")
            .trim_start_matches("const ")
            .trim_start_matches("let ")
            .split(' ').next()?
            .split('=').next()?
            .trim()
            .to_string();
        if !name.is_empty() && name.chars().next()?.is_alphabetic() {
            return Some(name);
        }
    }

    // Python: def function_name(
    if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
        let name = trimmed
            .trim_start_matches("async ")
            .trim_start_matches("def ")
            .split('(').next()?
            .trim()
            .to_string();
        if !name.is_empty() { return Some(name); }
    }

    // Rust: pub fn / fn / pub async fn
    if trimmed.contains(" fn ") {
        let after_fn = trimmed.split(" fn ").nth(1)?;
        let name = after_fn.split('(').next()?.split('<').next()?.trim().to_string();
        if !name.is_empty() { return Some(name); }
    }

    None
}

/// Format the report as human-readable Markdown (for terminal / PR comments)
pub fn format_markdown(report: &DiffBlastRadiusReport) -> String {
    let mut out = String::new();
    out.push_str("## 🔀 CHRONOS: Git Diff Blast-Radius\n\n");

    if report.changed_symbols.is_empty() {
        out.push_str("✅ No trackable function changes detected in diff.\n");
        return out;
    }

    out.push_str(&format!("**Changed symbols:** {}\n\n", report.changed_symbols.len()));
    out.push_str("| Symbol | File | Risk | Callers (direct/transitive) |\n");
    out.push_str("|--------|------|------|-----------------------------|\n");

    for r in &report.symbol_reports {
        let risk_icon = match r.risk_level.as_str() {
            "CRITICAL" => "🔴",
            "HIGH"     => "🟠",
            "AMBER"    => "🟡",
            _          => "🟢",
        };
        out.push_str(&format!(
            "| `{}` | `{}` | {} {} | {}/{} |\n",
            r.symbol,
            Path::new(&r.file).file_name()
                .and_then(|n| n.to_str()).unwrap_or(&r.file),
            risk_icon,
            r.risk_level,
            r.direct_callers,
            r.transitive_callers,
        ));
    }

    out.push('\n');
    out.push_str(&format!("**Overall Risk:** {}\n", report.overall_risk));

    if report.exit_code_for_ci != 0 {
        out.push_str("\n⚠️ **CI Gate: BLOCKED** — High-risk changes detected. Review before merge.\n");
    } else {
        out.push_str("\n✅ **CI Gate: PASSED** — All changes are low-risk.\n");
    }

    out
}

/// Format the report as JSON (for CI script consumption)
pub fn format_json(report: &DiffBlastRadiusReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -1,5 +10,20 @@ fn main() {"), Some(10));
        assert_eq!(parse_hunk_header("@@ -0,0 +1 @@"), Some(1));
    }

    #[test]
    fn test_extract_ts_function() {
        assert_eq!(
            extract_symbol_from_line("function processOrder(id: string) {", 10),
            Some("processOrder".to_string())
        );
        assert_eq!(
            extract_symbol_from_line("async function sendEmail(to: string) {", 20),
            Some("sendEmail".to_string())
        );
    }

    #[test]
    fn test_extract_python_function() {
        assert_eq!(
            extract_symbol_from_line("def validate_user(user_id: int):", 5),
            Some("validate_user".to_string())
        );
        assert_eq!(
            extract_symbol_from_line("async def create_order(data: dict):", 8),
            Some("create_order".to_string())
        );
    }

    #[test]
    fn test_extract_rust_function() {
        assert_eq!(
            extract_symbol_from_line("pub fn process_order(id: u64) -> bool {", 3),
            Some("process_order".to_string())
        );
        assert_eq!(
            extract_symbol_from_line("pub async fn get_user(&self, id: u64) -> Option<User> {", 7),
            Some("get_user".to_string())
        );
    }
}
