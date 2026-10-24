//! `chronos mcp` & `chronos schema` — Universal Multi-Agent Setup & SDK Schema Exporter

use anyhow::Result;
use serde_json::{json, Value};

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
    println!("11 Registered MCP Tools:");
    println!("  • chronos_analyze_file        • chronos_query_hotspots");
    println!("  • chronos_blast_radius        • chronos_mutation_gate");
    println!("  • chronos_contract_status     • chronos_query_graph");
    println!("  • chronos_search_graph        • chronos_get_snippet");
    println!("  • chronos_get_architecture    • chronos_hydrate_telemetry");
    println!("  • chronos_generate_agents_md");

    Ok(())
}

/// Export all 11 CHRONOS tools in:
/// - `openai` (OpenAI Chat Completions Function Calling `strict: true`)
/// - `responses` (OpenAI Responses API `strict: true`)
/// - `anthropic` / `claude` (Anthropic Messages API `tools` with `input_schema`)
/// - `gemini` / `antigravity` (Google Gemini / Vertex AI `functionDeclarations`)
/// - `codex-toml` (OpenAI Codex CLI `config.toml`)
/// - `mcp` (Standard MCP `tools/list` JSON)
pub fn export_schema(format: &str) -> Result<String> {
    let tools = raw_tool_specs();
    match format.trim().to_ascii_lowercase().as_str() {
        "codex-toml" | "toml" => {
            let toml_out = r#"# ~/.codex/config.toml — OpenAI Codex CLI Configuration for CHRONOS-AGENT
[mcp_servers.chronos-agent]
command = "chronos-mcp"
args = ["--workspace", "."]
"#;
            Ok(toml_out.to_string())
        }
        "responses" => {
            let mapped: Vec<Value> = tools
                .into_iter()
                .map(|(name, desc, params)| {
                    json!({
                        "type": "function",
                        "name": name,
                        "description": desc,
                        "strict": true,
                        "parameters": params
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&mapped)?)
        }
        "anthropic" | "claude" => {
            let mapped: Vec<Value> = tools
                .into_iter()
                .map(|(name, desc, params)| {
                    json!({
                        "name": name,
                        "description": desc,
                        "input_schema": params
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&mapped)?)
        }
        "gemini" | "antigravity" | "google" => {
            let declarations: Vec<Value> = tools
                .into_iter()
                .map(|(name, desc, params)| {
                    json!({
                        "name": name,
                        "description": desc,
                        "parameters": params
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&json!([{
                "functionDeclarations": declarations
            }]))?)
        }
        "mcp" => {
            let mapped: Vec<Value> = tools
                .into_iter()
                .map(|(name, desc, params)| {
                    json!({
                        "name": name,
                        "description": desc,
                        "inputSchema": params
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&json!({ "tools": mapped }))?)
        }
        _ => {
            // Default: OpenAI Function Calling (`"type": "function", "function": { ... }`)
            let mapped: Vec<Value> = tools
                .into_iter()
                .map(|(name, desc, params)| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": name,
                            "description": desc,
                            "strict": true,
                            "parameters": params
                        }
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&mapped)?)
        }
    }
}

fn raw_tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "chronos_analyze_file",
            "Parse a source file (.ts, .tsx, .js, .py, .rs, .go, .c, .cpp, .java) and insert all symbols, call edges, cross-language HTTP routes, test edges, and loop/complexity metrics into the 5D Knowledge Graph.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Workspace-relative or absolute file path" }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_query_hotspots",
            "Query the 5D Knowledge Graph for the top N performance bottlenecks and hot-path functions.",
            json!({
                "type": "object",
                "properties": {
                    "top_n": { "type": "integer", "description": "Number of hotspots to return" },
                    "filter": { "type": "string", "enum": ["bottlenecks", "hot_paths", "high_complexity", "all"] }
                },
                "required": ["top_n", "filter"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_blast_radius",
            "Analyze the blast-radius (change impact) of modifying a symbol: RiskZone, PageRank, Git co-change partners, 95% CI, and residual unknowns.",
            json!({
                "type": "object",
                "properties": {
                    "symbol_name": { "type": "string" },
                    "file_path": { "type": ["string", "null"] },
                    "contract_breaking": { "type": "boolean" },
                    "detail": { "type": "string", "enum": ["summary", "complete"] }
                },
                "required": ["symbol_name", "file_path", "contract_breaking", "detail"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_mutation_gate",
            "Run targeted Super-Mutant testing on a file and return the mutation kill score and survived mutant locations.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string" },
                    "test_command": { "type": "string" },
                    "strict": { "type": "boolean" }
                },
                "required": ["file_path", "test_command", "strict"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_contract_status",
            "Drain the Semantic Contract Broker's Pre-Tool Execution Gate for breaking-change alerts.",
            json!({
                "type": "object",
                "properties": {
                    "watch_dir": { "type": ["string", "null"] }
                },
                "required": ["watch_dir"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_query_graph",
            "Execute a Read-Only OpenCypher query against the embedded KuzuDB 5D Knowledge Graph.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "max_rows": { "type": "integer" }
                },
                "required": ["query", "max_rows"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_search_graph",
            "Search the 5D Knowledge Graph using BM25 identifier tokenization, regex matching, degree filtering, and nested loop depth.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": ["string", "null"] },
                    "name_pattern": { "type": ["string", "null"] },
                    "file_pattern": { "type": ["string", "null"] },
                    "kind": { "type": ["string", "null"] },
                    "min_degree": { "type": ["integer", "null"] },
                    "min_loop_depth": { "type": ["integer", "null"] },
                    "limit": { "type": "integer" }
                },
                "required": ["query", "name_pattern", "file_pattern", "kind", "min_degree", "min_loop_depth", "limit"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_get_snippet",
            "Read verbatim source code (start_line..=end_line) of a function/class with callers, callees, covering tests, and AST complexity.",
            json!({
                "type": "object",
                "properties": {
                    "symbol_name": { "type": "string" },
                    "file_path": { "type": ["string", "null"] }
                },
                "required": ["symbol_name", "file_path"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_get_architecture",
            "Get a high-level architecture overview: Label-Propagation community clusters, Cohesion score, top PageRank hubs, and O(N^2) loop hotspots.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": ["string", "null"] },
                    "auto_index": { "type": "boolean" }
                },
                "required": ["path", "auto_index"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_hydrate_telemetry",
            "Hydrate the 4th Dimension (runtime telemetry) from an offline profiler artifact (.cpuprofile, Speedscope JSON, Folded Stack Flamegraph, or OTLP Trace JSON).",
            json!({
                "type": "object",
                "properties": {
                    "profile_path": { "type": ["string", "null"] },
                    "profile_content": { "type": ["string", "null"] }
                },
                "required": ["profile_path", "profile_content"],
                "additionalProperties": false
            }),
        ),
        (
            "chronos_generate_agents_md",
            "Synthesize data-driven AGENTS.md, CLAUDE.md, GEMINI.md, Antigravity SKILL.md, Cursor .mdc rules, and MCP configs from the 5D Knowledge Graph.",
            json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "enum": ["codex", "antigravity", "claude", "opencode", "cursor", "windsurf", "copilot", "cline", "all"] },
                    "project_name": { "type": ["string", "null"] },
                    "write_to_disk": { "type": "boolean" },
                    "scaffold_configs": { "type": "boolean" },
                    "output_path": { "type": "string" },
                    "auto_index": { "type": "boolean" }
                },
                "required": ["target", "project_name", "write_to_disk", "scaffold_configs", "output_path", "auto_index"],
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_schema_openai_responses_anthropic_gemini_and_codex_toml() {
        let openai = export_schema("openai").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&openai).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 11);
        assert_eq!(parsed[0]["type"], "function");
        assert_eq!(parsed[0]["function"]["strict"], true);

        let responses = export_schema("responses").unwrap();
        let resp_val: serde_json::Value = serde_json::from_str(&responses).unwrap();
        assert_eq!(resp_val.as_array().unwrap().len(), 11);
        assert_eq!(resp_val[10]["name"], "chronos_generate_agents_md");

        let anthropic = export_schema("anthropic").unwrap();
        let anth_val: serde_json::Value = serde_json::from_str(&anthropic).unwrap();
        assert_eq!(anth_val.as_array().unwrap().len(), 11);
        assert!(anth_val[0]["input_schema"].is_object());

        let gemini = export_schema("gemini").unwrap();
        let gem_val: serde_json::Value = serde_json::from_str(&gemini).unwrap();
        assert_eq!(gem_val[0]["functionDeclarations"].as_array().unwrap().len(), 11);

        let toml = export_schema("codex-toml").unwrap();
        assert!(toml.contains("[mcp_servers.chronos-agent]"));
    }
}
