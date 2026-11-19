//! # Graph-Edge-Verified Multi-File Refactoring Engine (`chronos_refactor_symbol`)
//!
//! Performs safe, edge-verified multi-file refactoring (`rename`, `safe_delete`, `preview_usages`):
//! 1. Freshly parses defining and connected workspace files from disk (never writes using cached offsets)
//! 2. Walks Tree-sitter CST skipping comments and string literals
//! 3. Separates `verified_edits` (declaration, verified `CALLS`/`TESTS` call sites, `import`/`use` specifiers)
//!    from `candidate_sites` (unverified member accesses `obj.sym`, shadowed local bindings, callback values)
//! 4. Blocks ambiguous renames (`blocked_ambiguous_rename`) whenever `candidate_sites`, `homonyms > 1`,
//!    or `unresolved_calls > 0` exist without `force: true`
//! 5. Validates 100% of modified buffers in memory via Tree-sitter before executing an all-or-nothing
//!    atomic multi-file transaction (`AtomicFileWriter::commit_transaction`)
//! 6. Hardens `safe_delete` against `CALLS`, `TESTS`, `ApiRoute`, entrypoint (`main`/`#[test]`/`#[tokio::main]`),
//!    visibility (`pub`/`export` under incomplete coverage), and Rust trait impl guards.

use crate::ast::atomic_writer::AtomicFileWriter;
use crate::ast::dispatcher::LanguageDispatcher;
use crate::ast::edit::{
    apply_edits_in_memory, extract_go_method_sig, find_go_interface_match_in_tree,
    generate_unified_diff, FreshParsedFile, FreshSymbolSpan, TextEdit,
};
use crate::blast_radius::impact_matrix::CoverageReport;
use crate::graph::query::{GraphQueryEngine, VerificationSurface};
use crate::graph::schema::{ChronosNode, EdgeKind, NodeKind};
use crate::session::WorkspaceSession;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Supported actions for `chronos_refactor_symbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorAction {
    PreviewUsages,
    Rename,
    SafeDelete,
}

impl RefactorAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RefactorAction::PreviewUsages => "preview_usages",
            RefactorAction::Rename => "rename",
            RefactorAction::SafeDelete => "safe_delete",
        }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "preview_usages" | "preview" | "usages" => Ok(RefactorAction::PreviewUsages),
            "rename" => Ok(RefactorAction::Rename),
            "safe_delete" | "delete" => Ok(RefactorAction::SafeDelete),
            other => Err(anyhow!(
                "Unsupported refactor action '{}'. Expected 'preview_usages', 'rename', or 'safe_delete'.",
                other
            )),
        }
    }
}

/// Request payload for `WorkspaceSession::refactor_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSymbolRequest {
    pub symbol: String,
    pub action: RefactorAction,
    #[serde(default)]
    pub new_name: Option<String>,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub force: bool,
}

/// A classified AST occurrence discovered during fresh usage/rename analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefSite {
    pub index: usize,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    /// `"declaration"` | `"call"` | `"test_call"` | `"import"` | `"member_access"` | `"shadowed_local"` | `"unverified_call"`
    pub kind: String,
    pub reason: String,
    pub snippet: String,
}

/// Structured result returned by `WorkspaceSession::refactor_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorSymbolOutcome {
    /// `"preview"` | `"dry_run"` | `"applied"` | `"blocked_ambiguous_rename"` | `"blocked_active_dependents"` | `"syntax_error"`
    pub status: String,
    pub is_error: bool,
    pub action: String,
    pub symbol: String,
    pub target_file: String,
    pub new_name: Option<String>,
    pub homonyms: usize,
    pub unresolved_calls: usize,
    pub verified_edits: Vec<RefSite>,
    pub candidate_sites: Vec<RefSite>,
    pub files_modified: Vec<String>,
    pub blockers: Vec<String>,
    pub verification_surfaces: Vec<VerificationSurface>,
    pub unified_diff: String,
    pub summary: String,
}

/// Internal target resolution record pairing optional graph node with fresh CST parse.
struct ResolvedTarget {
    graph_node: Option<ChronosNode>,
    fresh_file: FreshParsedFile,
    fresh_symbol: FreshSymbolSpan,
    short_name: String,
}

/// Execute `preview_usages`, `rename`, or `safe_delete` within `session`.
pub async fn execute_refactor_symbol(
    session: &mut WorkspaceSession,
    req: RefactorSymbolRequest,
) -> Result<RefactorSymbolOutcome> {
    if !matches!(req.action, RefactorAction::PreviewUsages) && !req.dry_run {
        session.ensure_writable()?;
    }

    let resolved = resolve_refactor_target(session, &req.symbol, req.file.as_deref())?;
    let target_file_rel = to_rel_display(&session.workspace_root, &resolved.fresh_file.abs_path);

    match req.action {
        RefactorAction::PreviewUsages => {
            let analysis = collect_usage_classification(session, &resolved)?;
            let mut surfaces = Vec::new();
            if let Some(ref gnode) = resolved.graph_node {
                surfaces = GraphQueryEngine::new(&session.graph).trace_verification_surfaces(&gnode.id, 5);
            }
            Ok(RefactorSymbolOutcome {
                status: "preview".to_string(),
                is_error: false,
                action: req.action.as_str().to_string(),
                symbol: resolved.fresh_symbol.name.clone(),
                target_file: target_file_rel,
                new_name: req.new_name.clone(),
                homonyms: analysis.homonyms,
                unresolved_calls: analysis.unresolved_calls,
                summary: format!(
                    "Previewed '{}' usages: {} verified site(s), {} candidate site(s), {} homonym(s), {} unresolved call(s).",
                    resolved.fresh_symbol.name,
                    analysis.verified_sites.len(),
                    analysis.candidate_sites.len(),
                    analysis.homonyms,
                    analysis.unresolved_calls
                ),
                verified_edits: analysis.verified_sites,
                candidate_sites: analysis.candidate_sites,
                files_modified: Vec::new(),
                blockers: Vec::new(),
                verification_surfaces: surfaces,
                unified_diff: String::new(),
            })
        }
        RefactorAction::Rename => execute_rename(session, &req, resolved, target_file_rel).await,
        RefactorAction::SafeDelete => {
            execute_safe_delete(session, &req, resolved, target_file_rel).await
        }
    }
}

async fn execute_rename(
    session: &mut WorkspaceSession,
    req: &RefactorSymbolRequest,
    resolved: ResolvedTarget,
    target_file_rel: String,
) -> Result<RefactorSymbolOutcome> {
    let raw_new_input = req
        .new_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("'new_name' is required for refactor action 'rename'"))?;
    let raw_new = short_symbol_name(raw_new_input);

    if !is_valid_identifier(raw_new) {
        return Err(anyhow!(
            "Invalid target identifier '{}': must start with letter/underscore and contain alphanumeric/underscore characters.",
            raw_new
        ));
    }
    if raw_new == resolved.short_name {
        return Err(anyhow!(
            "Target name '{}' is identical to existing symbol name.",
            raw_new
        ));
    }

    // Prevent colliding with an existing symbol definition in the same file / class scope
    let collision = resolved.fresh_file.symbols.iter().any(|s| {
        let leaf = short_symbol_name(&s.name);
        leaf == raw_new
            && s.enclosing_type == resolved.fresh_symbol.enclosing_type
            && s.declaration_span.start_byte != resolved.fresh_symbol.declaration_span.start_byte
    });
    if collision {
        return Err(anyhow!(
            "Rename collision: symbol '{}' already exists in '{}'",
            raw_new,
            target_file_rel
        ));
    }

    let analysis = collect_usage_classification(session, &resolved)?;
    if analysis.verified_sites.is_empty() {
        return Err(anyhow!(
            "Could not locate declaration identifier span to rename for symbol '{}' in '{}'",
            resolved.fresh_symbol.name,
            target_file_rel
        ));
    }
    let mut surfaces = Vec::new();
    if let Some(ref gnode) = resolved.graph_node {
        surfaces = GraphQueryEngine::new(&session.graph).trace_verification_surfaces(&gnode.id, 5);
    }

    // Build proposed multi-file in-memory edits from `verified_edits`
    let mut edits_by_file: BTreeMap<PathBuf, Vec<TextEdit>> = BTreeMap::new();
    for site in &analysis.verified_sites {
        let abs = session.validate_workspace_path(Path::new(&site.file))?;
        edits_by_file.entry(abs).or_default().push(TextEdit::new(
            &site.file,
            site.start_byte,
            site.end_byte,
            site.line,
            site.line,
            raw_new,
        ));
    }

    let mut staged_writes: Vec<(PathBuf, String)> = Vec::with_capacity(edits_by_file.len());
    let mut diff_chunks: Vec<String> = Vec::new();
    let mut modified_rel: Vec<String> = Vec::new();

    for (abs_path, file_edits) in &edits_by_file {
        let orig_buf = analysis
            .file_sources
            .get(abs_path)
            .cloned()
            .ok_or_else(|| anyhow!("Missing source snapshot for {}", abs_path.display()))?;
        let rel = to_rel_display(&session.workspace_root, abs_path);
        let updated_buf = apply_edits_in_memory(&orig_buf, file_edits)?;

        // Stage 1 multi-file syntax check before any disk write or gate decision
        if let Some((err_line, err_col)) =
            WorkspaceSession::validate_syntax(abs_path, &updated_buf)?
        {
            let diff = generate_unified_diff(&rel, &orig_buf, &updated_buf);
            return Ok(RefactorSymbolOutcome {
                status: "syntax_error".to_string(),
                is_error: true,
                action: req.action.as_str().to_string(),
                symbol: resolved.fresh_symbol.name.clone(),
                target_file: target_file_rel,
                new_name: Some(raw_new.to_string()),
                homonyms: analysis.homonyms,
                unresolved_calls: analysis.unresolved_calls,
                verified_edits: analysis.verified_sites,
                candidate_sites: analysis.candidate_sites,
                files_modified: Vec::new(),
                blockers: vec![format!(
                    "In-memory syntax validation failed on '{}' at {}:{} — no files written.",
                    rel, err_line, err_col
                )],
                verification_surfaces: surfaces,
                unified_diff: diff,
                summary: format!(
                    "Aborted rename cleanly at {}:{} in '{}' — all files untouched.",
                    err_line, err_col, rel
                ),
            });
        }

        let diff = generate_unified_diff(&rel, &orig_buf, &updated_buf);
        if !diff.is_empty() {
            diff_chunks.push(diff);
        }
        modified_rel.push(rel);
        staged_writes.push((abs_path.clone(), updated_buf));
    }

    let combined_diff = diff_chunks.join("\n");
    let mut blockers = Vec::new();
    if !analysis.candidate_sites.is_empty() {
        blockers.push(format!(
            "{} unverified/ambiguous candidate site(s) detected (member accesses or shadowed locals)",
            analysis.candidate_sites.len()
        ));
    }
    if analysis.homonyms > 1 {
        blockers.push(format!(
            "{} homonym definitions found across workspace with name '{}'",
            analysis.homonyms, resolved.short_name
        ));
    }
    if analysis.unresolved_calls > 0 {
        blockers.push(format!(
            "{} dynamic/unresolved call site(s) exist in affected workspace files",
            analysis.unresolved_calls
        ));
    }

    if req.dry_run {
        return Ok(RefactorSymbolOutcome {
            status: "dry_run".to_string(),
            is_error: false,
            action: req.action.as_str().to_string(),
            symbol: resolved.fresh_symbol.name.clone(),
            target_file: target_file_rel,
            new_name: Some(raw_new.to_string()),
            homonyms: analysis.homonyms,
            unresolved_calls: analysis.unresolved_calls,
            verified_edits: analysis.verified_sites,
            candidate_sites: analysis.candidate_sites,
            files_modified: modified_rel,
            blockers,
            verification_surfaces: surfaces,
            unified_diff: combined_diff,
            summary: "Dry-run multi-file rename syntax validated cleanly; disk untouched."
                .to_string(),
        });
    }

    if !blockers.is_empty() && !req.force {
        return Ok(RefactorSymbolOutcome {
            status: "blocked_ambiguous_rename".to_string(),
            is_error: true,
            action: req.action.as_str().to_string(),
            symbol: resolved.fresh_symbol.name.clone(),
            target_file: target_file_rel,
            new_name: Some(raw_new.to_string()),
            homonyms: analysis.homonyms,
            unresolved_calls: analysis.unresolved_calls,
            verified_edits: analysis.verified_sites,
            candidate_sites: analysis.candidate_sites,
            files_modified: Vec::new(),
            blockers: blockers.clone(),
            verification_surfaces: surfaces,
            unified_diff: combined_diff,
            summary: format!(
                "Blocked rename '{} -> {}': {}. Review candidate_sites or pass force=true.",
                resolved.fresh_symbol.name,
                raw_new,
                blockers.join("; ")
            ),
        });
    }

    // All-or-nothing transactional disk write (with full rollback if any file fails)
    AtomicFileWriter::commit_transaction(&staged_writes)?;
    for (abs_path, _) in &staged_writes {
        let _ = session.index_file(abs_path).await?;
    }

    let skipped_note = if analysis.candidate_sites.is_empty() {
        String::new()
    } else {
        format!(
            " ({} unverified candidate site(s) left untouched for manual review)",
            analysis.candidate_sites.len()
        )
    };

    Ok(RefactorSymbolOutcome {
        status: "applied".to_string(),
        is_error: false,
        action: req.action.as_str().to_string(),
        symbol: resolved.fresh_symbol.name.clone(),
        target_file: target_file_rel,
        new_name: Some(raw_new.to_string()),
        homonyms: analysis.homonyms,
        unresolved_calls: analysis.unresolved_calls,
        verified_edits: analysis.verified_sites,
        candidate_sites: analysis.candidate_sites,
        files_modified: modified_rel.clone(),
        blockers,
        verification_surfaces: surfaces,
        unified_diff: combined_diff,
        summary: format!(
            "Renamed '{}' -> '{}' across {} file(s){}.",
            resolved.fresh_symbol.name,
            raw_new,
            modified_rel.len(),
            skipped_note
        ),
    })
}

async fn execute_safe_delete(
    session: &mut WorkspaceSession,
    req: &RefactorSymbolRequest,
    resolved: ResolvedTarget,
    target_file_rel: String,
) -> Result<RefactorSymbolOutcome> {
    let mut blockers: Vec<String> = Vec::new();
    let mut surfaces: Vec<VerificationSurface> = Vec::new();

    let (coverage, _, _) = CoverageReport::from_graph(&session.graph);

    if let Some(ref gnode) = resolved.graph_node {
        let callers = session.graph.callers_of(&gnode.id);
        if !callers.is_empty() {
            let names: Vec<String> = callers.iter().map(|c| c.name.clone()).collect();
            blockers.push(format!("Active CALLS dependents: {}", names.join(", ")));
            for caller in callers {
                if caller.kind == NodeKind::ApiRoute {
                    blockers.push(format!("Impacts exposed ApiRoute '{}'", caller.name));
                }
            }
        }
        let tests: Vec<_> = session
            .graph
            .find_covering_tests(&gnode.id)
            .into_iter()
            .filter(|t| t.id != gnode.id)
            .collect();
        if !tests.is_empty() {
            let names: Vec<String> = tests.iter().map(|t| t.name.clone()).collect();
            blockers.push(format!("Covered by TESTS: {}", names.join(", ")));
        }

        surfaces = GraphQueryEngine::new(&session.graph).trace_verification_surfaces(&gnode.id, 5);
        for s in &surfaces {
            if s.surface_kind == "CliCommand" || s.surface_kind == "ApiRoute" {
                blockers.push(format!(
                    "Reachable surface {} '{}' ({}:{})",
                    s.surface_kind, s.name, s.file, s.line
                ));
            }
        }
    }

    if resolved.fresh_symbol.has_entrypoint_attr {
        blockers.push(format!(
            "Entrypoint/test/route attribute guard on '{}'",
            resolved.fresh_symbol.name
        ));
    }
    if let Some(reason) = &resolved.fresh_symbol.interface_impl_reason {
        blockers.push(format!("{reason} cannot be safely deleted automatically"));
    } else if resolved.fresh_symbol.is_trait_impl {
        blockers.push(format!(
            "Rust trait implementation method '{}' cannot be safely deleted automatically",
            resolved.fresh_symbol.name
        ));
    } else if resolved
        .fresh_file
        .abs_path
        .extension()
        .and_then(|e| e.to_str())
        == Some("go")
    {
        if let Some(reason) = find_cross_file_go_interface_match(
            session,
            &resolved.fresh_file.abs_path,
            &resolved.fresh_file.source,
            resolved.fresh_symbol.declaration_span.start_byte,
            resolved.fresh_symbol.declaration_span.end_byte,
        ) {
            blockers.push(format!("{reason} cannot be safely deleted automatically"));
        }
    }
    if resolved.fresh_symbol.is_exported && coverage.completeness != "complete" {
        blockers.push(format!(
            "Exported/public symbol '{}' with coverage='{}' (pass force=true to override)",
            resolved.fresh_symbol.name, coverage.completeness
        ));
    }

    let full = resolved.fresh_symbol.full_symbol_span;
    let orig_src = &resolved.fresh_file.source;

    // Expand deletion range cleanly to swallow leading indent + trailing line break
    let line_start = orig_src[..full.start_byte]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let del_start = if orig_src[line_start..full.start_byte].trim().is_empty() {
        line_start
    } else {
        full.start_byte
    };

    let mut del_end = full.end_byte;
    if orig_src[del_end..].starts_with("\r\n") {
        del_end += 2;
    } else if orig_src[del_end..].starts_with('\n') {
        del_end += 1;
    }

    let mut edited = String::with_capacity(orig_src.len());
    edited.push_str(&orig_src[..del_start]);
    edited.push_str(&orig_src[del_end..]);
    let candidate_source = collapse_excess_blank_lines(&edited);

    let unified_diff = generate_unified_diff(&target_file_rel, orig_src, &candidate_source);

    // Stage 1 syntax validation before write
    if let Some((err_line, err_col)) =
        WorkspaceSession::validate_syntax(&resolved.fresh_file.abs_path, &candidate_source)?
    {
        return Ok(RefactorSymbolOutcome {
            status: "syntax_error".to_string(),
            is_error: true,
            action: req.action.as_str().to_string(),
            symbol: resolved.fresh_symbol.name.clone(),
            target_file: target_file_rel,
            new_name: None,
            homonyms: 0,
            unresolved_calls: coverage.unresolved_calls,
            verified_edits: Vec::new(),
            candidate_sites: Vec::new(),
            files_modified: Vec::new(),
            blockers: vec![format!(
                "Deleting '{}' would break syntax at {}:{}",
                resolved.fresh_symbol.name, err_line, err_col
            )],
            verification_surfaces: surfaces,
            unified_diff,
            summary: format!(
                "Aborted safe_delete due to syntax error at {}:{} — disk untouched.",
                err_line, err_col
            ),
        });
    }

    if req.dry_run {
        return Ok(RefactorSymbolOutcome {
            status: "dry_run".to_string(),
            is_error: false,
            action: req.action.as_str().to_string(),
            symbol: resolved.fresh_symbol.name.clone(),
            target_file: target_file_rel,
            new_name: None,
            homonyms: 0,
            unresolved_calls: coverage.unresolved_calls,
            verified_edits: Vec::new(),
            candidate_sites: Vec::new(),
            files_modified: vec![resolved.fresh_file.abs_path.to_string_lossy().to_string()],
            blockers,
            verification_surfaces: surfaces,
            unified_diff,
            summary: "Dry-run safe_delete preview computed cleanly; disk untouched.".to_string(),
        });
    }

    if !blockers.is_empty() && !req.force {
        return Ok(RefactorSymbolOutcome {
            status: "blocked_active_dependents".to_string(),
            is_error: true,
            action: req.action.as_str().to_string(),
            symbol: resolved.fresh_symbol.name.clone(),
            target_file: target_file_rel.clone(),
            new_name: None,
            homonyms: 0,
            unresolved_calls: coverage.unresolved_calls,
            verified_edits: Vec::new(),
            candidate_sites: Vec::new(),
            files_modified: Vec::new(),
            blockers: blockers.clone(),
            verification_surfaces: surfaces,
            unified_diff,
            summary: format!(
                "Blocked safe_delete of '{}': {}",
                resolved.fresh_symbol.name,
                blockers.join("; ")
            ),
        });
    }

    AtomicFileWriter::write_atomic_with_metadata(
        &resolved.fresh_file.abs_path,
        &candidate_source,
        resolved.fresh_file.encoding,
    )?;
    let _ = session.index_file(&resolved.fresh_file.abs_path).await?;

    Ok(RefactorSymbolOutcome {
        status: "applied".to_string(),
        is_error: false,
        action: req.action.as_str().to_string(),
        symbol: resolved.fresh_symbol.name.clone(),
        target_file: target_file_rel.clone(),
        new_name: None,
        homonyms: 0,
        unresolved_calls: coverage.unresolved_calls,
        verified_edits: Vec::new(),
        candidate_sites: Vec::new(),
        files_modified: vec![target_file_rel],
        blockers,
        verification_surfaces: surfaces,
        unified_diff,
        summary: format!(
            "Deleted '{}' (including attached leading attributes/docs) cleanly and synced graph.",
            resolved.fresh_symbol.name
        ),
    })
}

/// Inspect sibling `.go` files (and indexed `.go` files in the graph) to detect structural Go interface satisfaction.
fn find_cross_file_go_interface_match(
    session: &WorkspaceSession,
    target_abs: &Path,
    target_source: &str,
    decl_start: usize,
    decl_end: usize,
) -> Option<String> {
    let lang = LanguageDispatcher::language_for_path(target_abs).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(target_source, None)?;

    let mut decl_node = tree
        .root_node()
        .descendant_for_byte_range(decl_start, decl_end)?;
    while decl_node.kind() != "method_declaration" {
        decl_node = decl_node.parent()?;
    }
    let (method_name, param_count) = extract_go_method_sig(decl_node, target_source.as_bytes())?;

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(parent_dir) = target_abs.parent() {
        if let Ok(entries) = std::fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("go") && p != target_abs {
                    candidates.push(p);
                }
            }
        }
    }
    for n in session.graph.all_nodes() {
        if n.location.file.ends_with(".go") {
            let abs = session.workspace_root.join(&n.location.file);
            if abs != target_abs && !candidates.contains(&abs) {
                candidates.push(abs);
            }
        }
    }

    for go_file in candidates {
        let Ok(src) = std::fs::read_to_string(&go_file) else {
            continue;
        };
        if !src.contains(&method_name) {
            continue;
        }
        let Some(other_tree) = parser.parse(&src, None) else {
            continue;
        };
        if let Some(found) = find_go_interface_match_in_tree(
            other_tree.root_node(),
            src.as_bytes(),
            &method_name,
            param_count,
        ) {
            return Some(found);
        }
    }
    None
}

struct UsageAnalysis {
    verified_sites: Vec<RefSite>,
    candidate_sites: Vec<RefSite>,
    homonyms: usize,
    unresolved_calls: usize,
    file_sources: HashMap<PathBuf, String>,
}

fn collect_usage_classification(
    session: &WorkspaceSession,
    resolved: &ResolvedTarget,
) -> Result<UsageAnalysis> {
    let old_short = resolved.short_name.as_str();
    let defining_abs = resolved.fresh_file.abs_path.clone();
    let defining_stem = defining_abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    // Count homonym symbol definitions across the entire workspace graph
    let mut homonym_ids: HashSet<String> = HashSet::new();
    for node in session.graph.all_nodes() {
        if node.kind == NodeKind::File || node.kind == NodeKind::ApiRoute {
            continue;
        }
        if short_symbol_name(&node.name) == old_short {
            homonym_ids.insert(node.id.0.clone());
        }
    }
    let homonyms = homonym_ids.len().max(1);

    // Map inbound CALLS and TESTS edges pointing to target_id -> verified (file -> (caller_name -> lines))
    let mut verified_call_lines: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
    let mut verified_caller_files: HashSet<PathBuf> = HashSet::new();
    verified_caller_files.insert(defining_abs.clone());

    if let Some(ref gnode) = resolved.graph_node {
        for (src_id, dst_id, edge) in session.graph.all_edges() {
            if dst_id == gnode.id && matches!(edge.kind, EdgeKind::Calls | EdgeKind::Tests) {
                if let Some(caller_node) = session.graph.get_node(&src_id) {
                    if let Ok(abs_caller) = session.validate_workspace_path(&caller_node.location.file) {
                        verified_caller_files.insert(abs_caller.clone());
                        if let Some(line) = edge.call_site_line {
                            verified_call_lines.entry(abs_caller).or_default().insert(line);
                        }
                    }
                }
            }
        }
    }

    // Also include every workspace file indexed in graph whose content mentions `old_short`
    // (so unverified member accesses `other.old_short()` or shadowed locals are caught!).
    let mut candidate_files: BTreeMap<PathBuf, ()> = BTreeMap::new();
    candidate_files.insert(defining_abs.clone(), ());
    for f in &verified_caller_files {
        candidate_files.insert(f.clone(), ());
    }
    for node in session.graph.all_nodes() {
        if let Ok(abs_f) = session.validate_workspace_path(&node.location.file) {
            if abs_f.exists() {
                candidate_files.insert(abs_f, ());
            }
        }
    }

    let mut verified_sites = Vec::new();
    let mut candidate_sites = Vec::new();
    let mut file_sources: HashMap<PathBuf, String> = HashMap::new();
    let mut unresolved_calls = 0usize;

    let mut seen_offsets: HashSet<(PathBuf, usize, usize)> = HashSet::new();

    for abs_path in candidate_files.keys() {
        let fresh = if abs_path == &defining_abs {
            resolved.fresh_file.clone()
        } else {
            let Ok(raw) = std::fs::read_to_string(abs_path) else {
                continue;
            };
            if !raw.contains(old_short) {
                continue;
            }
            let Ok(fp) = session.parse_fresh_file(abs_path) else {
                continue;
            };
            fp
        };

        // Include local calls inside defining_file calling `old_short` directly
        for call in &fresh.parsed.call_refs {
            if short_symbol_name(&call.callee_name) == old_short {
                let callers_match = abs_path == &defining_abs
                    || fresh.parsed.import_refs.iter().any(|imp| {
                        imp.source_module.contains(&defining_stem)
                            || imp.imported_names.iter().any(|n| n == old_short || n == "*")
                    });
                if callers_match {
                    verified_call_lines
                        .entry(abs_path.clone())
                        .or_default()
                        .insert(call.call_site_line);
                }
            }
        }

        unresolved_calls += fresh.parsed.unresolved_call_sites;
        file_sources.insert(abs_path.clone(), fresh.source.clone());

        let imports_target_module = abs_path == &defining_abs
            || fresh.parsed.import_refs.iter().any(|imp| {
                !defining_stem.is_empty()
                    && (imp.source_module.contains(&defining_stem)
                        || imp.imported_names.iter().any(|n| n == old_short))
            });

        let rel_path = to_rel_display(&session.workspace_root, abs_path);
        let lang = crate::ast::dispatcher::LanguageDispatcher::language_for_path(abs_path)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang)?;
        let Some(tree) = parser.parse(&fresh.source, None) else {
            continue;
        };

        let verified_lines_for_file = verified_call_lines.get(abs_path);
        let target_decl_ident = if abs_path == &defining_abs {
            resolved.fresh_symbol.name_ident_span
        } else {
            None
        };

        collect_file_sites_from_cst(
            tree.root_node(),
            &fresh.source,
            abs_path,
            &rel_path,
            old_short,
            resolved.fresh_symbol.enclosing_type.as_deref(),
            &defining_stem,
            target_decl_ident,
            verified_lines_for_file,
            imports_target_module,
            &mut seen_offsets,
            &mut verified_sites,
            &mut candidate_sites,
        );
    }

    for (idx, site) in verified_sites.iter_mut().enumerate() {
        site.index = idx + 1;
    }
    for (idx, site) in candidate_sites.iter_mut().enumerate() {
        site.index = idx + 1;
    }

    Ok(UsageAnalysis {
        verified_sites,
        candidate_sites,
        homonyms,
        unresolved_calls,
        file_sources,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_file_sites_from_cst(
    root: tree_sitter::Node<'_>,
    source: &str,
    abs_path: &Path,
    rel_path: &str,
    old_short: &str,
    enclosing_type: Option<&str>,
    defining_stem: &str,
    target_decl_ident: Option<crate::ast::edit::ByteSpan>,
    verified_lines: Option<&HashSet<usize>>,
    imports_target_module: bool,
    seen_offsets: &mut HashSet<(PathBuf, usize, usize)>,
    verified_sites: &mut Vec<RefSite>,
    candidate_sites: &mut Vec<RefSite>,
) {
    let lines: Vec<&str> = source.lines().collect();
    let src_bytes = source.as_bytes();
    walk_cst_node(
        root,
        source,
        src_bytes,
        &lines,
        abs_path,
        rel_path,
        old_short,
        enclosing_type,
        defining_stem,
        target_decl_ident,
        verified_lines,
        imports_target_module,
        false,
        seen_offsets,
        verified_sites,
        candidate_sites,
    );
}

#[allow(clippy::too_many_arguments)]
fn walk_cst_node(
    node: tree_sitter::Node<'_>,
    source: &str,
    src_bytes: &[u8],
    lines: &[&str],
    abs_path: &Path,
    rel_path: &str,
    old_short: &str,
    enclosing_type: Option<&str>,
    defining_stem: &str,
    target_decl_ident: Option<crate::ast::edit::ByteSpan>,
    verified_lines: Option<&HashSet<usize>>,
    imports_target_module: bool,
    shadowed_in_scope: bool,
    seen_offsets: &mut HashSet<(PathBuf, usize, usize)>,
    verified_sites: &mut Vec<RefSite>,
    candidate_sites: &mut Vec<RefSite>,
) {
    let kind = node.kind();

    // Never touch comments or string literals
    if matches!(
        kind,
        "line_comment"
            | "block_comment"
            | "comment"
            | "doc_comment"
            | "string"
            | "string_literal"
            | "raw_string_literal"
            | "char_literal"
            | "template_string"
            | "interpreted_string_literal"
            | "string_content"
    ) {
        return;
    }

    // Track local variable / parameter shadowing inside function/block scopes
    let mut local_shadow = shadowed_in_scope;
    if is_function_scope_kind(kind) {
        local_shadow = scope_defines_local_binding(node, src_bytes, old_short, target_decl_ident);
    }

    if is_identifier_kind(kind) && node.utf8_text(src_bytes).ok() == Some(old_short) {
        let start = node.start_byte();
        let end = node.end_byte();
        let line = node.start_position().row + 1;
        let col = node.start_position().column + 1;
        let snippet = lines
            .get(line.saturating_sub(1))
            .map(|l| l.trim().to_string())
            .unwrap_or_default();

        if seen_offsets.insert((abs_path.to_path_buf(), start, end)) {
            let make_site = |site_kind: &str, reason: &str| RefSite {
                index: 0,
                file: rel_path.to_string(),
                line,
                col,
                start_byte: start,
                end_byte: end,
                kind: site_kind.to_string(),
                reason: reason.to_string(),
                snippet: snippet.clone(),
            };

            if target_decl_ident
                .map(|d| d.start_byte == start && d.end_byte == end)
                .unwrap_or(false)
            {
                verified_sites.push(make_site("declaration", "Target symbol declaration"));
            } else if is_other_symbol_declaration_name(node)
                || is_module_qualifier_in_scoped_path(node)
            {
                // Separate symbol declaration or module qualifier prefix (`mod::sym`); never rename module prefix
            } else if is_import_specifier_node(node, source, defining_stem) {
                if imports_target_module {
                    verified_sites.push(make_site("import", "Import/use specifier referencing target module"));
                } else {
                    candidate_sites.push(make_site("import", "Import specifier from unverified module"));
                }
            } else if local_shadow {
                candidate_sites.push(make_site(
                    "shadowed_local",
                    "Local binding or parameter shadows target symbol in this scope",
                ));
            } else if is_member_access_target(node, source, enclosing_type, defining_stem) {
                candidate_sites.push(make_site(
                    "member_access",
                    "Member access receiver type is not statically verified",
                ));
            } else if is_verified_call_target(node, line, verified_lines, defining_stem, source) {
                let is_test_call = is_inside_test_scope(node, src_bytes);
                verified_sites.push(make_site(
                    if is_test_call { "test_call" } else { "call" },
                    "Verified bare/scoped call site matching CALLS/TESTS edge",
                ));
            } else if is_call_callee_position(node) {
                candidate_sites.push(make_site(
                    "unverified_call",
                    "Call expression without verified CALLS edge or import provenance",
                ));
            } else {
                candidate_sites.push(make_site(
                    "unverified_call",
                    "Identifier reference outside verified call-edge set",
                ));
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_cst_node(
            child,
            source,
            src_bytes,
            lines,
            abs_path,
            rel_path,
            old_short,
            enclosing_type,
            defining_stem,
            target_decl_ident,
            verified_lines,
            imports_target_module,
            local_shadow,
            seen_offsets,
            verified_sites,
            candidate_sites,
        );
    }
}

fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier" | "field_identifier" | "property_identifier" | "type_identifier"
    )
}

fn is_function_scope_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "arrow_function"
            | "function_expression"
    )
}

fn scope_defines_local_binding(
    fn_node: tree_sitter::Node<'_>,
    src: &[u8],
    target_name: &str,
    target_decl_ident: Option<crate::ast::edit::ByteSpan>,
) -> bool {
    // Do not treat the target symbol's own body as shadowed by its own function name
    if let Some(decl) = target_decl_ident {
        if decl.start_byte >= fn_node.start_byte() && decl.end_byte <= fn_node.end_byte() {
            return false;
        }
    }
    let mut stack = vec![fn_node];
    while let Some(curr) = stack.pop() {
        let k = curr.kind();
        if k == "let_declaration" || k == "variable_declarator" {
            if let Some(pat) = curr
                .child_by_field_name("pattern")
                .or_else(|| curr.child_by_field_name("name"))
            {
                if pat.utf8_text(src).ok() == Some(target_name) {
                    return true;
                }
            }
        } else if k == "assignment" {
            if let Some(left) = curr.child_by_field_name("left") {
                if left.kind() == "identifier" && left.utf8_text(src).ok() == Some(target_name) {
                    return true;
                }
            }
        } else if matches!(k, "parameter" | "required_parameter" | "optional_parameter" | "typed_parameter") {
            let text = curr.utf8_text(src).unwrap_or("");
            if text
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|tok| tok == target_name)
            {
                return true;
            }
        }
        let mut c = curr.walk();
        for ch in curr.children(&mut c) {
            if ch.kind() == "identifier"
                && matches!(k, "parameters" | "formal_parameters" | "short_var_declaration")
                && ch.utf8_text(src).ok() == Some(target_name)
            {
                return true;
            }
            if !is_function_scope_kind(ch.kind()) {
                stack.push(ch);
            }
        }
    }
    false
}

fn is_other_symbol_declaration_name(id_node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = id_node.parent() else {
        return false;
    };
    let is_decl_kind = matches!(
        parent.kind(),
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "class_declaration"
            | "class_definition"
            | "struct_item"
    );
    if is_decl_kind {
        return parent
            .child_by_field_name("name")
            .map(|n| n.id() == id_node.id())
            .unwrap_or(false);
    }
    false
}

fn is_import_specifier_node(id_node: tree_sitter::Node<'_>, source: &str, defining_stem: &str) -> bool {
    if is_module_qualifier_in_scoped_path(id_node) {
        return false;
    }

    let mut curr = id_node;
    while let Some(parent) = curr.parent() {
        let k = parent.kind();
        if k == "import_from_statement" {
            // In Python `from <module_name> import <name>`, never rename the `module_name` subtree
            if let Some(mod_node) = parent.child_by_field_name("module_name") {
                if id_node.start_byte() >= mod_node.start_byte()
                    && id_node.end_byte() <= mod_node.end_byte()
                {
                    return false;
                }
            }
        }
        if matches!(
            k,
            "use_declaration"
                | "import_statement"
                | "import_from_statement"
                | "import_spec"
                | "import_declaration"
        ) {
            let text = &source[parent.start_byte()..parent.end_byte()];
            return !defining_stem.is_empty()
                && (text.contains(defining_stem)
                    || text.contains("::")
                    || text.contains("import "));
        }
        curr = parent;
    }
    false
}

fn is_module_qualifier_in_scoped_path(id_node: tree_sitter::Node<'_>) -> bool {
    let mut curr = id_node;
    while let Some(parent) = curr.parent() {
        if matches!(parent.kind(), "scoped_identifier" | "scoped_use_list") {
            if parent
                .child_by_field_name("path")
                .map(|p| p.id() == curr.id())
                .unwrap_or(false)
            {
                return true;
            }
        }
        curr = parent;
    }
    false
}

fn is_member_access_target(
    id_node: tree_sitter::Node<'_>,
    source: &str,
    enclosing_type: Option<&str>,
    _defining_stem: &str,
) -> bool {
    let Some(parent) = id_node.parent() else {
        return false;
    };
    match parent.kind() {
        "field_expression" => {
            // `obj.foo` -> `field` field is `id_node`
            parent
                .child_by_field_name("field")
                .map(|f| f.id() == id_node.id())
                .unwrap_or(false)
        }
        "member_expression" => {
            parent
                .child_by_field_name("property")
                .map(|p| p.id() == id_node.id())
                .unwrap_or(false)
        }
        "attribute" => {
            // Python `obj.foo`
            parent
                .child_by_field_name("attribute")
                .map(|a| a.id() == id_node.id())
                .unwrap_or(false)
        }
        "selector_expression" => parent
            .child_by_field_name("field")
            .map(|f| f.id() == id_node.id())
            .unwrap_or(false),
        "field_initializer" | "shorthand_field_initializer" => true,
        "scoped_identifier" => {
            // Static `Parent::foo` or `mod::foo` is NOT unverified member access when qualifier matches
            let scope_text = &source[parent.start_byte()..parent.end_byte()];
            if let Some(ty) = enclosing_type {
                if scope_text.starts_with(&format!("{ty}::")) || scope_text.starts_with("Self::") {
                    return false;
                }
            }
            false
        }
        _ => false,
    }
}

fn is_call_callee_position(id_node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = id_node.parent() else {
        return false;
    };
    if matches!(parent.kind(), "call_expression" | "call") {
        return parent
            .child_by_field_name("function")
            .map(|f| f.id() == id_node.id() || (f.kind() == "scoped_identifier" && f.child_by_field_name("name").map(|n| n.id() == id_node.id()).unwrap_or(false)))
            .unwrap_or(false);
    }
    if parent.kind() == "scoped_identifier" {
        if let Some(gp) = parent.parent() {
            if gp.kind() == "call_expression"
                && gp
                    .child_by_field_name("function")
                    .map(|f| f.id() == parent.id())
                    .unwrap_or(false)
                && parent
                    .child_by_field_name("name")
                    .map(|n| n.id() == id_node.id())
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    if parent.kind() == "token_tree" {
        // Inside Rust macro call e.g. `assert_eq!(foo(1), 2)`
        if let Some(next) = id_node.next_sibling() {
            if next.kind() == "token_tree" {
                return true;
            }
        }
    }
    false
}

fn is_verified_call_target(
    id_node: tree_sitter::Node<'_>,
    line: usize,
    verified_lines: Option<&HashSet<usize>>,
    _defining_stem: &str,
    _source: &str,
) -> bool {
    if !is_call_callee_position(id_node) {
        return false;
    }
    verified_lines
        .map(|set| set.contains(&line))
        .unwrap_or(false)
}

fn is_inside_test_scope(mut node: tree_sitter::Node<'_>, src: &[u8]) -> bool {
    while let Some(parent) = node.parent() {
        if is_function_scope_kind(parent.kind()) {
            if let Some(name) = parent.child_by_field_name("name").and_then(|n| n.utf8_text(src).ok()) {
                if name.starts_with("test_") || name.starts_with("Test") {
                    return true;
                }
            }
        }
        node = parent;
    }
    false
}

fn resolve_refactor_target(
    session: &WorkspaceSession,
    symbol: &str,
    file_hint: Option<&Path>,
) -> Result<ResolvedTarget> {
    let hint_str = file_hint.map(|p| p.to_string_lossy().replace('\\', "/"));

    let graph_node = if let Some(ref hint) = hint_str {
        session
            .graph
            .resolve_symbols(symbol, Some(hint))
            .into_iter()
            .find(|n| n.kind != NodeKind::File)
            .cloned()
            .or_else(|| {
                session
                    .graph
                    .resolve_symbols(symbol, None)
                    .into_iter()
                    .find(|n| n.kind != NodeKind::File)
                    .cloned()
            })
    } else {
        session
            .graph
            .resolve_symbols(symbol, None)
            .into_iter()
            .find(|n| n.kind != NodeKind::File)
            .cloned()
    };

    let target_path = if let Some(hint_path) = file_hint {
        session.validate_workspace_path(hint_path)?
    } else if let Some(ref gn) = graph_node {
        session.validate_workspace_path(&gn.location.file)?
    } else {
        anyhow::bail!(
            "Symbol '{}' not found in graph and no file hint was provided.",
            symbol
        );
    };

    let fresh_file = session.parse_fresh_file(&target_path)?;
    let fresh_symbol = fresh_file
        .resolve_symbol(symbol)
        .ok_or_else(|| {
            anyhow!(
                "Symbol '{}' not found in freshly parsed '{}'",
                symbol,
                fresh_file.abs_path.display()
            )
        })?
        .clone();

    let short_name = short_symbol_name(&fresh_symbol.name).to_string();

    Ok(ResolvedTarget {
        graph_node,
        fresh_file,
        fresh_symbol,
        short_name,
    })
}

fn short_symbol_name(name: &str) -> &str {
    name.rsplit(|c| c == '.' || c == ':' || c == '/')
        .next()
        .unwrap_or(name)
}

fn is_valid_identifier(ident: &str) -> bool {
    let mut chars = ident.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn to_rel_display(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

fn collapse_excess_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut consecutive_newlines = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                out.push('\n');
            }
        } else {
            if ch != '\r' {
                consecutive_newlines = 0;
            }
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rename_updates_declaration_and_verified_callers_across_files_ignoring_comments_and_strings(
    ) {
        let dir = tempfile::tempdir().unwrap();
        let math_file = dir.path().join("src").join("math.rs");
        let app_file = dir.path().join("src").join("app.rs");
        std::fs::create_dir_all(math_file.parent().unwrap()).unwrap();

        std::fs::write(
            &math_file,
            "// compute_tax calculates VAT\npub fn compute_tax(v: i32) -> i32 {\n    let label = \"compute_tax\";\n    let _ = label;\n    v * 2\n}\n",
        )
        .unwrap();
        std::fs::write(
            &app_file,
            "use crate::math::compute_tax;\n// note about compute_tax\npub fn run() -> i32 {\n    let tag = \"compute_tax\";\n    let _ = tag;\n    compute_tax(10)\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let outcome = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "compute_tax".to_string(),
                action: RefactorAction::Rename,
                new_name: Some("calculate_vat".to_string()),
                file: Some(math_file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.status, "applied", "Unexpected blockers: {:?}", outcome.blockers);
        assert!(!outcome.is_error);
        assert_eq!(outcome.files_modified.len(), 2);

        let math_after = std::fs::read_to_string(&math_file).unwrap();
        let app_after = std::fs::read_to_string(&app_file).unwrap();
        assert!(math_after.contains("pub fn calculate_vat(v: i32)"));
        assert!(math_after.contains("// compute_tax calculates VAT"), "Comments must remain untouched");
        assert!(math_after.contains("\"compute_tax\""), "Strings must remain untouched");

        assert!(app_after.contains("use crate::math::calculate_vat;"));
        assert!(app_after.contains("calculate_vat(10)"));
        assert!(app_after.contains("// note about compute_tax"), "Comments must remain untouched");
        assert!(app_after.contains("\"compute_tax\""), "Strings must remain untouched");
    }

    #[tokio::test]
    async fn test_rename_blocks_on_homonym_or_ambiguous_member_access_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let a_file = dir.path().join("src").join("alpha.rs");
        let b_file = dir.path().join("src").join("beta.rs");
        std::fs::create_dir_all(a_file.parent().unwrap()).unwrap();

        std::fs::write(
            &a_file,
            "pub fn process_item(x: i32) -> i32 {\n    x + 1\n}\n",
        )
        .unwrap();
        // Second file has BOTH a homonym definition AND an unverified member access `svc.process_item()`
        let b_orig = "pub fn call_member(svc: &Other) -> i32 {\n    svc.process_item()\n}\npub fn caller() -> i32 {\n    process_item(5)\n}\n";
        std::fs::write(&b_file, b_orig).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let blocked = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "process_item".to_string(),
                action: RefactorAction::Rename,
                new_name: Some("transform_item".to_string()),
                file: Some(a_file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(blocked.status, "blocked_ambiguous_rename");
        assert!(blocked.is_error);
        assert!(!blocked.candidate_sites.is_empty());
        assert_eq!(std::fs::read_to_string(&b_file).unwrap(), b_orig);

        let forced = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "process_item".to_string(),
                action: RefactorAction::Rename,
                new_name: Some("transform_item".to_string()),
                file: Some(a_file.clone()),
                dry_run: false,
                force: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(forced.status, "applied");
        assert!(std::fs::read_to_string(&a_file).unwrap().contains("pub fn transform_item("));
        // Unverified member access `svc.process_item()` must remain untouched even with force
        assert!(std::fs::read_to_string(&b_file).unwrap().contains("svc.process_item()"));
    }

    #[tokio::test]
    async fn test_rename_rolls_back_all_files_if_second_file_fails_syntax_check() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("src").join("one.rs");
        let file2 = dir.path().join("src").join("two.rs");
        std::fs::create_dir_all(file1.parent().unwrap()).unwrap();

        let orig1 = "pub fn shared_op() -> i32 {\n    1\n}\n";
        // Pre-existing broken syntax elsewhere in `two.rs` ensures multi-file Stage 1 syntax gate aborts ALL files
        let orig2 = "pub fn consumer() -> i32 {\n    shared_op()\n}\npub fn broken( {\n";
        std::fs::write(&file1, orig1).unwrap();
        std::fs::write(&file2, orig2).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let aborted = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "shared_op".to_string(),
                action: RefactorAction::Rename,
                new_name: Some("renamed_op".to_string()),
                file: Some(file1.clone()),
                dry_run: false,
                force: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(aborted.status, "syntax_error");
        assert!(aborted.is_error);
        // File 1 must remain completely untouched when File 2 fails validation
        assert_eq!(std::fs::read_to_string(&file1).unwrap(), orig1);
        assert_eq!(std::fs::read_to_string(&file2).unwrap(), orig2);
    }

    #[tokio::test]
    async fn test_safe_delete_blocks_when_callers_or_tests_or_entrypoint_exist() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("mod.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();

        let src = "fn callee() -> i32 {\n    7\n}\n\nfn main() {\n    let _ = callee();\n}\n";
        std::fs::write(&file, src).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        // 1. Delete callee blocked by active caller (`main`)
        let block_caller = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "callee".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(block_caller.status, "blocked_active_dependents");
        assert!(block_caller.is_error);

        // 2. Delete `main` blocked by entrypoint guard
        let block_main = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "main".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(block_main.status, "blocked_active_dependents");
        assert!(block_main.is_error);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), src);
    }

    #[tokio::test]
    async fn test_safe_delete_removes_symbol_and_leading_attributes_when_unreferenced() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("cleanup.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();

        std::fs::write(
            &file,
            "pub fn keep_me() -> i32 {\n    1\n}\n\n#[inline]\n/// Legacy helper\nfn dead_helper() -> i32 {\n    99\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let outcome = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "dead_helper".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.status, "applied", "Blockers: {:?}", outcome.blockers);
        assert!(!outcome.is_error);
        let after = std::fs::read_to_string(&file).unwrap();
        assert!(!after.contains("dead_helper"));
        assert!(!after.contains("Legacy helper"));
        assert!(!after.contains("#[inline]"));
        assert!(after.contains("pub fn keep_me()"));
        assert!(session.graph.resolve_first_symbol("dead_helper", None).is_none());
    }

    #[tokio::test]
    async fn test_rename_preserves_module_path_when_symbol_matches_module_name() {
        let dir = tempfile::tempdir().unwrap();
        let math_file = dir.path().join("src").join("math.rs");
        let app_file = dir.path().join("src").join("app.rs");
        std::fs::create_dir_all(math_file.parent().unwrap()).unwrap();

        // Symbol name `math` is identical to its defining file/module stem `math.rs`
        std::fs::write(&math_file, "pub fn math(x: i32) -> i32 {\n    x + 1\n}\n").unwrap();
        std::fs::write(
            &app_file,
            "use crate::math::math;\npub fn run() -> i32 {\n    math(5)\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let outcome = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "math".to_string(),
                action: RefactorAction::Rename,
                new_name: Some("compute_math".to_string()),
                file: Some(math_file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.status, "applied", "Blockers: {:?}", outcome.blockers);
        let app_after = std::fs::read_to_string(&app_file).unwrap();
        assert!(
            app_after.contains("use crate::math::compute_math;"),
            "Module path 'crate::math' must stay intact, got: {app_after}"
        );
        assert!(app_after.contains("compute_math(5)"));
    }

    #[tokio::test]
    async fn test_safe_delete_guards_typescript_implements_and_allows_private_helper() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("gateway.ts");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();

        let src = concat!(
            "interface PaymentGateway { charge(cents: number): boolean; }\n",
            "class StripePay implements PaymentGateway {\n",
            "  charge(cents: number): boolean { return cents > 0; }\n",
            "  private debugTrace(): number { return 1; }\n",
            "}\n"
        );
        std::fs::write(&file, src).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let blocked = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "StripePay.charge".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(blocked.status, "blocked_active_dependents");
        assert!(
            blocked.blockers.iter().any(|b| b.contains("PaymentGateway")),
            "Expected PaymentGateway blocker, got: {:?}",
            blocked.blockers
        );

        let allowed_private = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "StripePay.debugTrace".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            allowed_private.status, "applied",
            "Private helper should delete safely, blockers: {:?}",
            allowed_private.blockers
        );
        let after = std::fs::read_to_string(&file).unwrap();
        assert!(!after.contains("debugTrace"));
        assert!(after.contains("charge(cents: number)"));
    }

    #[tokio::test]
    async fn test_safe_delete_guards_java_override_and_implements() {
        let dir = tempfile::tempdir().unwrap();
        let iface_file = dir.path().join("Job.java");
        let file = dir.path().join("Worker.java");

        std::fs::write(&iface_file, "interface Job { void run(); }\n").unwrap();
        let src = concat!(
            "class Worker implements Job {\n",
            "  @Override\n",
            "  public void run() {}\n",
            "  private void unusedLocalHelper() {}\n",
            "}\n"
        );
        std::fs::write(&file, src).unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let blocked = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "Worker.run".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(blocked.status, "blocked_active_dependents");
        assert!(
            blocked.blockers.iter().any(|b| b.contains("@Override")),
            "Expected @Override blocker, got: {:?}",
            blocked.blockers
        );

        let allowed = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "Worker.unusedLocalHelper".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(allowed.status, "applied", "Blockers: {:?}", allowed.blockers);
    }

    #[tokio::test]
    async fn test_safe_delete_guards_go_interface_cross_file_and_allows_unmatched() {
        let dir = tempfile::tempdir().unwrap();
        let iface_file = dir.path().join("iface.go");
        let store_file = dir.path().join("store.go");

        std::fs::write(
            &iface_file,
            "package store\n\ntype Saver interface {\n\tSave(key string, val int) error\n}\n",
        )
        .unwrap();
        std::fs::write(
            &store_file,
            "package store\n\ntype MemStore struct{}\n\nfunc (m *MemStore) Save(key, val string) error {\n\treturn nil\n}\n\nfunc (m *MemStore) unusedDebug() int {\n\treturn 42\n}\n",
        )
        .unwrap();

        let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
        session.ensure_indexed(None, 100).unwrap();

        let blocked = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "Save".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(store_file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(blocked.status, "blocked_active_dependents");
        assert!(
            blocked.blockers.iter().any(|b| b.contains("Saver.Save")),
            "Expected Saver.Save interface blocker, got: {:?}",
            blocked.blockers
        );

        let allowed = execute_refactor_symbol(
            &mut session,
            RefactorSymbolRequest {
                symbol: "unusedDebug".to_string(),
                action: RefactorAction::SafeDelete,
                new_name: None,
                file: Some(store_file.clone()),
                dry_run: false,
                force: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(allowed.status, "applied", "Blockers: {:?}", allowed.blockers);
        let after = std::fs::read_to_string(&store_file).unwrap();
        assert!(!after.contains("unusedDebug"));
        assert!(after.contains("func (m *MemStore) Save"));
    }
}
