//! `chronos status` — Contract broker status, drain pending alerts

use anyhow::Result;
use chronos_core::contract::broker::ContractBroker;
use std::path::PathBuf;

pub async fn run(workspace: PathBuf, watch_dir: Option<PathBuf>) -> Result<()> {
    println!("🔒 CHRONOS Contract Broker Status\n");

    let broker = ContractBroker::new();

    // If watch_dir provided, index that directory
    if let Some(dir) = watch_dir {
        let abs_dir = if dir.is_absolute() { dir } else { workspace.join(dir) };
        println!("👁️  Indexing: {}", abs_dir.display());

        // Index all .ts files in the directory
        let mut indexed = 0;
        if let Ok(entries) = std::fs::read_dir(&abs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "ts").unwrap_or(false) {
                    if let Err(e) = broker.index_file(&path).await {
                        eprintln!("   ⚠️  {}: {}", path.display(), e);
                    } else {
                        indexed += 1;
                    }
                }
            }
        }
        println!("   ✓ Indexed {} TypeScript files\n", indexed);
    }

    // Drain pending alerts
    match broker.format_pending_alerts_for_agent().await {
        Some(alerts) => {
            println!("⚠️  PENDING CONTRACT ALERTS:\n");
            println!("{}", alerts);
            println!("\n─────────────────────────────────────────────────────────");
            println!("Action required: Update all callers of changed functions.");
        }
        None => {
            println!("✅ All Clear — No pending contract violation alerts.");
            println!("\nThe Pre-Tool Execution Gate is clear.");
            println!("Proceed with your coding task safely.");
        }
    }

    Ok(())
}
