//! Cross-platform Language Server Protocol binary discovery (`PATH` + Windows `.cmd` wrappers + env overrides).

use std::path::{Path, PathBuf};

/// Supported source language families for optional LSP hydration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Cpp,
    C,
    Java,
}

impl LspLanguage {
    /// Detect language family from file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "py" | "pyi" => Some(Self::Python),
            "go" => Some(Self::Go),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "c" | "h" => Some(Self::C),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Parse explicit `--lang` CLI flag values.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "rust" | "rs" => Some(Self::Rust),
            "typescript" | "ts" | "tsx" => Some(Self::TypeScript),
            "javascript" | "js" | "jsx" => Some(Self::JavaScript),
            "python" | "py" => Some(Self::Python),
            "go" | "golang" => Some(Self::Go),
            "cpp" | "c++" => Some(Self::Cpp),
            "c" => Some(Self::C),
            "java" => Some(Self::Java),
            _ => None,
        }
    }

    /// Standard LSP `textDocument.languageId` identifier.
    pub fn language_id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::Java => "java",
        }
    }

    fn env_override_key(self) -> &'static str {
        match self {
            Self::Rust => "CHRONOS_LSP_RUST",
            Self::TypeScript => "CHRONOS_LSP_TYPESCRIPT",
            Self::JavaScript => "CHRONOS_LSP_JAVASCRIPT",
            Self::Python => "CHRONOS_LSP_PYTHON",
            Self::Go => "CHRONOS_LSP_GO",
            Self::Cpp => "CHRONOS_LSP_CPP",
            Self::C => "CHRONOS_LSP_C",
            Self::Java => "CHRONOS_LSP_JAVA",
        }
    }

    fn candidate_binaries(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rust-analyzer"],
            Self::TypeScript | Self::JavaScript => &["typescript-language-server"],
            Self::Python => &["basedpyright-langserver", "pyright-langserver", "pylsp"],
            Self::Go => &["gopls"],
            Self::Cpp | Self::C => &["clangd"],
            Self::Java => &["jdtls"],
        }
    }

    fn default_args(self, binary_name: &str) -> &'static [&'static str] {
        let lower = binary_name.to_ascii_lowercase();
        if lower.contains("typescript-language-server")
            || lower.contains("pyright")
            || lower.contains("basedpyright")
        {
            &["--stdio"]
        } else {
            &[]
        }
    }
}

/// Discovered command specification ready for stdio spawn.
#[derive(Debug, Clone)]
pub struct LspCommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub language_id: &'static str,
    pub needs_cmd_shell: bool,
    pub source: &'static str,
}

impl LspCommandSpec {
    /// Build a `tokio::process::Command` wrapping Windows `.cmd`/`.bat` scripts via `cmd.exe /C`.
    pub fn to_tokio_command(&self, root_dir: &Path) -> tokio::process::Command {
        let mut cmd = if self.needs_cmd_shell && cfg!(windows) {
            let mut c = tokio::process::Command::new("cmd.exe");
            c.arg("/C").arg(&self.program);
            c.args(&self.args);
            c
        } else {
            let mut c = tokio::process::Command::new(&self.program);
            c.args(&self.args);
            c
        };
        cmd.current_dir(root_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        cmd
    }
}

/// Discover an LSP binary via env overrides (`CHRONOS_LSP_COMMAND` / `CHRONOS_LSP_<LANG>`) or `PATH`.
pub fn discover_lsp_command(lang: LspLanguage) -> Option<LspCommandSpec> {
    if let Ok(raw) = std::env::var("CHRONOS_LSP_COMMAND") {
        if let Some(spec) = build_env_override_spec(&raw, lang, "env:CHRONOS_LSP_COMMAND") {
            return Some(spec);
        }
    }
    if let Ok(raw) = std::env::var(lang.env_override_key()) {
        if let Some(spec) = build_env_override_spec(&raw, lang, lang.env_override_key()) {
            return Some(spec);
        }
    }

    let path_var = std::env::var_os("PATH")?;
    for base in lang.candidate_binaries() {
        if let Some(found) = find_on_path(&path_var, base) {
            let needs_cmd_shell = is_cmd_or_bat(&found);
            let args = lang
                .default_args(base)
                .iter()
                .map(|s| (*s).to_string())
                .collect();
            return Some(LspCommandSpec {
                program: found,
                args,
                language_id: lang.language_id(),
                needs_cmd_shell,
                source: "PATH",
            });
        }
    }
    None
}

fn build_env_override_spec(raw: &str, lang: LspLanguage, source: &'static str) -> Option<LspCommandSpec> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let extra_args: Vec<String> = std::env::var("CHRONOS_LSP_ARGS")
        .ok()
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    let direct_path = Path::new(trimmed);
    let (program, args) = if direct_path.exists() {
        (direct_path.to_path_buf(), extra_args)
    } else if !extra_args.is_empty() {
        (PathBuf::from(trimmed), extra_args)
    } else {
        let mut parts = trimmed.split_whitespace();
        let exe = parts.next()?;
        let rest = parts.map(ToString::to_string).collect();
        (PathBuf::from(exe), rest)
    };

    Some(LspCommandSpec {
        needs_cmd_shell: is_cmd_or_bat(&program),
        program,
        args,
        language_id: lang.language_id(),
        source,
    })
}

fn find_on_path(path_var: &std::ffi::OsStr, binary_base: &str) -> Option<PathBuf> {
    for dir in std::env::split_paths(path_var) {
        if cfg!(windows) {
            for ext in ["cmd", "bat", "exe", ""] {
                let candidate = if ext.is_empty() {
                    dir.join(binary_base)
                } else {
                    dir.join(format!("{binary_base}.{ext}"))
                };
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        } else {
            let candidate = dir.join(binary_base);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_cmd_or_bat(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false)
}
