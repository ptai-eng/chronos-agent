//! `chronos mcp` & `chronos schema` — Universal Multi-Agent Setup & SDK Schema Exporter

use anyhow::Result;
use chronos_core::registry::{SchemaFormat, ToolRegistry};

pub fn run(client: String) -> Result<()> {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.parent().map(|d| {
                d.join(if cfg!(windows) {
                    "chronos-mcp.exe"
                } else {
                    "chronos-mcp"
                })
            })
        })
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "chronos-mcp".to_string());

    println!(
        "🚀 CHRONOS MCP Server v{} — Universal Multi-Agent & CLI Integration Guide\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("Binary: {binary}");
    println!("One-Command Auto-Scaffold: chronos init --client <all|codex|antigravity|claude|opencode|cursor|windsurf|copilot|cline>\n");

    let c = client.trim().to_ascii_lowercase();
    let show_all = c == "all";

    if show_all || c == "codex" || c == "openai" {
        println!("┌─ 1. OpenAI Codex CLI & ChatGPT Codex ───────────────────────");
        println!("│  Auto-scaffold AGENTS.md + .codex/config.toml:");
        println!("│    chronos init --client codex");
        println!("│");
        println!("│  Or one-command CLI registration:");
        println!("│    codex mcp add chronos-agent -- \"{binary}\" --workspace .");
        println!("│");
        println!("│  ~/.codex/config.toml or .codex/config.toml:");
        println!("│    [mcp_servers.chronos-agent]");
        println!("│    command = \"{}\"", binary.replace('\\', "\\\\"));
        println!("│    args = [\"--workspace\", \".\"]");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "antigravity" || c == "agy" || c == "gemini" {
        println!("┌─ 2. Google Antigravity (agy CLI & Antigravity 2.0 IDE) ─────");
        println!("│  Auto-scaffold GEMINI.md + .agents/skills/ + .agents/plugins/:");
        println!("│    chronos init --client antigravity");
        println!("│");
        println!("│  Global (~/.gemini/config/mcp_config.json) or Plugin (.agents/plugins/chronos-agent/mcp_config.json):");
        println!("│    {{");
        println!("│      \"mcpServers\": {{");
        println!("│        \"chronos-agent\": {{");
        println!("│          \"command\": \"{}\",", binary.replace('\\', "\\\\"));
        println!("│          \"args\": [\"--workspace\", \".\"]");
        println!("│        }}");
        println!("│      }}");
        println!("│    }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "claude" || c == "claude-code" {
        println!("┌─ 3. Anthropic Claude Code CLI & Claude Desktop ─────────────");
        println!("│  Auto-scaffold CLAUDE.md + .mcp.json + .claude/commands/chronos-audit.md:");
        println!("│    chronos init --client claude");
        println!("│");
        println!("│  Or register via Claude Code CLI:");
        println!("│    claude mcp add chronos-agent -- \"{binary}\" --workspace .");
        println!("│");
        println!("│  Invoke slash command inside Claude Code:");
        println!("│    /project:chronos-audit <symbol_or_file>");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "opencode" {
        println!("┌─ 4. OpenCode CLI (opencode) ────────────────────────────────");
        println!("│  Auto-scaffold opencode.json + .opencode/agent/chronos-auditor.md:");
        println!("│    chronos init --client opencode");
        println!("│");
        println!("│  opencode.json:");
        println!("│    {{");
        println!("│      \"$schema\": \"https://opencode.ai/config.json\",");
        println!("│      \"mcp\": {{");
        println!("│        \"chronos-agent\": {{");
        println!("│          \"type\": \"local\",");
        println!("│          \"command\": [\"{}\", \"--workspace\", \".\"],", binary.replace('\\', "\\\\"));
        println!("│          \"enabled\": true");
        println!("│        }}");
        println!("│      }}");
        println!("│    }}");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "cursor" {
        println!("┌─ 5. Cursor IDE & Cursor Agent CLI ──────────────────────────");
        println!("│  Auto-scaffold .cursor/mcp.json + .cursor/rules/chronos-5d.mdc:");
        println!("│    chronos init --client cursor");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "windsurf" {
        println!("┌─ 6. Windsurf Cascade ───────────────────────────────────────");
        println!("│  Auto-scaffold .windsurf/mcp.json + .windsurfrules:");
        println!("│    chronos init --client windsurf");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "copilot" || c == "vscode" {
        println!("┌─ 7. GitHub Copilot Coding Agent & VS Code Native MCP ─────");
        println!("│  Auto-scaffold .vscode/mcp.json + .github/copilot-instructions.md:");
        println!("│    chronos init --client copilot");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "cline" || c == "roo" {
        println!("┌─ 8. Cline & Roo Code (VS Code) ─────────────────────────────");
        println!("│  Auto-scaffold .clinerules:");
        println!("│    chronos init --client cline");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "openai-agents" || c == "chatgpt" || c == "sdk" {
        println!("┌─ 9. Native AI SDK Tool Schemas (OpenAI / Anthropic / Gemini) ");
        println!("│  • OpenAI Responses API:   chronos schema --format responses");
        println!("│  • OpenAI Function Call:   chronos schema --format openai");
        println!("│  • Anthropic Claude API:   chronos schema --format anthropic");
        println!("│  • Google Gemini / Vertex: chronos schema --format gemini");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || c == "continue" || c == "zed" {
        println!("┌─ 10. Continue & Zed Editor ─────────────────────────────────");
        println!("│  Configure stdio context_servers in .continue/config.json or ~/.config/zed/settings.json.");
        println!("└─────────────────────────────────────────────────────────────\n");
    }

    println!("─────────────────────────────────────────────────────────────────");
    println!("14 Registered MCP Tools:");
    println!("  • chronos_analyze_file        • chronos_query_hotspots");
    println!("  • chronos_blast_radius        • chronos_mutation_gate");
    println!("  • chronos_contract_status     • chronos_query_graph");
    println!("  • chronos_search_graph        • chronos_get_snippet");
    println!("  • chronos_get_architecture    • chronos_hydrate_telemetry");
    println!("  • chronos_generate_agents_md  • chronos_explore (1-Call Context)");
    println!("  • chronos_edit_symbol         • chronos_refactor_symbol");

    Ok(())
}

/// Export all 14 CHRONOS tools from the canonical `chronos_core::registry::ToolRegistry`:
/// - `openai` (OpenAI Chat Completions Function Calling `strict: true`)
/// - `responses` (OpenAI Responses API `strict: true`)
/// - `anthropic` / `claude` (Anthropic Messages API `tools` with `input_schema`)
/// - `gemini` / `antigravity` (Google Gemini / Vertex AI `functionDeclarations`)
/// - `codex-toml` (OpenAI Codex CLI `config.toml`)
/// - `mcp` (Standard MCP `tools/list` JSON)
pub fn export_schema(format: &str) -> Result<String> {
    ToolRegistry::export_schema(SchemaFormat::from_str_loose(format))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_schema_openai_responses_anthropic_gemini_and_codex_toml() {
        let openai = export_schema("openai").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&openai).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 14);
        assert_eq!(parsed[0]["type"], "function");
        assert_eq!(parsed[0]["function"]["strict"], true);

        let responses = export_schema("responses").unwrap();
        let resp_val: serde_json::Value = serde_json::from_str(&responses).unwrap();
        assert_eq!(resp_val.as_array().unwrap().len(), 14);
        assert_eq!(resp_val[10]["name"], "chronos_generate_agents_md");
        assert_eq!(resp_val[11]["name"], "chronos_explore");
        assert_eq!(resp_val[12]["name"], "chronos_edit_symbol");
        assert_eq!(resp_val[13]["name"], "chronos_refactor_symbol");

        let anthropic = export_schema("anthropic").unwrap();
        let anth_val: serde_json::Value = serde_json::from_str(&anthropic).unwrap();
        assert_eq!(anth_val.as_array().unwrap().len(), 14);
        assert!(anth_val[0]["input_schema"].is_object());

        let gemini = export_schema("gemini").unwrap();
        let gem_val: serde_json::Value = serde_json::from_str(&gemini).unwrap();
        assert_eq!(gem_val[0]["functionDeclarations"].as_array().unwrap().len(), 14);

        let toml = export_schema("codex-toml").unwrap();
        assert!(toml.contains("[mcp_servers.chronos-agent]"));
    }
}
