# Contributing to CHRONOS-AGENT

Thanks for helping improve CHRONOS-AGENT.

## Prerequisites

- **Rust toolchain** (1.85+ via `rustup`)
- **CMake & C++ compiler** (MSVC Build Tools on Windows, GCC/Clang on Linux, Xcode CLT on macOS) for embedded `kuzu` 0.11.3 C++ FFI
- **Node.js 18+** (only if modifying `npm/chronos-mcp/` or `chronos-vscode/`)

## Local Setup & Testing

```bash
# Run the full workspace test suite (recommended -j 4 concurrency on 8-core machines)
cargo test --workspace -j 4

# Build CLI & MCP binaries
cargo build -p chronos-cli -p chronos-mcp -j 4
```

## Core Engineering Invariants

1. **Never mutate files from cached graph byte offsets.** All edits and refactors must read current disk bytes and re-parse with Tree-sitter immediately (`WorkspaceSession::parse_fresh_file`).
2. **Preserve BOM and line endings.** Write through `AtomicFileWriter` so `CRLF`/`LF`, UTF-8 BOM, and rollback guarantees hold across every file mutation.
3. **Keep `ScriptProfile::ReadOnly` strictly read-only.** Never register mutating functions in the MCP `chronos_explore.script` Rhai engine or when `--readonly` is enabled.
4. **Canonical tool schemas live in one place.** Edit [`chronos-core/src/registry.rs`](./chronos-core/src/registry.rs) first and keep `test_zero_schema_drift_across_mcp_openai_responses_anthropic_and_gemini` green.

## Good Areas to Contribute

- **Hybrid LSP Adapter (`v1.8.1`):** See [`docs/ROADMAP-v1.8-SERENA-EXPANSION.md`](./docs/ROADMAP-v1.8-SERENA-EXPANSION.md).
- **Language-Specific Refactoring Guards:** Extending trait/interface implementation detection to Go, Java, and TypeScript in `chronos-core/src/ast/edit.rs`.
- **Reproducible Benchmarks:** Measuring token and tool-call savings on medium-sized polyglot repositories.
