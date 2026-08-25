//! # Mutation Runner — Parallel Evaluation Engine
//!
//! Runs all mutants against the selected tests in parallel,
//! collects kill/survive results, and computes the Mutation Score.

use crate::mutation::operators::Mutant;
use crate::mutation::watchdog::{run_mutant_test, WatchdogConfig};
use anyhow::Result;
use std::path::Path;
use tracing::{debug, info};

/// Run a batch of mutants against their selected tests.
///
/// Uses async sequential execution to prevent file locking race conditions.
pub async fn run_mutants_sequential(
    mutants: &mut Vec<Mutant>,
    source_file: &Path,
    test_files: &[&str],
    config: &WatchdogConfig,
    working_dir: &Path,
) -> Result<()> {
    let total = mutants.len();
    info!("Running {} mutants with timeout {}ms each", total, config.timeout_ms);

    // Run sequentially with async (true parallel would require Arc<Mutex> around file writes)
    // Phase 5+ can parallelize by copying the project to temp directories
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
        // else: mutant survived
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_module_exists() {
        // Sanity check that module compiles
        let config = WatchdogConfig::default();
        assert_eq!(config.timeout_ms, 5000);
    }
}
