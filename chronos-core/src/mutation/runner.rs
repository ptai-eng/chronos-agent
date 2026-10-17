//! # Mutation Runner — Parallel Evaluation Engine
//!
//! Runs all mutants against the selected tests in parallel using isolated
//! sandbox directories (one tempdir per mutant), then collects kill/survive
//! results and computes the Mutation Score.

use crate::mutation::operators::{build_super_mutant_source, Mutant};
use crate::mutation::sandbox::MutantSandbox;
use crate::mutation::watchdog::{run_mutant_test, run_super_mutant_id, WatchdogConfig};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Run a batch of mutants using a single combined "Super-Mutant" sandbox (`CHRONOS_MUTANT_ID`).
///
/// Only ONE sandbox directory and ONE compilation pass is required; all non-overlapping
/// line mutants are evaluated concurrently against the shared compiled artifact by passing
/// `CHRONOS_MUTANT_ID=<mutant.id>` per test process.
pub async fn run_super_mutant_parallel(
    mutants: &mut Vec<Mutant>,
    source_file: &Path,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<()> {
    if mutants.is_empty() {
        return Ok(());
    }

    let original_source = std::fs::read_to_string(source_file)?;
    let ext = source_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("ts");
    let super_source = build_super_mutant_source(&original_source, mutants, ext);

    let sandbox = Arc::new(MutantSandbox::create(
        &super_source,
        source_file,
        working_dir,
        test_files,
    )?);

    let sem = Arc::new(Semaphore::new(config.max_concurrency));
    let config = Arc::new(config.clone());

    let handles: Vec<_> = mutants
        .iter()
        .map(|mutant| {
            let mutant_id = mutant.id;
            let sem = sem.clone();
            let config = config.clone();
            let sandbox = sandbox.clone();

            tokio::spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                let sandbox_test_refs: Vec<&str> =
                    sandbox.test_files.iter().map(|s| s.as_str()).collect();
                match run_super_mutant_id(
                    mutant_id,
                    &sandbox_test_refs,
                    &config,
                    &sandbox.working_dir,
                )
                .await
                {
                    Ok(r) => (mutant_id, r.killed, r.timed_out, r.exit_code),
                    Err(e) => {
                        warn!("Super-mutant {} test error: {}", mutant_id, e);
                        (mutant_id, true, false, None)
                    }
                }
            })
        })
        .collect();

    for (mutant, handle) in mutants.iter_mut().zip(handles) {
        match handle.await {
            Ok((_, killed, timed_out, exit_code)) => {
                if timed_out {
                    mutant.timeout();
                } else if killed {
                    mutant.kill(format!("exit_code={}", exit_code.unwrap_or(-1)));
                }
            }
            Err(e) => {
                warn!("Task join error for super-mutant: {}", e);
                mutant.kill("task_join_error".to_string());
            }
        }
    }

    Ok(())
}

/// Run a batch of mutants in parallel using sandboxed tempdir isolation.
///
/// Each mutant gets its own copy of the project in a temporary directory,
/// so tests can run concurrently without file-lock conflicts.
/// Concurrency is bounded by `config.max_concurrency` to prevent OOM.
pub async fn run_mutants_parallel(
    mutants: &mut Vec<Mutant>,
    source_file: &Path,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<()> {
    let total = mutants.len();
    info!(
        "Running {} mutants in parallel (max_concurrency={})",
        total, config.max_concurrency
    );

    // Semaphore limits how many sandboxes run simultaneously
    let sem = Arc::new(Semaphore::new(config.max_concurrency));
    let config = Arc::new(config.clone());
    let working_dir = working_dir.to_path_buf();
    let source_file = source_file.to_path_buf();
    let test_files_owned: Vec<String> = test_files.iter().map(|s| s.to_string()).collect();

    // Spawn one task per mutant
    let handles: Vec<_> = mutants
        .iter()
        .map(|mutant| {
            let mutant_source = mutant.mutated_source.clone();
            let mutant_id = mutant.id;
            let sem = sem.clone();
            let config = config.clone();
            let working_dir = working_dir.clone();
            let source_file = source_file.clone();
            let test_files_owned = test_files_owned.clone();

            tokio::spawn(async move {
                // Acquire semaphore permit before creating sandbox
                let _permit = sem.acquire().await.expect("semaphore closed");

                debug!("Spawning sandbox for mutant {}", mutant_id);

                // Create isolated tempdir sandbox
                let test_refs: Vec<&str> = test_files_owned.iter().map(|s| s.as_str()).collect();
                let sandbox = match MutantSandbox::create(
                    &mutant_source,
                    &source_file,
                    &working_dir,
                    &test_refs,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Failed to create sandbox for mutant {}: {}", mutant_id, e);
                        return (mutant_id, false, false, Some(-1i32));
                    }
                };

                let sandbox_test_refs: Vec<&str> =
                    sandbox.test_files.iter().map(|s| s.as_str()).collect();

                let result = run_mutant_test(
                    &mutant_source,
                    &sandbox.source_file,
                    &sandbox_test_refs,
                    &config,
                    &sandbox.working_dir,
                )
                .await;

                // sandbox drops at end of scope → tempdir auto-deleted

                match result {
                    Ok(r) => (mutant_id, r.killed, r.timed_out, r.exit_code),
                    Err(e) => {
                        warn!("Mutant {} test error: {}", mutant_id, e);
                        (mutant_id, true, false, None)
                    }
                }
            })
        })
        .collect();

    // Collect results and update mutant statuses
    for (mutant, handle) in mutants.iter_mut().zip(handles) {
        match handle.await {
            Ok((_, killed, timed_out, exit_code)) => {
                if timed_out {
                    mutant.timeout();
                } else if killed {
                    mutant.kill(format!("exit_code={}", exit_code.unwrap_or(-1)));
                }
                // else: mutant survived
            }
            Err(e) => {
                warn!("Task join error for mutant: {}", e);
                mutant.kill("task_join_error".to_string());
            }
        }
    }

    Ok(())
}

/// Fallback: sequential runner (kept for compatibility & debugging)
pub async fn run_mutants_sequential(
    mutants: &mut Vec<Mutant>,
    source_file: &Path,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<()> {
    let total = mutants.len();
    info!("Running {} mutants sequentially (debug mode)", total);

    for (i, mutant) in mutants.iter_mut().enumerate() {
        debug!("Testing mutant {}/{}: {:?}", i + 1, total, mutant.kind.description());

        let mut result = run_mutant_test(
            &mutant.mutated_source,
            source_file,
            test_files,
            config,
            working_dir,
        ).await?;

        result.mutant_id = mutant.id;

        if result.timed_out {
            mutant.timeout();
        } else if result.killed {
            mutant.kill(format!("exit_code={}", result.exit_code.unwrap_or(-1)));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_module_exists() {
        let config = WatchdogConfig::default();
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_concurrency, 4);
    }
}
