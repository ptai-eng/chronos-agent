# CHRONOS Agent for Visual Studio Code

**5D Living Code Intelligence Harness & Real-Time Blast-Radius Protection for Autonomous AI Agents**

---

## ⚡ Features

- **Continuous 5D Telemetry on Save:** Automatically parses 7 language families (TypeScript/JavaScript, Python, Rust, Go, C/C++, Java) into the local 5D Knowledge Graph upon file save (`onDidSaveTextDocument`).
- **Interactive Blast-Radius Inspector:** Pinpoints downstream impacts, dependency call paths, and high-risk functions before code changes are committed.
- **Hotspot & Bottleneck Analysis:** Queries runtime p95/p99 latency hotspots directly from the embedded graph engine.
- **Zero-Friction MCP Integration:** Communicates with the local `chronos-mcp` binary via stdio JSON-RPC 2.0.

---

## 🚀 Quick Start

1. Install the extension from the VS Code Marketplace (or load the `.vsix` file).
2. Open a TypeScript or JavaScript workspace.
3. If `chronos-mcp` binary is not found on your system PATH, the extension will prompt you to configure or download the pre-compiled binary.
4. Use `Ctrl+Shift+P` (or `Cmd+Shift+P`) and type:
   - `CHRONOS: Analyze File`
   - `CHRONOS: Show Hotspots`
   - `CHRONOS: Blast Radius`

---

## ⚙️ Extension Settings

This extension contributes the following settings:

* `chronos.mcpBinaryPath`: Path to the `chronos-mcp` executable (e.g. `C:\tools\chronos-mcp.exe` or `/usr/local/bin/chronos-mcp`).

---

## 📄 License

MIT License. Engineered with ❤️ by [PTAI-ENG](https://github.com/ptai-eng/chronos-agent).
