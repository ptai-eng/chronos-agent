//! `chronos status` — Contract broker status, drain pending alerts

use anyhow::Result;
use chronos_core::{
    contract::ContractEngine,
    session::collect_supported_files,
};
use std::path::PathBuf;

pub async fn run(workspace: PathBuf, watch_dir: Option<PathBuf>) -> Result<()> {
    println!("🔒 CHRONOS Contract Broker Status\n");

    let broker = ContractEngine::new();

    // If watch_dir provided, index supported polyglot files inside that directory
    if let Some(dir) = watch_dir {
        let abs_dir = if dir.is_absolute() { dir } else { workspace.join(dir) };
        println!("👁️  Indexing: {}", abs_dir.display());

        let mut files = Vec::new();
        if abs_dir.is_file() {
            files.push(abs_dir);
        } else {
            collect_supported_files(&abs_dir, &mut files, 500);
        }

        let mut indexed = 0;
        for path in &files {
            if let Err(e) = broker.index_file(path).await {
                eprintln!("   ⚠️  {}: {}", path.display(), e);
            } else {
                indexed += 1;
            }
        }
        println!("   ✓ Indexed {} supported source files\n", indexed);
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
