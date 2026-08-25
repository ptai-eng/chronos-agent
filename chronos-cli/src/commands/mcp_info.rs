//! `chronos mcp` — Show MCP server startup instructions for all AI clients

use anyhow::Result;

pub fn run(client: String) -> Result<()> {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("chronos-mcp.exe")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "path/to/chronos-mcp.exe".to_string());

    println!("🚀 CHRONOS MCP Server — Integration Guide\n");
    println!("Binary: {binary}\n");

    let show_all = client == "all";

    if show_all || client == "antigravity" {
        println!("┌─ 1. Google Antigravity ─────────────────────────────────────");
        println!("│  Add to ~/.gemini/antigravity/mcp/ or .agy/config.toml:");
        println!("│");
        println!("│  [[mcp.servers]]");
        println!("│  name = \"chronos-agent\"");
        println!("│  command = \"{binary}\"");
        println!("│  args = [\"--workspace\", \"${{workspaceFolder}}\"]");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "cursor" {
        println!("┌─ 2. Cursor ─────────────────────────────────────────────────");
        println!("│  Create .cursor/mcp.json in your project root:");
        println!("│");
        println!("│  {{");
        println!("│    \"mcpServers\": {{");
        println!("│      \"chronos-agent\": {{");
        println!("│        \"command\": \"{binary}\",");
        println!("│        \"args\": [\"--workspace\", \"${{workspaceFolder}}\"]");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "codex" {
        println!("┌─ 3. OpenAI Codex CLI ───────────────────────────────────────");
        println!("│  Add to ~/.codex/config.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"mcp\": {{");
        println!("│      \"servers\": {{");
        println!("│        \"chronos-agent\": {{");
        println!("│          \"command\": \"{binary}\",");
        println!("│          \"args\": [\"--workspace\", \".\"],");
        println!("│          \"transport\": \"stdio\"");
        println!("│        }}");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "opencode" {
        println!("┌─ 4. OpenCode ───────────────────────────────────────────────");
        println!("│  Create .opencode.toml in your project root:");
        println!("│");
        println!("│  [mcp.servers.chronos]");
        println!("│  command = \"{binary}\"");
        println!("│  args = [\"--workspace\", \".\"]");
        println!("│  type = \"local\"");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "claude" {
        println!("┌─ 5. Claude Desktop ─────────────────────────────────────────");
        println!("│  Add to %APPDATA%\\Claude\\claude_desktop_config.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"mcpServers\": {{");
        println!("│      \"chronos-agent\": {{");
        println!("│        \"command\": \"{binary}\",");
        println!("│        \"args\": [\"--workspace\", \"C:\\\\your\\\\project\"]");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "cline" {
        println!("┌─ 6. Cline (VS Code) ────────────────────────────────────────");
        println!("│  Add to VS Code settings.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"cline.mcpServers\": {{");
        println!("│      \"chronos-agent\": {{");
        println!("│        \"command\": \"{binary}\",");
        println!("│        \"args\": [\"--workspace\", \"${{workspaceFolder}}\"]");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "windsurf" {
        println!("┌─ 7. Windsurf ───────────────────────────────────────────────");
        println!("│  Create .windsurf/mcp.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"mcpServers\": {{");
        println!("│      \"chronos-agent\": {{");
        println!("│        \"serverType\": \"stdio\",");
        println!("│        \"command\": \"{binary}\",");
        println!("│        \"args\": [\"--workspace\", \"${{workspaceFolder}}\"]");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "continue" {
        println!("┌─ 8. Continue (VS Code / JetBrains) ────────────────────────");
        println!("│  Add to .continue/config.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"experimental\": {{");
        println!("│      \"modelContextProtocolServers\": [{{");
        println!("│        \"transport\": {{");
        println!("│          \"type\": \"stdio\",");
        println!("│          \"command\": \"{binary}\",");
        println!("│          \"args\": [\"--workspace\", \".\"]");
        println!("│        }}");
        println!("│      }}]");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "zed" {
        println!("┌─ 9. Zed Editor ─────────────────────────────────────────────");
        println!("│  Add to ~/.config/zed/settings.json:");
        println!("│");
        println!("│  {{");
        println!("│    \"context_servers\": {{");
        println!("│      \"chronos-agent\": {{");
        println!("│        \"command\": {{");
        println!("│          \"path\": \"{binary}\",");
        println!("│          \"args\": [\"--workspace\", \".\"]");
        println!("│        }}");
        println!("│      }}");
        println!("│    }}");
        println!("│  }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }

    println!("─────────────────────────────────────────────────────────────────");
    println!("Test the server (pipe JSON to binary):");
    println!("  echo '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{{}}}}' | {binary}");
    println!();
    println!("Tools: chronos_analyze_file | chronos_query_hotspots |");
    println!("       chronos_blast_radius | chronos_mutation_gate | chronos_contract_status");

    Ok(())
}
