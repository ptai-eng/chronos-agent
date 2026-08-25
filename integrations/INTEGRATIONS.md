# CHRONOS-AGENT — Integrations Guide

Sau khi build `chronos-mcp` binary, file này hướng dẫn cách kết nối với từng AI client.

## Build binary

```powershell
cd chronos-agent
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;$env:PATH"
cargo build --release --package chronos-mcp
# Binary: target\release\chronos-mcp.exe
```

---

## 1. Antigravity (Google) — Native MCP

Tạo file schema tại: `C:\Users\PT\.gemini\antigravity\mcp\chronos\`

```json
// chronos-agent.json (tool schema)
{
  "name": "chronos_analyze_file",
  "description": "Parse TypeScript file and insert into 4D Knowledge Graph",
  "parameters": {
    "type": "object",
    "properties": {
      "file_path": { "type": "string" }
    },
    "required": ["file_path"]
  }
}
```

Config trong `antigravity.json` hoặc `.agy/config.toml`:
```toml
[[mcp.servers]]
name = "chronos-agent"
command = "C:\\path\\to\\chronos-mcp.exe"
args = ["--workspace", "${workspaceFolder}"]
```

---

## 2. Cursor

Tạo `.cursor/mcp.json` tại root project:

```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"],
      "env": {}
    }
  }
}
```

Hoặc global tại `%APPDATA%\Cursor\User\globalStorage\cursor.mcp\mcp.json`.

---

## 3. OpenAI Codex CLI

Codex CLI hỗ trợ MCP từ v1.0+. Tạo `~/.codex/config.json`:

```json
{
  "mcp": {
    "servers": {
      "chronos-agent": {
        "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
        "args": ["--workspace", "."],
        "transport": "stdio"
      }
    }
  }
}
```

Sử dụng:
```bash
codex --mcp chronos-agent "analyze orderService.ts and show me hotspots"
```

---

## 4. OpenCode

Tạo `~/.config/opencode/config.toml` hoặc `.opencode.toml` tại project root:

```toml
[mcp.servers.chronos]
command = "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe"
args = ["--workspace", "."]
type = "local"
```

---

## 5. Claude Desktop

`%APPDATA%\Claude\claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
      "args": ["--workspace", "C:\\your\\project\\root"]
    }
  }
}
```

---

## 6. Cline (VS Code Extension)

Trong VS Code `settings.json`:

```json
{
  "cline.mcpServers": {
    "chronos-agent": {
      "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"],
      "disabled": false,
      "autoApprove": ["chronos_contract_status", "chronos_query_hotspots", "chronos_blast_radius"]
    }
  }
}
```

---

## 7. Windsurf (Codeium)

`.windsurf/mcp.json` tại project root:

```json
{
  "mcpServers": {
    "chronos-agent": {
      "serverType": "stdio",
      "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  }
}
```

---

## 8. Continue (VS Code / JetBrains)

`.continue/config.json`:

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "transport": {
          "type": "stdio",
          "command": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
          "args": ["--workspace", "."]
        }
      }
    ]
  }
}
```

---

## 9. Zed Editor

`~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "chronos-agent": {
      "command": {
        "path": "C:\\Users\\PT\\Downloads\\Claude\\GDGoC-HCMUS AI CHALLENGE 2026\\chronos-agent\\target\\release\\chronos-mcp.exe",
        "args": ["--workspace", "."],
        "env": {}
      }
    }
  }
}
```

---

## Kiểm tra kết nối

Test thủ công bằng cách pipe JSON vào binary:

```powershell
# Test initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"rootPath":"C:\\your\\project"}}' | .\target\release\chronos-mcp.exe

# Test tools/list
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | .\target\release\chronos-mcp.exe

# Test chronos_contract_status
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"chronos_contract_status","arguments":{}}}' | .\target\release\chronos-mcp.exe
```

Expected output format:
```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"chronos-agent","version":"0.1.0"}}}
```
