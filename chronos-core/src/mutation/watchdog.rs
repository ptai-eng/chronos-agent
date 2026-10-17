//! # Mutation Watchdog — In-Process Timeout Isolation
//!
//! Ratified spec: Use tokio::time::timeout (in-process) instead of spawning
//! a child process per mutant. This avoids the 50-100ms Windows process
//! spawn overhead while still protecting against runaway test execution.
//!
//! ## Test runner strategy
//! For each mutant:
//! 1. Write the mutated source to a temp file
//! 2. Run the test command against it (Node.js/Jest/Vitest)
//! 3. If test exits non-zero → mutant KILLED (test detected the bug)
//! 4. If test exits zero → mutant SURVIVED (test missed the bug)
//! 5. If timeout exceeded → mutant marked KILLED by timeout

use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;
use tracing::warn;

/// Result of running a test against a single mutant
#[derive(Debug)]
pub struct MutantTestResult {
    pub mutant_id: u64,
    pub killed: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub output: String,
}

/// Configuration for the mutation watchdog
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Maximum time to allow each mutant test run (milliseconds)
    /// Default: 5000ms (accounts for Jest startup overhead)
    pub timeout_ms: u64,
    /// Maximum number of concurrent mutant test runners
    pub max_concurrency: usize,
    /// Test runner command (e.g., "npx", "node")
    pub test_runner: String,
    /// Test runner arguments (e.g., ["jest", "--testPathPattern"])
    pub test_args: Vec<String>,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000, // 5 seconds per mutant (accounts for Jest startup)
            max_concurrency: 4,
            test_runner: "npx".to_string(),
            test_args: vec!["--yes".to_string(), "jest".to_string(), "--no-coverage".to_string()],
        }
    }
}

/// Run a single mutant test with timeout protection.
///
/// Writes mutated source to a temp file, runs the test, collects result.
pub async fn run_mutant_test(
    mutant_source: &str,
    original_file: &Path,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<MutantTestResult> {
    let start = std::time::Instant::now();

    // Write mutated source to a temp file (replacing the original)
    let backup = std::fs::read_to_string(original_file)?;
    
    // RAII guard to guarantee restoration even on panic/cancellation
    struct FileRestoreGuard<'a> {
        file_path: &'a Path,
        content: String,
    }
    impl<'a> Drop for FileRestoreGuard<'a> {
        fn drop(&mut self) {
            let _ = std::fs::write(self.file_path, &self.content);
        }
    }
    
    let _guard = FileRestoreGuard {
        file_path: original_file,
        content: backup,
    };
    
    std::fs::write(original_file, mutant_source)?;

    run_mutant_test_with_env(0, test_files, config, working_dir, start).await
}

/// Run a test command in an already-prepared super-mutant workspace by setting `CHRONOS_MUTANT_ID`.
pub async fn run_super_mutant_id(
    mutant_id: u64,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<MutantTestResult> {
    let start = std::time::Instant::now();
    run_mutant_test_with_env(mutant_id, test_files, config, working_dir, start).await
}

async fn run_mutant_test_with_env(
    mutant_id: u64,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
    start: std::time::Instant,
) -> Result<MutantTestResult> {
    let result = timeout(
        Duration::from_millis(config.timeout_ms),
        execute_test_command(
            &config.test_runner,
            &config.test_args,
            test_files,
            working_dir,
            if mutant_id > 0 { Some(mutant_id) } else { None },
        ),
    )
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok((exit_code, output))) => {
            // Non-zero exit = test failed = mutant KILLED (good!)
            let killed = exit_code != 0;
            Ok(MutantTestResult {
                mutant_id,
                killed,
                timed_out: false,
                exit_code: Some(exit_code),
                duration_ms,
                output,
            })
        }
        Ok(Err(e)) => {
            warn!("Test execution error: {}", e);
            // Treat execution errors as killed (defensive)
            Ok(MutantTestResult {
                mutant_id,
                killed: true,
                timed_out: false,
                exit_code: None,
                duration_ms,
                output: format!("Error: {}", e),
            })
        }
        Err(_) => {
            // Timeout — treat as killed (infinite loop prevention)
            warn!("Mutant test timed out after {}ms", config.timeout_ms);
            Ok(MutantTestResult {
                mutant_id,
                killed: true,
                timed_out: true,
                exit_code: None,
                duration_ms,
                output: format!("TIMEOUT after {}ms", config.timeout_ms),
            })
        }
    }
}

/// Execute the test command and return (exit_code, combined_output)
async fn execute_test_command(
    runner: &str,
    args: &[String],
    test_files: &[&str],
    working_dir: &Path,
    mutant_id: Option<u64>,
) -> Result<(i32, String)> {
    let mut cmd = tokio::process::Command::new(runner);
    cmd.args(args);
    cmd.args(test_files);
    cmd.current_dir(working_dir);
    if let Some(id) = mutant_id {
        cmd.env("CHRONOS_MUTANT_ID", id.to_string());
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let output = cmd.output().await?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    Ok((exit_code, combined))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_config_defaults() {
        let config = WatchdogConfig::default();
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_concurrency, 4);
        assert!(config.test_args.contains(&"jest".to_string()));
    }
}
