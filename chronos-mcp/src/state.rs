//! # Shared Server State
//!
//! Wraps `chronos_core::session::WorkspaceSession` in `Arc<RwLock<>>` for safe
//! concurrent MCP tool & background file-watcher access.

use chronos_core::session::WorkspaceSession;
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::RwLock;

/// The shared CHRONOS runtime state backed by `WorkspaceSession`.
pub struct ChronosState {
    pub session: WorkspaceSession,
}

impl ChronosState {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self::new_with_options(workspace_root, false)
    }

    pub fn new_with_options(workspace_root: PathBuf, readonly: bool) -> Self {
        let session = WorkspaceSession::open(workspace_root.clone(), readonly)
            .unwrap_or_else(|e| {
                tracing::warn!("Session persistence unavailable ({}), using in-memory mode", e);
                WorkspaceSession::open_in_memory(workspace_root, readonly)
            });
        Self { session }
    }
}

impl Deref for ChronosState {
    type Target = WorkspaceSession;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl DerefMut for ChronosState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}

/// Thread-safe handle to the shared state
pub type SharedState = Arc<RwLock<ChronosState>>;

pub fn new_shared_state(workspace_root: PathBuf) -> SharedState {
    Arc::new(RwLock::new(ChronosState::new(workspace_root)))
}

pub fn new_shared_state_with_options(workspace_root: PathBuf, readonly: bool) -> SharedState {
    Arc::new(RwLock::new(ChronosState::new_with_options(
        workspace_root,
        readonly,
    )))
}
