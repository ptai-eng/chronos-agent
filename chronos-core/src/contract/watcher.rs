//! # File Watcher — Non-blocking workspace monitor
//!
//! Watches TypeScript source files for changes and dispatches events
//! to the Contract Broker for signature analysis.

use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// A file change event
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

#[derive(Debug, Clone)]
pub enum FileChangeKind {
    Modified,
    Created,
    Deleted,
}

/// Start watching a directory for TypeScript file changes.
///
/// Returns a broadcast receiver for file change events.
/// Debounces events with a 30ms window (ratified spec).
pub fn start_watching(
    watch_dir: PathBuf,
    sender: broadcast::Sender<FileChangeEvent>,
) -> Result<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(Duration::from_millis(30)),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;
    info!("🔍 Watching for changes in: {}", watch_dir.display());

    // Process events in a background thread
    let sender_clone = sender.clone();
    std::thread::spawn(move || {
        let mut last_event: std::collections::HashMap<PathBuf, std::time::Instant> = 
            std::collections::HashMap::new();

        for result in rx {
            match result {
                Ok(event) => {
                    process_notify_event(event, &mut last_event, &sender_clone);
                }
                Err(e) => {
                    warn!("File watcher error: {}", e);
                }
            }
        }
    });

    Ok(watcher)
}

/// Process a raw notify event with debouncing (simplified trailing edge via small sleep in thread)
fn process_notify_event(
    event: Event,
    last_events: &mut std::collections::HashMap<PathBuf, std::time::Instant>,
    sender: &broadcast::Sender<FileChangeEvent>,
) {
    let kind = match event.kind {
        EventKind::Modify(_) => FileChangeKind::Modified,
        EventKind::Create(_) => FileChangeKind::Created,
        EventKind::Remove(_) => FileChangeKind::Deleted,
        _ => return,
    };

    for path in event.paths {
        // Only care about TypeScript/JavaScript files
        if !is_ts_file(&path) {
            continue;
        }

        // Instead of dropping events entirely, we sleep briefly so the OS settles file writes,
        // then dispatch. We track the time to prevent a flood of dispatches.
        let now = std::time::Instant::now();
        if let Some(&last) = last_events.get(&path) {
            if now.duration_since(last).as_millis() < 30 {
                // If it's rapid fire, sleep slightly to let the file finish writing 
                // but DO NOT return, we still want to read the final state.
                std::thread::sleep(Duration::from_millis(30));
            }
        }
        last_events.insert(path.clone(), std::time::Instant::now());

        debug!("File changed: {} ({:?})", path.display(), kind);

        let event = FileChangeEvent { path, kind: kind.clone() };
        if let Err(e) = sender.send(event) {
            warn!("Failed to send file change event: {}", e);
        }
    }
}

fn is_ts_file(path: &PathBuf) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ts") | Some("tsx") | Some("js") | Some("jsx") | Some("mts") | Some("cts") => true,
        _ => false,
    }
}
