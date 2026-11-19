//! On-demand hybrid `LspSemanticAdapter` implementing compiler-grade reference resolution
//! over stdio JSON-RPC with bounded timeouts and deterministic `AstSemanticAdapter` fallback.

use crate::blast_radius::impact_matrix::CoverageReport;
use crate::graph::query::{AstSemanticAdapter, SemanticReference, SemanticResolver};
use crate::lsp::discovery::{discover_lsp_command, LspLanguage};
use crate::lsp::transport::LspTransport;
use crate::session::WorkspaceSession;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncReadExt;

pub const DEFAULT_LSP_HYDRATE_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_LSP_QUERY_TIMEOUT: Duration = Duration::from_secs(15);

/// Result of running hybrid compiler-grade LSP reference resolution (or falling back to AST).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspHydrationResult {
    pub adapter_name: String,
    pub language_id: String,
    pub server_binary: Option<String>,
    pub used_fallback: bool,
    pub fallback_reason: Option<String>,
    pub diagnostics: Vec<String>,
    pub references: Vec<SemanticReference>,
    pub coverage: CoverageReport,
    pub residual_unknowns: Vec<String>,
}

/// On-demand single-shot LSP semantic resolver.
pub struct LspSemanticAdapter;

impl LspSemanticAdapter {
    /// Spawn the configured language server for `abs_file`, run `initialize -> references -> shutdown`,
    /// and convert UTF-16 LSP ranges back into UTF-8 byte spans inside `workspace_root`.
    pub async fn resolve_references_for_location(
        workspace_root: &Path,
        abs_file: &Path,
        source: &str,
        line_zero: u32,
        col_utf16: u32,
        timeout: Duration,
    ) -> Result<LspHydrationResult> {
        let lang = LspLanguage::from_path(abs_file).ok_or_else(|| {
            anyhow!(
                "No LSP language mapping for extension on '{}'",
                abs_file.display()
            )
        })?;
        let spec = discover_lsp_command(lang).ok_or_else(|| {
            anyhow!(
                "No LSP server binary found on PATH (or env override) for '{}'",
                lang.language_id()
            )
        })?;

        let server_binary = spec.program.to_string_lossy().to_string();
        let mut cmd = spec.to_tokio_command(workspace_root);
        let mut child = cmd.spawn().map_err(|e| {
            anyhow!(
                "Failed spawning LSP server '{}' ({}): {e}",
                server_binary,
                spec.source
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed capturing LSP stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed capturing LSP stdout"))?;
        let stderr_drain = child.stderr.take().map(|mut err| {
            tokio::spawn(async move {
                let mut sink = Vec::new();
                let _ = err.read_to_end(&mut sink).await;
            })
        });

        let run_future = run_lsp_session(
            LspTransport::new(stdin, stdout),
            workspace_root,
            abs_file,
            source,
            spec.language_id,
            &server_binary,
            line_zero,
            col_utf16,
        );

        let session_result = tokio::time::timeout(timeout, run_future).await;
        if let Some(handle) = stderr_drain {
            handle.abort();
        }
        // Always reap/terminate child on exit or timeout so zero background daemons linger.
        let _ = child.kill().await;
        let _ = child.wait().await;

        match session_result {
            Ok(inner) => inner,
            Err(_) => Err(anyhow!(
                "LSP server '{}' timed out after {}s",
                server_binary,
                timeout.as_secs()
            )),
        }
    }
}

/// Execute hybrid LSP resolution, transparently falling back to `AstSemanticAdapter` if
/// no LSP server is installed, startup fails, or the request exceeds `timeout`.
pub async fn resolve_references_hybrid(
    session: &WorkspaceSession,
    abs_file: &Path,
    source: &str,
    line_zero: u32,
    col_utf16: u32,
    symbol: &str,
    file_hint: Option<&str>,
    timeout: Duration,
) -> LspHydrationResult {
    match LspSemanticAdapter::resolve_references_for_location(
        &session.workspace_root,
        abs_file,
        source,
        line_zero,
        col_utf16,
        timeout,
    )
    .await
    {
        Ok(res) => res,
        Err(err) => {
            let ast = AstSemanticAdapter::new(&session.graph).resolve_references(symbol, file_hint);
            let (default_cov, default_unknowns, _) = CoverageReport::from_graph(&session.graph);
            let mut coverage = ast.coverage.unwrap_or(default_cov);
            coverage.completeness = "partial".to_string();
            let mut residual_unknowns = if ast.residual_unknowns.is_empty() {
                default_unknowns
            } else {
                ast.residual_unknowns
            };
            residual_unknowns.push(format!("LSP fallback to AST adapter: {err}"));
            let lang_id = LspLanguage::from_path(abs_file)
                .map(|l| l.language_id())
                .unwrap_or("unknown")
                .to_string();
            LspHydrationResult {
                adapter_name: ast.adapter_name,
                language_id: lang_id,
                server_binary: None,
                used_fallback: true,
                fallback_reason: Some(err.to_string()),
                diagnostics: ast.compiler_diagnostics,
                references: ast.references,
                coverage,
                residual_unknowns,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_lsp_session(
    mut transport: LspTransport,
    workspace_root: &Path,
    abs_file: &Path,
    source: &str,
    language_id: &str,
    server_binary: &str,
    line_zero: u32,
    col_utf16: u32,
) -> Result<LspHydrationResult> {
    let root_uri = path_to_file_uri(workspace_root)?;
    let doc_uri = path_to_file_uri(abs_file)?;

    let mut active_progress: HashSet<String> = HashSet::new();
    let mut diagnostics: Vec<String> = Vec::new();

    // 1. `initialize`
    transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "window": { "workDoneProgress": true },
                    "textDocument": {
                        "references": { "dynamicRegistration": false },
                        "publishDiagnostics": {}
                    }
                },
                "workspaceFolders": [{
                    "uri": root_uri,
                    "name": workspace_root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("workspace")
                }]
            }
        }))
        .await?;

    let _init_resp = read_until_response_id(
        &mut transport,
        1,
        workspace_root,
        &mut active_progress,
        &mut diagnostics,
    )
    .await?;

    // 2. `initialized` + `textDocument/didOpen`
    transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))
        .await?;

    transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": doc_uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": source
                }
            }
        }))
        .await?;

    // 3. `textDocument/references` (drains $/progress and publishDiagnostics en route)
    transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/references",
            "params": {
                "textDocument": { "uri": doc_uri },
                "position": { "line": line_zero, "character": col_utf16 },
                "context": { "includeDeclaration": true }
            }
        }))
        .await?;

    let refs_msg = read_until_response_id(
        &mut transport,
        2,
        workspace_root,
        &mut active_progress,
        &mut diagnostics,
    )
    .await?;

    if let Some(err) = refs_msg.get("error") {
        return Err(anyhow!("LSP textDocument/references returned error: {err}"));
    }

    let references = parse_lsp_locations(
        refs_msg.get("result").unwrap_or(&Value::Null),
        workspace_root,
    );

    // 4. Best-effort graceful shutdown before drop/reap
    let _ = transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "shutdown",
            "params": Value::Null
        }))
        .await;
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        read_until_response_id(
            &mut transport,
            3,
            workspace_root,
            &mut active_progress,
            &mut diagnostics,
        ),
    )
    .await;
    let _ = transport
        .send_message(&json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": Value::Null
        }))
        .await;

    let mut residual_unknowns = Vec::new();
    let coverage = CoverageReport {
        call_edges_available: !references.is_empty(),
        completeness: if active_progress.is_empty() {
            "complete".to_string()
        } else {
            "partial".to_string()
        },
        resolved_calls: references.len(),
        unresolved_calls: 0,
        telemetry_hydrated: false,
    };
    if !active_progress.is_empty() {
        residual_unknowns.push(format!(
            "{} LSP background workDoneProgress token(s) were still active",
            active_progress.len()
        ));
    }

    Ok(LspHydrationResult {
        adapter_name: format!("chronos-lsp-adapter ({language_id})"),
        language_id: language_id.to_string(),
        server_binary: Some(server_binary.to_string()),
        used_fallback: false,
        fallback_reason: None,
        diagnostics,
        references,
        coverage,
        residual_unknowns,
    })
}

async fn read_until_response_id(
    transport: &mut LspTransport,
    expected_id: i64,
    workspace_root: &Path,
    active_progress: &mut HashSet<String>,
    diagnostics: &mut Vec<String>,
) -> Result<Value> {
    loop {
        let msg = transport.read_message().await?;

        // Match response for `expected_id` (no `method` field, matching `id`).
        if msg.get("method").is_none() {
            if msg.get("id").and_then(|v| v.as_i64()) == Some(expected_id) {
                return Ok(msg);
            }
            continue;
        }

        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        // Acknowledge server-initiated requests cleanly so real servers (`rust-analyzer`, `gopls`) never stall.
        if let Some(server_req_id) = msg.get("id") {
            let reply_result = if method == "workspace/configuration" {
                json!([])
            } else {
                Value::Null
            };
            let _ = transport
                .send_message(&json!({
                    "jsonrpc": "2.0",
                    "id": server_req_id,
                    "result": reply_result
                }))
                .await;
            continue;
        }

        // Track `$/progress` begin/end tokens and `textDocument/publishDiagnostics`.
        if method == "$/progress" {
            if let Some(params) = msg.get("params") {
                let token = params
                    .get("token")
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                let kind = params
                    .get("value")
                    .and_then(|v| v.get("kind"))
                    .and_then(|k| k.as_str())
                    .unwrap_or("");
                if kind == "begin" && !token.is_empty() {
                    active_progress.insert(token);
                } else if kind == "end" && !token.is_empty() {
                    active_progress.remove(&token);
                }
            }
        } else if method == "textDocument/publishDiagnostics" {
            if let Some(params) = msg.get("params") {
                let file_rel = params
                    .get("uri")
                    .and_then(|u| u.as_str())
                    .and_then(|u| uri_to_relative_path(u, workspace_root))
                    .unwrap_or_else(|| "<external>".to_string());
                if let Some(arr) = params.get("diagnostics").and_then(|d| d.as_array()) {
                    for d in arr {
                        let line = d
                            .pointer("/range/start/line")
                            .and_then(|l| l.as_u64())
                            .unwrap_or(0)
                            + 1;
                        let text = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                        if !text.is_empty() {
                            diagnostics.push(format!("{file_rel}:{line}: {text}"));
                        }
                    }
                }
            }
        }
    }
}

fn parse_lsp_locations(result: &Value, workspace_root: &Path) -> Vec<SemanticReference> {
    let Some(items) = result.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len());
    let mut source_cache: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();

    for loc in items {
        // Supports both `Location` (`uri` + `range`) and `LocationLink` (`targetUri` + `targetSelectionRange`).
        let uri_opt = loc
            .get("uri")
            .or_else(|| loc.get("targetUri"))
            .and_then(|u| u.as_str());
        let range_opt = loc
            .get("range")
            .or_else(|| loc.get("targetSelectionRange"))
            .or_else(|| loc.get("targetRange"));

        let (Some(uri), Some(range)) = (uri_opt, range_opt) else {
            continue;
        };
        let Some(rel_file) = uri_to_relative_path(uri, workspace_root) else {
            continue;
        };

        let start_line_0 = range
            .pointer("/start/line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let start_col_0 = range
            .pointer("/start/character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let end_line_0 = range
            .pointer("/end/line")
            .and_then(|v| v.as_u64())
            .unwrap_or(start_line_0 as u64) as u32;
        let end_col_0 = range
            .pointer("/end/character")
            .and_then(|v| v.as_u64())
            .unwrap_or(start_col_0 as u64) as u32;

        let cached_src = source_cache
            .entry(rel_file.clone())
            .or_insert_with(|| std::fs::read_to_string(workspace_root.join(&rel_file)).ok());

        let (start_byte, end_byte) = if let Some(src) = cached_src.as_deref() {
            (
                utf16_position_to_byte_offset(src, start_line_0, start_col_0),
                utf16_position_to_byte_offset(src, end_line_0, end_col_0),
            )
        } else {
            (None, None)
        };

        out.push(SemanticReference {
            file: rel_file,
            line: start_line_0 as usize + 1,
            column: start_col_0 as usize + 1,
            start_byte,
            end_byte,
            is_definition: false,
        });
    }
    out
}

/// Convert a local filesystem path into a canonical `file://` URI (stripping Windows `\\?\` prefix).
pub fn path_to_file_uri(path: &Path) -> Result<String> {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let normalized = strip_windows_verbatim_prefix(&abs);
    url::Url::from_file_path(&normalized)
        .map(|u| u.to_string())
        .map_err(|_| anyhow!("Failed converting path '{}' to file URI", normalized.display()))
}

/// Convert an LSP `file://` URI into a forward-slash path relative to `workspace_root`.
pub fn uri_to_relative_path(uri: &str, workspace_root: &Path) -> Option<String> {
    let parsed = url::Url::parse(uri).ok()?;
    let file_path = parsed.to_file_path().ok()?;
    let norm_file = normalize_for_prefix(&file_path);
    let norm_root = normalize_for_prefix(workspace_root);
    let rel = norm_file.strip_prefix(&norm_root).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn normalize_for_prefix(path: &Path) -> PathBuf {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    strip_windows_verbatim_prefix(&canon)
}

fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Convert a UTF-8 byte offset in `source` into 0-based LSP `(line, utf16_character)`.
pub fn byte_offset_to_utf16_position(source: &str, byte_offset: usize) -> (u32, u32) {
    let clamped = byte_offset.min(source.len());
    let prefix = &source[..clamped];
    let mut line = 0u32;
    let mut last_line_start = 0usize;
    for (idx, b) in prefix.bytes().enumerate() {
        if b == b'\n' {
            line += 1;
            last_line_start = idx + 1;
        }
    }
    let line_slice = &prefix[last_line_start..];
    let utf16_col: u32 = line_slice.chars().map(|c| c.len_utf16() as u32).sum();
    (line, utf16_col)
}

/// Convert a 0-based LSP `(line, utf16_character)` position into a UTF-8 byte offset in `source`.
pub fn utf16_position_to_byte_offset(
    source: &str,
    target_line_zero: u32,
    target_utf16_col: u32,
) -> Option<usize> {
    let mut current_line = 0u32;
    let mut line_start = 0usize;

    if target_line_zero > 0 {
        for (idx, b) in source.bytes().enumerate() {
            if b == b'\n' {
                current_line += 1;
                if current_line == target_line_zero {
                    line_start = idx + 1;
                    break;
                }
            }
        }
        if current_line != target_line_zero {
            return None;
        }
    }

    let rest = &source[line_start..];
    let line_end_rel = rest.find('\n').unwrap_or(rest.len());
    let line_str = rest[..line_end_rel].trim_end_matches('\r');

    let mut utf16_acc = 0u32;
    for (byte_rel, ch) in line_str.char_indices() {
        if utf16_acc >= target_utf16_col {
            return Some(line_start + byte_rel);
        }
        utf16_acc += ch.len_utf16() as u32;
    }
    if utf16_acc == target_utf16_col {
        Some(line_start + line_str.len())
    } else {
        None
    }
}
