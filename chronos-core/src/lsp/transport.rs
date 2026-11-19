//! Async `Content-Length`-framed JSON-RPC 2.0 transport over stdio pipes.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

const MAX_LSP_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Stdio framed JSON-RPC transport for an active child language server.
pub struct LspTransport {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl LspTransport {
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        Self {
            stdin,
            stdout: BufReader::new(stdout),
        }
    }

    /// Encode and flush a `Content-Length` framed JSON-RPC message.
    pub async fn send_message(&mut self, msg: &Value) -> Result<()> {
        let payload = serde_json::to_vec(msg).context("Failed to serialize LSP JSON-RPC payload")?;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        self.stdin
            .write_all(header.as_bytes())
            .await
            .context("Failed writing LSP header to child stdin")?;
        self.stdin
            .write_all(&payload)
            .await
            .context("Failed writing LSP body to child stdin")?;
        self.stdin
            .flush()
            .await
            .context("Failed flushing LSP message to child stdin")?;
        Ok(())
    }

    /// Read one `Content-Length` framed JSON-RPC message from child stdout.
    pub async fn read_message(&mut self) -> Result<Value> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = self
                .stdout
                .read_line(&mut line)
                .await
                .context("Failed reading LSP header line from child stdout")?;
            if bytes_read == 0 {
                return Err(anyhow!("Unexpected EOF waiting for LSP response header"));
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, val)) = trimmed.split_once(':') {
                if name.trim().eq_ignore_ascii_case("Content-Length") {
                    let parsed = val
                        .trim()
                        .parse::<usize>()
                        .with_context(|| format!("Invalid LSP Content-Length header: '{val}'"))?;
                    content_length = Some(parsed);
                }
            }
        }

        let len = content_length.ok_or_else(|| anyhow!("Missing LSP Content-Length header"))?;
        if len > MAX_LSP_FRAME_BYTES {
            bail!("LSP frame exceeds maximum allowed size ({len} bytes > {MAX_LSP_FRAME_BYTES})");
        }

        let mut body = vec![0u8; len];
        self.stdout
            .read_exact(&mut body)
            .await
            .with_context(|| format!("Failed reading {len}-byte LSP body"))?;
        let json: Value = serde_json::from_slice(&body)
            .context("Failed parsing LSP JSON-RPC response body")?;
        Ok(json)
    }
}
