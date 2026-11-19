# @ptai-eng/chronos-mcp

Zero-build `npx` launcher for [CHRONOS-AGENT](https://github.com/ptai-eng/chronos-agent) (`chronos-mcp` MCP server + `chronos` CLI).

On first invocation, downloads and caches the matching prebuilt native binary (`win32-x64`, `linux-x64`, `darwin-arm64`) from GitHub Releases and forwards stdio directly.

## MCP Configuration

```json
{
  "mcpServers": {
    "chronos-agent": {
      "command": "npx",
      "args": ["-y", "@ptai-eng/chronos-mcp", "--workspace", "."]
    }
  }
}
```
