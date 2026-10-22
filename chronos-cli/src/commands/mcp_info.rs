//! `chronos mcp` & `chronos schema` — MCP server startup instructions & OpenAI Schema Exporter

use anyhow::Result;
use serde_json::{json, Value};

pub fn run(client: String) -> Result<()> {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "chronos-mcp.exe" } else { "chronos-mcp" })))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "chronos-mcp".to_string());

    println!("🚀 CHRONOS MCP Server v{} — Universal Integration Guide\n", env!("CARGO_PKG_VERSION"));
    println!("Binary: {binary}\n");

    let show_all = client == "all";

    if show_all || client == "codex" || client == "openai" {
        println!("┌─ 1. OpenAI Codex CLI & ChatGPT Codex (Recommended) ─────────");
        println!("│  Option A — One-command CLI registration:");
        println!("│    codex mcp add chronos-agent -- \"{binary}\" --workspace .");
        println!("│");
        println!("│  Option B — Add to ~/.codex/config.toml:");
        println!("│    [mcp_servers.chronos-agent]");
        println!("│    command = \"{}\"", binary.replace('\\', "\\\\"));
        println!("│    args = [\"--workspace\", \".\"]");
        println!("│");
        println!("│  Synthesize AGENTS.md for OpenAI Codex:");
        println!("│    chronos agents-md --write");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "openai-agents" || client == "chatgpt" {
        println!("┌─ 2. OpenAI Agents SDK & Responses API ──────────────────────");
        println!("│  Export strict JSON Schema for OpenAI Responses / Chat API:");
        println!("│    chronos schema --format responses > openai_tools.json");
        println!("│");
        println!("│  Python OpenAI Agents SDK (MCPServerStdio):");
        println!("│    from agents.mcp import MCPServerStdio");
        println!("│    chronos = MCPServerStdio(params={{");
        println!("│        \"command\": \"{}\",", binary.replace('\\', "\\\\"));
        println!("│        \"args\": [\"--workspace\", \".\"]");
        println!("│    }})");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "antigravity" {
        println!("┌─ 3. Google Antigravity ─────────────────────────────────────");
        println!("│  Add to ~/.gemini/antigravity/mcp/ or .agy/config.toml:");
        println!("│");
        println!("│  [[mcp.servers]]");
        println!("│  name = \"chronos-agent\"");
        println!("│  command = \"{binary}\"");
        println!("│  args = [\"--workspace\", \"${{workspaceFolder}}\"]");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "cursor" {
        println!("┌─ 4. Cursor ─────────────────────────────────────────────────");
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
    if show_all || client == "opencode" {
        println!("┌─ 5. OpenCode ───────────────────────────────────────────────");
        println!("│  Create .opencode.toml in your project root:");
        println!("│");
        println!("│  [mcp.servers.chronos]");
        println!("│  command = \"{binary}\"");
        println!("│  args = [\"--workspace\", \".\"]");
        println!("│  type = \"local\"");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "claude" {
        println!("┌─ 6. Claude Desktop / Claude Code ───────────────────────────");
        println!("│  claude mcp add chronos-agent -- \"{binary}\" --workspace .");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "cline" {
        println!("┌─ 7. Cline (VS Code) ────────────────────────────────────────");
        println!("│  Add to VS Code settings.json under \"cline.mcpServers\".");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "windsurf" {
        println!("┌─ 8. Windsurf ───────────────────────────────────────────────");
        println!("│  Create .windsurf/mcp.json with serverType = \"stdio\".");
        println!("└─────────────────────────────────────────────────────────────\n");
    }
    if show_all || client == "continue" || client == "zed" {
        println!("┌─ 9. Continue & Zed Editor ──────────────────────────────────");
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

/// Export all 11 CHRONOS tools in OpenAI Chat Completions (`openai`),
/// OpenAI Responses API (`responses`), OpenAI Codex CLI TOML (`codex-toml`), or MCP (`mcp`) format.
pub fn export_schema(format: &str) -> Result<String> {
    let tools = raw_tool_specs();
    match format {
        "codex-toml" | "toml" => {
            let toml_out = r#"# ~/.codex/config.toml — OpenAI Codex CLI Configuration for CHRONOS-AGENT v1.5.0
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
            "Synthesize a data-driven AGENTS.md blueprint from the 5D Knowledge Graph for OpenAI Codex CLI and ChatGPT Codex Cloud.",
            json!({
                "type": "object",
                "properties": {
                    "project_name": { "type": ["string", "null"] },
                    "write_to_disk": { "type": "boolean" },
                    "output_path": { "type": "string" },
                    "auto_index": { "type": "boolean" }
                },
                "required": ["project_name", "write_to_disk", "output_path", "auto_index"],
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_schema_openai_and_responses_and_codex_toml() {
        let openai = export_schema("openai").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&openai).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 11);
        assert_eq!(parsed[0]["type"], "function");
        assert_eq!(parsed[0]["function"]["strict"], true);

        let responses = export_schema("responses").unwrap();
        let resp_val: serde_json::Value = serde_json::from_str(&responses).unwrap();
        assert_eq!(resp_val.as_array().unwrap().len(), 11);
        assert_eq!(resp_val[10]["name"], "chronos_generate_agents_md");

        let toml = export_schema("codex-toml").unwrap();
        assert!(toml.contains("[mcp_servers.chronos-agent]"));
    }
}

