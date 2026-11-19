//! # WorkspaceSession — Deep Canonical Workspace Lifecycle Manager
//!
//! Consolidates workspace validation, ignore-list directory walking, incremental
//! xxHash64 AST indexing, KuzuDB delta persistence, git coupling ingestion,
//! contract diffing, and write/readonly guards behind a single deep module.

use crate::{
    ast::{
        dispatcher::LanguageDispatcher,
        edit::FreshParsedFile,
        refactor::{execute_refactor_symbol, RefactorSymbolOutcome, RefactorSymbolRequest},
        symbol_editor::{execute_edit_symbol, EditSymbolOutcome, EditSymbolRequest},
        ts_parser::ParsedModule,
    },
    contract::{broker::ContractEvent, ContractEngine},
    graph::{
        git_coupling::ingest_git_co_changes,
        persistent::{compute_file_xxhash64, GraphRepository},
        schema::ChronosGraph,
    },
    script::{ScriptEngine, ScriptEvalOutcome, ScriptProfile},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use tracing::{info, warn};

/// Canonical directory names excluded from automated workspace indexing.
pub const IGNORED_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    ".chronos",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".next",
    ".turbo",
];

/// Summary of a workspace or scoped batch index operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_scanned: usize,
    pub files_reparsed: usize,
    pub unchanged_files: usize,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub git_co_change_edges: usize,
}

/// Summary of an atomic single-file re-index operation (`WorkspaceSession::index_file`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileIndexReport {
    pub abs_path: PathBuf,
    pub functions: usize,
    pub classes: usize,
    pub api_routes: usize,
    pub test_cases: usize,
    pub call_refs: usize,
    pub import_refs: usize,
    pub unresolved_call_sites: usize,
    pub nodes_inserted: usize,
    pub edges_added: usize,
    pub breaking_contract_event: Option<ContractEvent>,
    #[serde(default)]
    pub skipped_unchanged: bool,
}

/// Canonical runtime session bound to a single workspace root.
pub struct WorkspaceSession {
    pub workspace_root: PathBuf,
    pub graph: ChronosGraph,
    pub repo: Option<GraphRepository>,
    pub contracts: ContractEngine,
    pub readonly: bool,
    /// Tracks last indexed `xxHash64` per canonical file path so `FileWatcher` bursts
    /// after direct AST writes skip redundant re-parsing in <0.2ms.
    pub last_indexed_hashes: HashMap<PathBuf, String>,
}

impl WorkspaceSession {
    /// Open a workspace session backed by `.chronos/graph.kuzu` when writable/present,
    /// falling back cleanly to pure in-memory mode if persistence is unavailable.
    pub fn open(workspace_root: PathBuf, readonly: bool) -> Result<Self> {
        let root = normalize_workspace_root(&workspace_root);
        let db_path = root.join(".chronos").join("graph.kuzu");
        let repo = open_repository_if_allowed(&db_path, readonly);

        let mut graph = ChronosGraph::new();
        if let Some(ref r) = repo {
            match r.load_into_graph(&mut graph) {
                Ok(count) => info!("Restored {} nodes from persisted graph", count),
                Err(e) => warn!("Failed to restore persisted graph (starting empty): {}", e),
            }
        }

        Ok(Self {
            workspace_root: root,
            graph,
            repo,
            contracts: ContractEngine::new(),
            readonly,
            last_indexed_hashes: HashMap::new(),
        })
    }

    /// Create an in-memory-only session without opening or creating `.chronos/graph.kuzu`.
    pub fn open_in_memory(workspace_root: PathBuf, readonly: bool) -> Self {
        Self {
            workspace_root: normalize_workspace_root(&workspace_root),
            graph: ChronosGraph::new(),
            repo: None,
            contracts: ContractEngine::new(),
            readonly,
            last_indexed_hashes: HashMap::new(),
        }
    }

    /// Rebind the session when MCP `initialize` specifies a different existing workspace root.
    pub fn rebind_workspace(&mut self, new_root: PathBuf) -> Result<()> {
        let normalized = normalize_workspace_root(&new_root);
        if normalized == self.workspace_root || !normalized.exists() {
            return Ok(());
        }
        let reopened = Self::open(normalized, self.readonly)?;
        self.workspace_root = reopened.workspace_root;
        self.graph = reopened.graph;
        self.repo = reopened.repo;
        self.contracts = reopened.contracts;
        self.last_indexed_hashes = reopened.last_indexed_hashes;
        Ok(())
    }

    pub fn graph(&self) -> &ChronosGraph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut ChronosGraph {
        &mut self.graph
    }

    pub fn contracts(&self) -> &ContractEngine {
        &self.contracts
    }

    pub fn repo(&self) -> Option<&GraphRepository> {
        self.repo.as_ref()
    }

    pub fn ensure_writable(&self) -> Result<()> {
        if self.readonly {
            anyhow::bail!("CHRONOS is running in --readonly mode; workspace modifications are disabled.");
        }
        Ok(())
    }

    /// Validate that `candidate` (relative or absolute) resolves strictly inside `workspace_root`.
    pub fn validate_workspace_path(&self, candidate: &Path) -> Result<PathBuf> {
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.workspace_root.join(candidate)
        };

        let resolved = if joined.exists() {
            strip_windows_verbatim(
                joined
                    .canonicalize()
                    .with_context(|| format!("Failed to canonicalize path {}", joined.display()))?,
            )
        } else {
            resolve_nonexistent_within_workspace(&joined)?
        };

        if !resolved.starts_with(&self.workspace_root) {
            anyhow::bail!(
                "Security: path '{}' escapes workspace root '{}'",
                candidate.display(),
                self.workspace_root.display()
            );
        }

        Ok(resolved)
    }

    /// Parse an in-memory buffer with Tree-sitter and return `Some((line, col))` on first syntax error.
    pub fn validate_syntax(path: &Path, source: &str) -> Result<Option<(usize, usize)>> {
        LanguageDispatcher::check_syntax_errors(path, source)
    }

    /// Always read current disk bytes and freshly parse a workspace file via Tree-sitter before
    /// any mutation or symbol-boundary resolution (never trusts cached graph byte offsets).
    pub fn parse_fresh_file(&self, file_path: &Path) -> Result<FreshParsedFile> {
        let abs_path = self.validate_workspace_path(file_path)?;
        if !abs_path.exists() {
            anyhow::bail!("File not found: {}", abs_path.display());
        }
        if !LanguageDispatcher::is_supported(&abs_path) {
            let ext = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("?");
            anyhow::bail!(
                "Unsupported file type: '.{}'. Supported: .ts .tsx .js .jsx .py .rs .go .c .cpp .java",
                ext
            );
        }
        FreshParsedFile::from_disk(&abs_path)
    }

    /// Execute a 2-stage syntax & blast-radius gated symbol edit against a fresh disk parse.
    pub async fn edit_symbol(&mut self, req: EditSymbolRequest) -> Result<EditSymbolOutcome> {
        execute_edit_symbol(self, req).await
    }

    /// Execute graph-edge-verified multi-file refactoring (`preview_usages`, `rename`, `safe_delete`).
    pub async fn refactor_symbol(
        &mut self,
        req: RefactorSymbolRequest,
    ) -> Result<RefactorSymbolOutcome> {
        execute_refactor_symbol(self, req).await
    }

    /// Evaluate a bounded `rhai` 5D graph script under strict `ScriptProfile` capabilities.
    pub fn eval_script(
        &mut self,
        profile: ScriptProfile,
        script: &str,
    ) -> Result<ScriptEvalOutcome> {
        ScriptEngine::eval(self, profile, script)
    }

    /// Ensure the workspace (or an optional scoped subdirectory/file) is indexed incrementally.
    pub fn ensure_indexed(
        &mut self,
        scope: Option<&Path>,
        max_files: usize,
    ) -> Result<IndexStats> {
        let files = match scope {
            Some(p) => {
                let resolved = self.validate_workspace_path(p)?;
                let mut out = Vec::new();
                if resolved.is_dir() {
                    collect_supported_files(&resolved, &mut out, max_files);
                } else if resolved.exists() && LanguageDispatcher::is_supported(&resolved) {
                    out.push(resolved);
                }
                out
            }
            None => {
                let mut out = Vec::new();
                collect_supported_files(&self.workspace_root, &mut out, max_files);
                out
            }
        };
        self.index_file_list(&files, false)
    }

    /// Index an explicit list of files/directories (used by CLI subcommands with optional `-f` args).
    pub fn ensure_indexed_files(
        &mut self,
        explicit_files: &[PathBuf],
        max_files: usize,
    ) -> Result<IndexStats> {
        if explicit_files.is_empty() {
            return self.ensure_indexed(None, max_files);
        }

        let mut targets = Vec::new();
        for p in explicit_files {
            let resolved = if p.is_absolute() {
                strip_windows_verbatim(p.canonicalize().unwrap_or_else(|_| p.clone()))
            } else {
                self.workspace_root.join(p)
            };
            if resolved.is_dir() {
                collect_supported_files(&resolved, &mut targets, max_files);
            } else if resolved.exists() && LanguageDispatcher::is_supported(&resolved) {
                targets.push(resolved);
            }
        }
        self.index_file_list(&targets, true)
    }

    fn index_file_list(&mut self, files: &[PathBuf], force_explicit: bool) -> Result<IndexStats> {
        let mut dispatcher = LanguageDispatcher::new()?;
        let mut files_reparsed = 0usize;
        let mut unchanged_files = 0usize;

        if !self.readonly {
            if let Some(ref repo) = self.repo {
                let sync = repo.sync_workspace_incremental(&mut self.graph, files, &mut dispatcher)?;
                files_reparsed = sync.re_parsed_files;
                unchanged_files = sync.unchanged_files;
                for f in files {
                    if let Ok(h) = compute_file_xxhash64(f) {
                        let clean_p = strip_windows_verbatim(f.canonicalize().unwrap_or_else(|_| f.clone()));
                        self.last_indexed_hashes.insert(clean_p, h);
                    }
                }
            } else if force_explicit || self.graph.node_count() == 0 {
                for f in files {
                    if let Ok(parsed) = dispatcher.parse_file(f) {
                        if force_explicit {
                            self.graph.remove_nodes_for_file(f);
                        }
                        self.graph.ingest_parsed_module(&parsed);
                        files_reparsed += 1;
                        if let Ok(h) = compute_file_xxhash64(f) {
                            let clean_p = strip_windows_verbatim(f.canonicalize().unwrap_or_else(|_| f.clone()));
                            self.last_indexed_hashes.insert(clean_p, h);
                        }
                    }
                }
                self.graph.resolve_pending_calls_and_routes();
            } else {
                unchanged_files = files.len();
            }
        } else if force_explicit || self.graph.node_count() == 0 {
            for f in files {
                if let Ok(parsed) = dispatcher.parse_file(f) {
                    if force_explicit {
                        self.graph.remove_nodes_for_file(f);
                    }
                    self.graph.ingest_parsed_module(&parsed);
                    files_reparsed += 1;
                    if let Ok(h) = compute_file_xxhash64(f) {
                        let clean_p = strip_windows_verbatim(f.canonicalize().unwrap_or_else(|_| f.clone()));
                        self.last_indexed_hashes.insert(clean_p, h);
                    }
                }
            }
            self.graph.resolve_pending_calls_and_routes();
        } else {
            unchanged_files = files.len();
        }

        let git_co_change_edges = ingest_git_co_changes(&mut self.graph, &self.workspace_root, 200);

        Ok(IndexStats {
            files_scanned: files.len(),
            files_reparsed,
            unchanged_files,
            total_nodes: self.graph.node_count(),
            total_edges: self.graph.edge_count(),
            git_co_change_edges,
        })
    }

    /// Atomically re-index a single source file (BUG-02 fix):
    /// 1. Validate canonical path is inside workspace root
    /// 2. Evict all stale nodes previously belonging to this file
    /// 3. Parse fresh AST and ingest updated symbols + resolve call/route edges
    /// 4. Run polyglot ContractEngine diff cross-referenced with downstream callers
    /// 5. Flush file-scoped KuzuDB delta + xxHash64
    pub async fn index_file(&mut self, file_path: &Path) -> Result<FileIndexReport> {
        let abs_path = self.validate_workspace_path(file_path)?;
        if !abs_path.exists() {
            anyhow::bail!("File not found: {}", abs_path.display());
        }
        if !LanguageDispatcher::is_supported(&abs_path) {
            let ext = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("?");
            anyhow::bail!(
                "Unsupported file type: '.{}'. Supported: .ts .tsx .js .jsx .py .rs .go .c .cpp .java",
                ext
            );
        }

        let current_hash = compute_file_xxhash64(&abs_path)?;
        if self.last_indexed_hashes.get(&abs_path) == Some(&current_hash) {
            return Ok(FileIndexReport {
                abs_path,
                skipped_unchanged: true,
                ..Default::default()
            });
        }

        let (source, _enc) = crate::ast::AtomicFileWriter::read_utf8_preserving_metadata(&abs_path)?;
        let mut dispatcher = LanguageDispatcher::new()?;
        let parsed: ParsedModule = dispatcher.parse_source(&abs_path, &source)?;

        let functions = parsed.functions.len();
        let classes = parsed.classes.len();
        let api_routes = parsed.api_routes.len();
        let test_cases = parsed.test_cases.len();
        let call_refs = parsed.call_refs.len();
        let import_refs = parsed.import_refs.len();
        let unresolved_call_sites = parsed.unresolved_call_sites;

        // BUG-02 fix: always evict stale symbols before ingesting updated AST
        self.graph.remove_nodes_for_file(&abs_path);
        let (nodes_inserted, edges_added) = self.graph.ingest_parsed_module(&parsed);
        self.graph.resolve_pending_calls_and_routes();

        let breaking_contract_event = self
            .contracts
            .update_from_parsed(&parsed, &source, Some(&self.graph))
            .await;

        if !self.readonly {
            if let Some(ref repo) = self.repo {
                if let Err(e) = repo.sync_file_delta(&self.graph, &abs_path, Some(&current_hash)) {
                    warn!(
                        "KuzuDB delta sync failed for {}: {}",
                        abs_path.display(),
                        e
                    );
                }
            }
        }
        self.last_indexed_hashes.insert(abs_path.clone(), current_hash);

        Ok(FileIndexReport {
            abs_path,
            functions,
            classes,
            api_routes,
            test_cases,
            call_refs,
            import_refs,
            unresolved_call_sites,
            nodes_inserted,
            edges_added,
            breaking_contract_event,
            skipped_unchanged: false,
        })
    }

    /// Evict nodes and persisted delta when a tracked workspace file is deleted.
    pub fn evict_file(&mut self, file_path: &Path) {
        self.last_indexed_hashes.remove(file_path);
        self.graph.remove_nodes_for_file(file_path);
        if let Some(ref repo) = self.repo {
            if let Err(e) = repo.sync_file_delta(&self.graph, file_path, None) {
                warn!("Failed to evict deleted file {} from KuzuDB: {}", file_path.display(), e);
            }
        }
    }
}

/// Walk `dir` collecting supported source files up to `max_files` while respecting `IGNORED_DIR_NAMES`.
pub fn collect_supported_files(dir: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
    if out.len() >= max_files {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= max_files {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if IGNORED_DIR_NAMES.iter().any(|ignored| *ignored == name) {
                continue;
            }
            collect_supported_files(&path, out, max_files);
        } else if LanguageDispatcher::is_supported(&path) {
            out.push(path);
        }
    }
}

/// Clean URI prefixes and strip Windows `\\?\` verbatim prefix after canonicalization.
pub fn normalize_workspace_root(raw: &Path) -> PathBuf {
    let text = raw.to_string_lossy();
    let uri_cleaned = if let Some(rest) = text.strip_prefix("file://") {
        // Handle `file:///C:/...` on Windows vs `file:///unix/path`
        if rest.starts_with('/') && rest.as_bytes().get(2) == Some(&b':') {
            PathBuf::from(&rest[1..])
        } else {
            PathBuf::from(rest)
        }
    } else {
        raw.to_path_buf()
    };

    let canonical = uri_cleaned.canonicalize().unwrap_or(uri_cleaned);
    strip_windows_verbatim(canonical)
}

fn strip_windows_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

fn resolve_nonexistent_within_workspace(target: &Path) -> Result<PathBuf> {
    let lexical = lexically_normalize(target);
    let mut missing_suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = lexical.clone();

    while !cursor.exists() {
        match cursor.file_name() {
            Some(name) => {
                missing_suffix.push(name.to_os_string());
                if !cursor.pop() {
                    break;
                }
            }
            None => break,
        }
    }

    if cursor.exists() {
        let mut base = strip_windows_verbatim(cursor.canonicalize()?);
        for seg in missing_suffix.into_iter().rev() {
            base.push(seg);
        }
        Ok(base)
    } else {
        Ok(lexical)
    }
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn open_repository_if_allowed(db_path: &Path, readonly: bool) -> Option<GraphRepository> {
    if readonly && !db_path.exists() {
        return None;
    }
    if !readonly {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    GraphRepository::open(db_path)
        .map_err(|e| warn!("KuzuDB unavailable: {} (using in-memory only)", e))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_index_file_evicts_deleted_symbols_and_updates_repo_delta() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("service.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            "pub fn legacy_alpha(x: i32) -> i32 { x + 1 }\npub fn keep_beta(y: i32) -> i32 { y * 2 }\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open(dir.path().to_path_buf(), false).unwrap();
        let first = session.index_file(&file).await.unwrap();
        assert_eq!(first.functions, 2);
        assert!(session.graph.resolve_first_symbol("legacy_alpha", None).is_some());
        assert!(session.graph.resolve_first_symbol("keep_beta", None).is_some());

        // Overwrite file removing `legacy_alpha` and renaming/adding `fresh_gamma`
        std::fs::write(
            &file,
            "pub fn keep_beta(y: i32) -> i32 { y * 2 }\npub fn fresh_gamma(z: i32) -> i32 { z + 3 }\n",
        )
        .unwrap();

        let second = session.index_file(&file).await.unwrap();
        assert_eq!(second.functions, 2);
        assert!(
            session.graph.resolve_first_symbol("legacy_alpha", None).is_none(),
            "BUG-02 regression: deleted symbol legacy_alpha must be evicted on re-index"
        );
        assert!(session.graph.resolve_first_symbol("keep_beta", None).is_some());
        assert!(session.graph.resolve_first_symbol("fresh_gamma", None).is_some());

        // Verify KuzuDB delta persistence also dropped the deleted node
        drop(session);
        let restored = WorkspaceSession::open(dir.path().to_path_buf(), false).unwrap();
        assert!(
            restored.graph.resolve_first_symbol("legacy_alpha", None).is_none(),
            "BUG-02/BUG-03: evicted symbol must not reappear from persisted KuzuDB"
        );
        assert!(restored.graph.resolve_first_symbol("fresh_gamma", None).is_some());
    }

    #[test]
    fn test_validate_workspace_path_rejects_escape_and_enforces_readonly() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().parent().unwrap().join("escape_target.rs");
        let session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), true);

        assert!(session.validate_workspace_path(&outside).is_err());
        assert!(session
            .validate_workspace_path(Path::new("../outside_rel.rs"))
            .is_err());
        assert!(session
            .validate_workspace_path(Path::new("src/new_file.rs"))
            .is_ok());
        assert!(session.ensure_writable().is_err());
    }

    #[test]
    fn test_collect_supported_files_respects_canonical_ignore_list() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        std::fs::create_dir_all(dir.path().join("__pycache__")).unwrap();

        std::fs::write(dir.path().join("src/app.ts"), "export function ok() {}").unwrap();
        std::fs::write(
            dir.path().join("node_modules/pkg/index.ts"),
            "export function ignoreMe() {}",
        )
        .unwrap();
        std::fs::write(dir.path().join("target/debug/gen.rs"), "fn ignore_me() {}").unwrap();
        std::fs::write(dir.path().join("__pycache__/mod.py"), "def ignore_me(): pass").unwrap();

        let mut out = Vec::new();
        collect_supported_files(dir.path(), &mut out, 100);
        assert_eq!(out.len(), 1);
        assert!(out[0].ends_with("app.ts"));
    }

    #[tokio::test]
    async fn test_index_file_skips_unchanged_xxhash64() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("cached.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "pub fn stable() -> i32 { 42 }\n").unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        let first = session.index_file(&file).await.unwrap();
        assert!(!first.skipped_unchanged);
        assert_eq!(first.functions, 1);

        // Second call without modifying file content short-circuits via xxHash64
        let second = session.index_file(&file).await.unwrap();
        assert!(second.skipped_unchanged);
        assert_eq!(second.functions, 0);

        // Editing content invalidates xxHash64 cache and triggers full re-parse
        std::fs::write(&file, "pub fn stable() -> i32 { 43 }\n").unwrap();
        let third = session.index_file(&file).await.unwrap();
        assert!(!third.skipped_unchanged);
        assert_eq!(third.functions, 1);
    }
}
