//! `chronos lsp-hydrate` — On-demand hybrid compiler LSP inspection & fallback report.

use anyhow::Result;
use chronos_core::{
    ast::edit::FreshParsedFile,
    lsp::{
        byte_offset_to_utf16_position, discover_lsp_command, resolve_references_hybrid,
        LspLanguage, DEFAULT_LSP_HYDRATE_TIMEOUT,
    },
    session::WorkspaceSession,
};
use std::path::PathBuf;
use std::time::Duration;

pub async fn run(
    workspace: PathBuf,
    file: PathBuf,
    symbol: Option<String>,
    lang_override: Option<String>,
    timeout_secs: u64,
    json_out: bool,
) -> Result<()> {
    let mut session = WorkspaceSession::open(workspace.clone(), false)?;
    session.ensure_indexed(None, 400)?;

    let abs_file = session.validate_workspace_path(&file)?;
    let fresh = FreshParsedFile::from_disk(&abs_file)?;

    let lang = if let Some(ref l) = lang_override {
        LspLanguage::parse(l)
    } else {
        LspLanguage::from_path(&abs_file)
    };
    let discovered = lang.and_then(discover_lsp_command);

    let target_sym = symbol
        .as_deref()
        .and_then(|s| fresh.resolve_symbol(s).into_iter().next().cloned())
        .or_else(|| fresh.symbols.first().cloned());

    let (sym_name, line_zero, col_utf16) = if let Some(ref sym) = target_sym {
        let span = sym.name_ident_span.unwrap_or(sym.declaration_span);
        let (l, c) = byte_offset_to_utf16_position(&fresh.source, span.start_byte);
        (sym.name.clone(), l, c)
    } else {
        (symbol.clone().unwrap_or_else(|| "<file>".to_string()), 0, 0)
    };

    let rel_hint = abs_file
        .strip_prefix(&session.workspace_root)
        .unwrap_or(&abs_file)
        .to_string_lossy()
        .replace('\\', "/");

    let timeout = if timeout_secs == 0 {
        DEFAULT_LSP_HYDRATE_TIMEOUT
    } else {
        Duration::from_secs(timeout_secs)
    };

    let result = resolve_references_hybrid(
        &session,
        &abs_file,
        &fresh.source,
        line_zero,
        col_utf16,
        &sym_name,
        Some(&rel_hint),
        timeout,
    )
    .await;

    if json_out {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("🔌 CHRONOS Hybrid LSP Hydration Report\n");
    println!("  File:       {}", rel_hint);
    println!("  Symbol:     {} (line {}, col {})", sym_name, line_zero + 1, col_utf16 + 1);
    println!(
        "  Discovered: {}",
        discovered
            .as_ref()
            .map(|d| format!("{} ({})", d.program.display(), d.source))
            .unwrap_or_else(|| "none on PATH".to_string())
    );
    println!("  Adapter:    {}", result.adapter_name);
    println!("  Fallback:   {}", result.used_fallback);
    if let Some(ref reason) = result.fallback_reason {
        println!("  Reason:     {}", reason);
    }
    println!(
        "  Coverage:   {} (resolved_calls={}, residual_unknowns={})",
        result.coverage.completeness,
        result.coverage.resolved_calls,
        result.residual_unknowns.len()
    );
    println!("  References: {} site(s)", result.references.len());
    for r in result.references.iter().take(20) {
        println!("    - {}:{}:{}", r.file, r.line, r.column);
    }
    if !result.diagnostics.is_empty() {
        println!("  Diagnostics: {}", result.diagnostics.len());
        for d in result.diagnostics.iter().take(15) {
            println!("    * {d}");
        }
    }

    Ok(())
}
