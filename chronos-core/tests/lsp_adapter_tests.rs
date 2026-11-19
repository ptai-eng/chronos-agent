use chronos_core::ast::refactor::{execute_refactor_symbol, RefactorAction, RefactorSymbolRequest};
use chronos_core::lsp::{
    byte_offset_to_utf16_position, path_to_file_uri, resolve_references_hybrid,
    uri_to_relative_path, utf16_position_to_byte_offset,
};
use chronos_core::session::WorkspaceSession;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());
static MOCK_LSP_BIN: OnceLock<PathBuf> = OnceLock::new();

struct EnvGuard {
    saved_cmd: Option<String>,
    saved_args: Option<String>,
}

impl EnvGuard {
    fn set(cmd: &str, args: Option<&str>) -> Self {
        let saved_cmd = std::env::var("CHRONOS_LSP_COMMAND").ok();
        let saved_args = std::env::var("CHRONOS_LSP_ARGS").ok();
        std::env::set_var("CHRONOS_LSP_COMMAND", cmd);
        match args {
            Some(a) => std::env::set_var("CHRONOS_LSP_ARGS", a),
            None => std::env::remove_var("CHRONOS_LSP_ARGS"),
        }
        Self { saved_cmd, saved_args }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.saved_cmd {
            Some(v) => std::env::set_var("CHRONOS_LSP_COMMAND", v),
            None => std::env::remove_var("CHRONOS_LSP_COMMAND"),
        }
        match &self.saved_args {
            Some(v) => std::env::set_var("CHRONOS_LSP_ARGS", v),
            None => std::env::remove_var("CHRONOS_LSP_ARGS"),
        }
        for key in [
            "MOCK_LSP_REF_URI",
            "MOCK_LSP_REF_LINE",
            "MOCK_LSP_REF_COL",
            "MOCK_LSP_REF_END_COL",
        ] {
            std::env::remove_var(key);
        }
    }
}

const MOCK_LSP_SOURCE: &str = r##"
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

fn send(msg: &str) {
    let mut out = io::stdout().lock();
    let header = format!("Content-Length: {}\r\n\r\n", msg.len());
    let _ = out.write_all(header.as_bytes());
    let _ = out.write_all(msg.as_bytes());
    let _ = out.flush();
}

fn read_frame(reader: &mut BufReader<io::StdinLock<'_>>) -> Option<String> {
    let mut len: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.trim().eq_ignore_ascii_case("Content-Length") {
                len = v.trim().parse().ok()?;
            }
        }
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

fn extract_str(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\":\"", key);
    let idx = body.find(&pat)? + pat.len();
    let rest = &body[idx..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn main() {
    let hang = std::env::args().any(|a| a == "--hang");
    let refs_file = std::env::var("MOCK_LSP_REF_URI").unwrap_or_default();
    let refs_line: usize = std::env::var("MOCK_LSP_REF_LINE").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let refs_col: usize = std::env::var("MOCK_LSP_REF_COL").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let refs_end_col: usize = std::env::var("MOCK_LSP_REF_END_COL").ok().and_then(|s| s.parse().ok()).unwrap_or(refs_col + 5);

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut root_uri = String::new();

    while let Some(frame) = read_frame(&mut reader) {
        if frame.contains("\"method\":\"initialize\"") {
            if let Some(uri) = extract_str(&frame, "rootUri") {
                root_uri = uri;
            }
            if hang {
                std::thread::sleep(std::time::Duration::from_secs(30));
                break;
            }
            send(r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"referencesProvider":true}}}"#);
            // Send server->client workDoneProgress/create request to verify bidirectional handling
            send(r#"{"jsonrpc":"2.0","id":99,"method":"window/workDoneProgress/create","params":{"token":"idx-1"}}"#);
        } else if frame.contains("\"method\":\"initialized\"") {
            send(r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":"idx-1","value":{"kind":"begin","title":"Indexing"}}}"#);
            send(r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":"idx-1","value":{"kind":"end"}}}"#);
        } else if frame.contains("\"method\":\"textDocument/didOpen\"") {
            let doc_uri = extract_str(&frame, "uri").unwrap_or(root_uri.clone());
            let diag = format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{{\"uri\":\"{doc_uri}\",\"diagnostics\":[{{\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":0,\"character\":1}}}},\"message\":\"mock-lsp check passed\"}}]}}}}"
            );
            send(&diag);
        } else if frame.contains("\"method\":\"textDocument/references\"") {
            let doc_uri = extract_str(&frame, "uri").unwrap_or_default();
            let loc2 = if !refs_file.is_empty() && Path::new(&refs_file.trim_start_matches("file:///")).as_os_str().len() > 0 {
                format!(
                    ",{{\"uri\":\"{refs_file}\",\"range\":{{\"start\":{{\"line\":{refs_line},\"character\":{refs_col}}},\"end\":{{\"line\":{refs_line},\"character\":{refs_end_col}}}}}}}"
                )
            } else {
                String::new()
            };
            let resp = format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":[{{\"uri\":\"{doc_uri}\",\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":0,\"character\":1}}}}}}{loc2}]}}"
            );
            send(&resp);
        } else if frame.contains("\"method\":\"shutdown\"") {
            send(r#"{"jsonrpc":"2.0","id":3,"result":null}"#);
        } else if frame.contains("\"method\":\"exit\"") {
            break;
        }
    }
}
"##;

fn ensure_mock_lsp_binary() -> PathBuf {
    MOCK_LSP_BIN
        .get_or_init(|| {
            let build_dir = std::env::temp_dir().join("chronos_mock_lsp_bin");
            std::fs::create_dir_all(&build_dir).unwrap();
            let src_path = build_dir.join("mock_lsp.rs");
            let exe_name = if cfg!(windows) { "mock_lsp.exe" } else { "mock_lsp" };
            let exe_path = build_dir.join(exe_name);
            std::fs::write(&src_path, MOCK_LSP_SOURCE).unwrap();
            let status = std::process::Command::new("rustc")
                .arg(&src_path)
                .arg("-o")
                .arg(&exe_path)
                .status()
                .expect("rustc must be available during cargo test");
            assert!(status.success(), "Failed compiling mock LSP binary");
            exe_path
        })
        .clone()
}

#[test]
fn test_utf16_position_round_trip_with_emojis_and_crlf() {
    let src = "// 🚀 header\r\npub fn target_fn() -> i32 { 1 }\r\n";
    let byte_off = src.find("target_fn").unwrap();
    let (line_0, col_u16) = byte_offset_to_utf16_position(src, byte_off);
    assert_eq!(line_0, 1);
    assert_eq!(col_u16, 7);
    assert_eq!(utf16_position_to_byte_offset(src, line_0, col_u16), Some(byte_off));

    // Emoji on line 0 (`🚀` = 2 UTF-16 code units)
    let hdr_end = src.find("header").unwrap();
    let (l0, c0) = byte_offset_to_utf16_position(src, hdr_end);
    assert_eq!(l0, 0);
    assert_eq!(c0, 6); // "// " (3) + 🚀 (2) + " " (1)
    assert_eq!(utf16_position_to_byte_offset(src, 0, c0), Some(hdr_end));
}

#[test]
fn test_uri_round_trip_on_current_platform() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub dir").join("app.rs");
    std::fs::create_dir_all(sub.parent().unwrap()).unwrap();
    std::fs::write(&sub, "fn main() {}\n").unwrap();

    let uri = path_to_file_uri(&sub).unwrap();
    let rel = uri_to_relative_path(&uri, dir.path()).unwrap();
    assert_eq!(rel, "sub dir/app.rs");
}

#[tokio::test]
async fn test_lsp_hybrid_fallback_when_server_missing() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::set("Z:/definitely_nonexistent_chronos_lsp_binary_xyz", None);

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("src").join("mod.rs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "pub fn demo_sym() -> i32 { 7 }\n").unwrap();

    let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
    session.ensure_indexed(None, 50).unwrap();

    let res = resolve_references_hybrid(
        &session,
        &file,
        "pub fn demo_sym() -> i32 { 7 }\n",
        0,
        7,
        "demo_sym",
        Some("src/mod.rs"),
        Duration::from_secs(2),
    )
    .await;

    assert!(res.used_fallback);
    assert!(res.fallback_reason.is_some());
    assert_eq!(res.adapter_name, "chronos-ast-adapter");
    assert_eq!(res.coverage.completeness, "partial");
    assert!(!res.residual_unknowns.is_empty());
}

#[tokio::test]
async fn test_lsp_timeout_kills_hung_server_and_falls_back_gracefully() {
    let _lock = ENV_LOCK.lock().await;
    let mock_bin = ensure_mock_lsp_binary();
    let _guard = EnvGuard::set(&mock_bin.to_string_lossy(), Some("--hang"));

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("src").join("hang.rs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "pub fn slow_sym() -> i32 { 1 }\n").unwrap();

    let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
    session.ensure_indexed(None, 50).unwrap();

    let res = resolve_references_hybrid(
        &session,
        &file,
        "pub fn slow_sym() -> i32 { 1 }\n",
        0,
        7,
        "slow_sym",
        Some("src/hang.rs"),
        Duration::from_millis(350),
    )
    .await;

    assert!(res.used_fallback);
    let reason = res.fallback_reason.unwrap_or_default();
    assert!(
        reason.contains("timed out"),
        "Expected timeout reason, got: {reason}"
    );
}

#[tokio::test]
async fn test_lsp_end_to_end_mock_promotes_ambiguous_member_candidate_during_rename() {
    let _lock = ENV_LOCK.lock().await;
    let mock_bin = ensure_mock_lsp_binary();

    let dir = tempfile::tempdir().unwrap();
    let svc_file = dir.path().join("src").join("svc.rs");
    let app_file = dir.path().join("src").join("app.rs");
    std::fs::create_dir_all(svc_file.parent().unwrap()).unwrap();

    std::fs::write(&svc_file, "pub fn dispatch_job(x: i32) -> i32 {\n    x + 1\n}\n").unwrap();
    let app_src = "pub fn caller(svc: &Runner) -> i32 {\n    svc.dispatch_job()\n}\n";
    std::fs::write(&app_file, app_src).unwrap();

    let app_uri = path_to_file_uri(&app_file).unwrap();
    let byte_off = app_src.find("dispatch_job").unwrap();
    let (ref_line, ref_col) = byte_offset_to_utf16_position(app_src, byte_off);
    let ref_end_col = ref_col + "dispatch_job".chars().count() as u32;

    std::env::set_var("MOCK_LSP_REF_URI", &app_uri);
    std::env::set_var("MOCK_LSP_REF_LINE", ref_line.to_string());
    std::env::set_var("MOCK_LSP_REF_COL", ref_col.to_string());
    std::env::set_var("MOCK_LSP_REF_END_COL", ref_end_col.to_string());
    let _guard = EnvGuard::set(&mock_bin.to_string_lossy(), None);

    let mut session = WorkspaceSession::open_in_memory(dir.path().to_path_buf(), false);
    session.ensure_indexed(None, 100).unwrap();

    // Without LSP: ambiguous member access blocks without force
    let blocked = execute_refactor_symbol(
        &mut session,
        RefactorSymbolRequest {
            symbol: "dispatch_job".to_string(),
            action: RefactorAction::Rename,
            new_name: Some("execute_job".to_string()),
            file: Some(svc_file.clone()),
            dry_run: false,
            force: false,
            use_lsp: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(blocked.status, "blocked_ambiguous_rename");
    assert_eq!(blocked.candidate_sites.len(), 1);

    // With opt-in use_lsp: mock LSP promotes candidate -> verified edit & applies without force
    let applied = execute_refactor_symbol(
        &mut session,
        RefactorSymbolRequest {
            symbol: "dispatch_job".to_string(),
            action: RefactorAction::Rename,
            new_name: Some("execute_job".to_string()),
            file: Some(svc_file.clone()),
            dry_run: false,
            force: false,
            use_lsp: true,
        },
    )
    .await
    .unwrap();

    assert_eq!(applied.status, "applied", "Blockers: {:?}", applied.blockers);
    assert!(applied.candidate_sites.is_empty());
    assert!(
        applied
            .verified_edits
            .iter()
            .any(|s| s.kind == "lsp_verified_reference"),
        "Expected promoted lsp_verified_reference site, got: {:?}",
        applied.verified_edits
    );
    let svc_after = std::fs::read_to_string(&svc_file).unwrap();
    let app_after = std::fs::read_to_string(&app_file).unwrap();
    assert!(svc_after.contains("pub fn execute_job("));
    assert!(app_after.contains("svc.execute_job()"));
}
