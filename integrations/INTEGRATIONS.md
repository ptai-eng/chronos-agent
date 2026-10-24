# CHRONOS-AGENT — Universal Multi-Agent & CLI Ecosystem Guide

`chronos-mcp` speaks standard **MCP JSON-RPC 2.0 (`v2024-11-05`)** over `stdio`, `chronos init` auto-scaffolds native rules/skills/plugins/MCP configs from the 5D Knowledge Graph, and `chronos schema` exports strict tool schemas for **OpenAI Responses API**, **OpenAI Function Calling**, **Anthropic Claude API**, and **Google Gemini / Antigravity**.

---

## 0. One-Command Multi-Agent Bootstrap (`chronos init`)

Instead of manually editing JSON/TOML files or writing static prompts, run `chronos init` at the root of any repository to parse the 5D Knowledge Graph and generate live, data-backed configurations for your AI coding CLI or IDE:

```bash
# Scaffold ALL supported AI CLIs & IDEs at once (16 files):
chronos init --client all

# Or target a specific CLI / IDE ecosystem:
chronos init --client codex        # AGENTS.md + .codex/config.toml
chronos init --client antigravity  # GEMINI.md + .agents/skills/chronos-agent/SKILL.md + .agents/plugins/chronos-agent/
chronos init --client claude       # CLAUDE.md + .mcp.json + .claude/commands/chronos-audit.md
chronos init --client opencode     # opencode.json + .opencode/agent/chronos-auditor.md
chronos init --client cursor       # .cursor/mcp.json + .cursor/rules/chronos-5d.mdc
chronos init --client windsurf     # .windsurf/mcp.json + .windsurfrules
chronos init --client copilot      # .vscode/mcp.json + .github/copilot-instructions.md
chronos init --client cline        # .clinerules
```

| Target CLI / IDE | Native Context / Rule / Skill Files Synthesized | Native MCP Config File |
| :--- | :--- | :--- |
| **OpenAI Codex CLI & ChatGPT Codex** | [`AGENTS.md`](../AGENTS.md) | [`.codex/config.toml`](../.codex/config.toml) |
| **Google Antigravity (`agy` CLI & IDE)** | [`GEMINI.md`](../GEMINI.md) + [`.agents/skills/chronos-agent/SKILL.md`](../.agents/skills/chronos-agent/SKILL.md) | [`.agents/plugins/chronos-agent/mcp_config.json`](../.agents/plugins/chronos-agent/mcp_config.json) |
| **Anthropic Claude Code & Desktop** | [`CLAUDE.md`](../CLAUDE.md) + [`.claude/commands/chronos-audit.md`](../.claude/commands/chronos-audit.md) | [`.mcp.json`](../.mcp.json) |
| **OpenCode CLI (`opencode`)** | [`.opencode/agent/chronos-auditor.md`](../.opencode/agent/chronos-auditor.md) | [`opencode.json`](../opencode.json) |
| **Cursor IDE & Cursor Agent CLI** | [`.cursor/rules/chronos-5d.mdc`](../.cursor/rules/chronos-5d.mdc) (`alwaysApply: true`) | [`.cursor/mcp.json`](../.cursor/mcp.json) |
| **Windsurf Cascade** | [`.windsurfrules`](../.windsurfrules) | [`.windsurf/mcp.json`](../.windsurf/mcp.json) |
| **GitHub Copilot Coding Agent & VS Code** | [`.github/copilot-instructions.md`](../.github/copilot-instructions.md) | [`.vscode/mcp.json`](../.vscode/mcp.json) |
| **Cline & Roo Code** | [`.clinerules`](../.clinerules) | VS Code `cline.mcpServers` / `.roo/mcp.json` |

---

## 1. OpenAI Codex CLI & ChatGPT Codex (`AGENTS.md` + `.codex/config.toml`)

```bash
# Auto-scaffold AGENTS.md and .codex/config.toml:
chronos init --client codex

# Or register globally with Codex CLI:
codex mcp add chronos-agent -- chronos-mcp --workspace .
```

Example prompts inside `codex`:
```bash
codex "Use chronos_get_architecture and chronos_blast_radius to audit impact before refactoring verify_token"
codex "Run chronos_mutation_gate on src/services/orderService.ts and add unit assertions for any survived mutants"
```

---

## 2. Google Antigravity (`agy` CLI, Antigravity 2.0 IDE & Gemini CLI)

Google Antigravity automatically discovers project rules (`GEMINI.md`, `AGENTS.md`), progressive-disclosure workspace skills (`.agents/skills/<name>/SKILL.md`), and workspace plugins (`.agents/plugins/<name>/`).

```bash
# Auto-scaffold GEMINI.md + .agents/skills/chronos-agent/SKILL.md + .agents/plugins/chronos-agent/:
chronos init --client antigravity
```

To register globally across all workspaces on your machine, add to `~/.gemini/config/mcp_config.json`:
```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "chronos-mcp",
      "args": ["--workspace", "."]
    }
  }
}
```

---

## 3. Anthropic Claude Code (`claude` CLI) & Claude Desktop

Claude Code automatically loads `CLAUDE.md`, project-scoped `.mcp.json`, and custom slash commands in `.claude/commands/`.

```bash
# Auto-scaffold CLAUDE.md + .mcp.json + .claude/commands/chronos-audit.md:
chronos init --client claude

# Or register via Claude Code CLI:
claude mcp add chronos-agent -- chronos-mcp --workspace .
```

Inside Claude Code, invoke the bundled 5D slash command:
```text
/project:chronos-audit handle_blast_radius
```

---

## 4. OpenCode CLI (`opencode`)

OpenCode reads `opencode.json` at the repository root and discovers specialized subagents in `.opencode/agent/*.md`.

```bash
# Auto-scaffold opencode.json + .opencode/agent/chronos-auditor.md:
chronos init --client opencode
```

Generated `opencode.json`:
```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "chronos-agent": {
      "type": "local",
      "command": ["chronos-mcp", "--workspace", "."],
      "enabled": true
    }
  }
}
```

---

## 5. Cursor IDE & Cursor Agent CLI

Cursor discovers `.cursor/mcp.json` and `.cursor/rules/*.mdc` rules.

```bash
chronos init --client cursor
```

---

## 6. Windsurf, GitHub Copilot Coding Agent & Cline

```bash
chronos init --client windsurf   # Generates .windsurf/mcp.json + .windsurfrules
chronos init --client copilot    # Generates .vscode/mcp.json + .github/copilot-instructions.md
chronos init --client cline      # Generates .clinerules
```

---

## 7. Multi-Provider AI SDK Schema Exporter (`chronos schema`)

Building a custom agent with the **OpenAI Agents SDK**, **Anthropic Claude SDK**, or **Google Gemini / Vertex AI SDK**? Export all 11 CHRONOS tools in your provider's exact JSON schema format:

```bash
# OpenAI Responses API (strict: true):
chronos schema --format responses > openai_responses_tools.json

# OpenAI Chat Completions Function Calling (strict: true):
chronos schema --format openai > openai_chat_tools.json

# Anthropic Claude Messages API (input_schema):
chronos schema --format anthropic > anthropic_claude_tools.json

# Google Gemini / Vertex AI (functionDeclarations):
chronos schema --format gemini > google_gemini_tools.json

# Standard MCP tools/list JSON:
chronos schema --format mcp > mcp_tools.json
```

---

## 8. Smoke-Testing the MCP Server Over `stdio`

```bash
# 1. Initialize handshake
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootPath":"."}}' | chronos-mcp

# 2. List all 11 registered MCP tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | chronos-mcp

# 3. Synthesize Antigravity / Claude / Codex rules via MCP
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"chronos_generate_agents_md","arguments":{"target":"antigravity","project_name":"chronos-agent"}}}' | chronos-mcp
```
