//! # Real-time File Watcher
//!
//! Monitors the workspace for source file changes and automatically
//! re-parses them into the 4D graph — so AI agents always see an
//! up-to-date knowledge graph without having to call `chronos_analyze_file`.

use crate::state::SharedState;
use chronos_core::ast::dispatcher::LanguageDispatcher;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

pub struct FileWatcher {
    handle: JoinHandle<()>,
    pub workspace: PathBuf,
}

/// Status snapshot returned by `chronos_watcher_status`
#[derive(serde::Serialize)]
pub struct WatcherStatus {
    pub watching: bool,
    pub workspace: String,
    pub last_sync: Option<String>,
    pub auto_synced_count: usize,
}

impl FileWatcher {
    /// Start the background file watcher for the given workspace.
    pub fn start(workspace_root: PathBuf, state: SharedState) -> anyhow::Result<Self> {
        let ws = workspace_root.clone();
        let (tx, mut rx) = mpsc::channel::<notify::Result<notify::Event>>(64);

        // Spawn the notify watcher on a blocking thread
        let ws_clone = workspace_root.clone();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
            let _ = tx.blocking_send(event);
        })?;
        watcher.watch(&ws_clone, RecursiveMode::Recursive)?;

        let handle = tokio::spawn(async move {
            // Keep watcher alive inside the task
            let _watcher = watcher;

            let mut pending: HashSet<PathBuf> = HashSet::new();
            let debounce = Duration::from_millis(300);
            let mut last_event = Instant::now();

            loop {
                // Try to receive with a short poll timeout for debounce flushing
                let recv = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;

                match recv {
                    Ok(Some(Ok(event))) => {
                        // Collect all modified / created paths
                        for path in event.paths {
                            if LanguageDispatcher::is_supported(&path) {
                                pending.insert(path);
                                last_event = Instant::now();
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        warn!("File watcher error: {}", e);
                    }
                    Ok(None) => {
                        // Channel closed — shut down
                        info!("File watcher channel closed, stopping.");
                        break;
                    }
                    Err(_) => {
                        // Timeout — check debounce
                    }
                }

                // Flush pending files after debounce window
                if !pending.is_empty() && last_event.elapsed() >= debounce {
                    let paths: Vec<PathBuf> = pending.drain().collect();
                    debug!("Auto-syncing {} changed files to graph", paths.len());

                    let mut dispatcher = match LanguageDispatcher::new() {
                        Ok(d) => d,
                        Err(e) => { warn!("Dispatcher init failed: {}", e); continue; }
                    };

                    let workspace = workspace_root.clone();
                    for path in paths {
                        // Security: only sync files within workspace
                        if let Ok(canonical) = path.canonicalize() {
                            if !canonical.starts_with(&workspace) {
                                continue;
                            }
                        }

                        match dispatcher.parse_file(&path) {
                            Ok(parsed) => {
                                let all_nodes: Vec<_> =
                                    parsed.all_nodes().into_iter().cloned().collect();
                                let mut st = state.write().await;
                                for node in &all_nodes {
                                    st.graph.add_node(node.clone());
                                }
                                info!("Auto-synced {} ({} symbols)", path.display(), all_nodes.len());
                            }
                            Err(e) => {
                                warn!("Auto-sync parse failed for {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        });

        Ok(Self { handle, workspace: ws })
    }

    /// Check if the watcher task is still running
    pub fn is_alive(&self) -> bool {
        !self.handle.is_finished()
    }
}
