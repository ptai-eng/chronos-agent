# CHRONOS-AGENT v1.5.0 — Universal AI Client & OpenAI Codex Integration Guide

`chronos-mcp` speaks standard **MCP JSON-RPC 2.0 (`v2024-11-05`)** over `stdio`, and `chronos schema` exports strict **OpenAI Function Calling / Responses API JSON Schemas** for custom agents.

---

## 0. Build or Install `chronos` & `chronos-mcp`

```bash
cargo install --path chronos-cli
cargo install --path chronos-mcp
# Verify installation:
chronos --version
```

---

## 1. OpenAI Codex CLI & ChatGPT Codex (`AGENTS.md` + `config.toml`)

### A. Register `chronos-mcp` with OpenAI Codex CLI (`openai/codex`)
Run the one-liner command:
```bash
codex mcp add chronos-agent -- chronos-mcp --workspace .
```

Or add directly to `~/.codex/config.toml` (generate via `chronos schema --format codex-toml`):
```toml
[mcp_servers.chronos-agent]
command = "chronos-mcp"
args = ["--workspace", "."]
```

### B. Synthesize a Live `AGENTS.md` from the 5D Knowledge Graph
OpenAI Codex automatically reads `AGENTS.md` at the root of your repository. Generate or refresh it from the 5D graph with one command:
```bash
chronos agents-md --write
```

### C. Example OpenAI Codex CLI Prompts
```bash
codex "Use chronos_get_architecture and chronos_blast_radius to audit impact before refactoring verify_token"
codex "Run chronos_mutation_gate on src/services/orderService.ts and add unit assertions for any survived mutants"
```

---

## 2. OpenAI Agents SDK & OpenAI Responses API (`strict: true`)

### A. Export OpenAI Responses API / Function Calling JSON Schema
```bash
# For OpenAI Responses API (gpt-5, o3, codex-mini):
chronos schema --format responses > openai_responses_tools.json

# For OpenAI Chat Completions API & Custom GPT Actions:
chronos schema --format openai > openai_chat_tools.json
```

### B. Connect `chronos-mcp` to the OpenAI Agents SDK (Python)
```python
import asyncio
from agents import Agent, Runner
from agents.mcp import MCPServerStdio

async def main():
    async with MCPServerStdio(
        name="chronos-agent",
        params={"command": "chronos-mcp", "args": ["--workspace", "."]},
    ) as chronos_server:
        agent = Agent(
            name="Codex Maintainer Agent",
            instructions="Always check chronos_blast_radius before modifying hub functions and verify changes with chronos_mutation_gate.",
            mcp_servers=[chronos_server],
        )
        result = await Runner.run(agent, "Find all O(N^2) loop hotspots and check their blast radius.")
        print(result.final_output)

asyncio.run(main())
```

---

## 3. Google Antigravity

Add to `~/.gemini/antigravity/mcp/` or `.agy/config.toml`:
```toml
[[mcp.servers]]
name = "chronos-agent"
command = "chronos-mcp"
args = ["--workspace", "${workspaceFolder}"]
```

---

## 4. Cursor

Create `.cursor/mcp.json` at the root of your project:
```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "chronos-mcp",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  }
}
```

---

## 5. Claude Code & Claude Desktop

Register via Claude Code CLI:
```bash
claude mcp add chronos-agent -- chronos-mcp --workspace .
```

Or add to `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "chronos-mcp",
      "args": ["--workspace", "/path/to/your/repo"]
    }
  }
}
```

---

## 6. OpenCode

Create `.opencode.toml` at your project root:
```toml
[mcp.servers.chronos]
command = "chronos-mcp"
args = ["--workspace", "."]
type = "local"
```

---

## 7. Cline (VS Code Extension)

Add to VS Code `settings.json`:
```json
{
  "cline.mcpServers": {
    "chronos-agent": {
      "command": "chronos-mcp",
      "args": ["--workspace", "${workspaceFolder}"],
      "disabled": false,
      "autoApprove": [
        "chronos_contract_status",
        "chronos_query_hotspots",
        "chronos_blast_radius",
        "chronos_get_architecture",
        "chronos_search_graph",
        "chronos_get_snippet"
      ]
    }
  }
}
```

---

## 8. Windsurf

Create `.windsurf/mcp.json`:
```json
{
  "mcpServers": {
    "chronos-agent": {
      "serverType": "stdio",
      "command": "chronos-mcp",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  }
}
```

---

## 9. Continue & Zed Editor

### Continue (`.continue/config.json`)
```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "transport": {
          "type": "stdio",
          "command": "chronos-mcp",
          "args": ["--workspace", "."]
        }
      }
    ]
  }
}
```

### Zed (`~/.config/zed/settings.json`)
```json
{
  "context_servers": {
    "chronos-agent": {
      "command": {
        "path": "chronos-mcp",
        "args": ["--workspace", "."],
        "env": {}
      }
    }
  }
}
```

---

## Smoke-Testing the MCP Server Over `stdio`

```bash
# 1. Initialize handshake
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootPath":"."}}' | chronos-mcp

# 2. List all 11 registered MCP tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | chronos-mcp

# 3. Synthesize OpenAI Codex AGENTS.md via MCP
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"chronos_generate_agents_md","arguments":{"project_name":"chronos-agent"}}}' | chronos-mcp
```
