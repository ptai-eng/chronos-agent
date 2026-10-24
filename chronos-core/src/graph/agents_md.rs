//! # Universal Multi-Agent 5D Context & Scaffolding Synthesizer
//!
//! Compiles the 5D Knowledge Graph (Community Clusters, PageRank Hubs, Algorithmic Hotspots,
//! Cross-Boundary HTTP Routes, and Git Temporal Co-Change Coupling) into native context files,
//! skills, rules, slash commands, and MCP manifests for:
//! - **OpenAI Codex CLI & ChatGPT Codex** (`AGENTS.md`, `.codex/config.toml`)
//! - **Google Antigravity (`agy` CLI & IDE)** (`GEMINI.md`, `.agents/skills/chronos-agent/SKILL.md`, `.agents/plugins/chronos-agent/`)
//! - **Anthropic Claude Code & Claude Desktop** (`CLAUDE.md`, `.mcp.json`, `.claude/commands/chronos-audit.md`)
//! - **OpenCode CLI (`opencode`)** (`opencode.json`, `.opencode/agent/chronos-auditor.md`)
//! - **Cursor IDE & Agent CLI** (`.cursor/mcp.json`, `.cursor/rules/chronos-5d.mdc`)
//! - **Windsurf Cascade** (`.windsurf/mcp.json`, `.windsurfrules`)
//! - **GitHub Copilot Coding Agent & VS Code** (`.vscode/mcp.json`, `.github/copilot-instructions.md`)
//! - **Cline & Roo Code** (`.clinerules`, `.roo/mcp.json`)

use crate::graph::{
    query::GraphQueryEngine,
    schema::{ChronosGraph, EdgeKind, NodeKind},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

/// Supported AI coding CLI and IDE targets for 5D Knowledge Graph synthesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    Codex,
    Antigravity,
    ClaudeCode,
    OpenCode,
    Cursor,
    Windsurf,
    Copilot,
    Cline,
    All,
}

impl AgentTarget {
    /// Parse a target identifier from CLI flag or MCP parameter.
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "codex" | "openai" | "chatgpt" | "agents" | "agents.md" => Self::Codex,
            "antigravity" | "agy" | "gemini" | "gemini.md" | "google" => Self::Antigravity,
            "claude" | "claude-code" | "claudecode" | "claude.md" | "anthropic" => Self::ClaudeCode,
            "opencode" | "open-code" | "sst" => Self::OpenCode,
            "cursor" | "mdc" => Self::Cursor,
            "windsurf" | "codeium" | "cascade" => Self::Windsurf,
            "copilot" | "github-copilot" | "vscode" | "github" => Self::Copilot,
            "cline" | "roo" | "roocode" => Self::Cline,
            "all" | "*" | "universal" => Self::All,
            _ => Self::Codex,
        }
    }

    /// Default primary context file path (relative to workspace root) for single-file output.
    pub fn default_context_path(&self) -> &'static str {
        match self {
            Self::Codex | Self::All => "AGENTS.md",
            Self::Antigravity => "GEMINI.md",
            Self::ClaudeCode => "CLAUDE.md",
            Self::OpenCode => ".opencode/agent/chronos-auditor.md",
            Self::Cursor => ".cursor/rules/chronos-5d.mdc",
            Self::Windsurf => ".windsurfrules",
            Self::Copilot => ".github/copilot-instructions.md",
            Self::Cline => ".clinerules",
        }
    }

    /// Human-readable display label for the target ecosystem.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Codex => "OpenAI Codex CLI & ChatGPT Codex",
            Self::Antigravity => "Google Antigravity (agy CLI & Antigravity IDE)",
            Self::ClaudeCode => "Anthropic Claude Code CLI & Claude Desktop",
            Self::OpenCode => "OpenCode CLI (opencode)",
            Self::Cursor => "Cursor IDE & Cursor Agent CLI",
            Self::Windsurf => "Windsurf Cascade",
            Self::Copilot => "GitHub Copilot Coding Agent & VS Code",
            Self::Cline => "Cline & Roo Code",
            Self::All => "Universal Multi-Agent Suite (Codex, Antigravity, Claude Code, OpenCode, Cursor, Windsurf, Copilot, Cline)",
        }
    }
}

/// A synthesized configuration, rule, or skill file ready to be written to the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldedFile {
    /// Workspace-relative file path (using forward slashes).
    pub relative_path: String,
    /// Target client/ecosystem description.
    pub target_client: String,
    /// Full file content.
    pub content: String,
}

/// Synthesize a complete, data-driven `AGENTS.md` specification from a populated `ChronosGraph`.
/// Preserved for 100% backward compatibility with `v1.5.0` callers.
pub fn synthesize_agents_md(
    graph: &ChronosGraph,
    workspace: &Path,
    project_name_override: Option<&str>,
) -> String {
    synthesize_target_context(graph, workspace, AgentTarget::Codex, project_name_override)
}

/// Synthesize a tailored 5D Knowledge Graph context document for a specific `AgentTarget`.
pub fn synthesize_target_context(
    graph: &ChronosGraph,
    workspace: &Path,
    target: AgentTarget,
    project_name_override: Option<&str>,
) -> String {
    let project_name = resolve_project_name(workspace, project_name_override);
    let core_body = render_5d_knowledge_sections(graph, workspace);

    let mut out = String::with_capacity(10240);

    match target {
        AgentTarget::Codex | AgentTarget::All => {
            out.push_str(&format!("# AGENTS.md — {project_name}\n\n"));
            out.push_str(
                "> Auto-synthesized by **CHRONOS-AGENT** (5D Living Code Intelligence Engine) for **OpenAI Codex CLI**, **ChatGPT Codex**, **Google Antigravity**, **OpenCode**, and MCP-compliant autonomous coding agents.\n\n",
            );
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. OpenAI Codex & MCP Tool Protocol (Mandatory Before Committing)\n\n");
            out.push_str(
                "When connected to `chronos-mcp` (`codex mcp add chronos-agent -- chronos-mcp --workspace .` or `claude mcp add chronos-agent -- chronos-mcp --workspace .`), follow this 4-step verification loop:\n\n",
            );
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::ClaudeCode => {
            out.push_str(&format!("# CLAUDE.md — {project_name}\n\n"));
            out.push_str(
                "> Auto-synthesized by **CHRONOS-AGENT** (5D Living Code Intelligence Engine) for **Anthropic Claude Code (`claude` CLI)** and **Claude Desktop**.\n\n",
            );
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Claude Code MCP Workflow (`mcp__chronos-agent__*`)\n\n");
            out.push_str(
                "This repository includes `.mcp.json` and `.claude/commands/chronos-audit.md` (`/project:chronos-audit`). Always use the `chronos-agent` MCP server before editing shared symbols:\n\n",
            );
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::Antigravity => {
            out.push_str(&format!("# GEMINI.md — {project_name}\n\n"));
            out.push_str(
                "> Auto-synthesized by **CHRONOS-AGENT** (5D Living Code Intelligence Engine) for **Google Antigravity (`agy` CLI & Antigravity 2.0 IDE)** and **Gemini CLI**.\n\n",
            );
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Antigravity Workspace Customizations & MCP Protocol\n\n");
            out.push_str(
                "This workspace ships native Antigravity customizations under `.agents/skills/chronos-agent/SKILL.md` and `.agents/plugins/chronos-agent/`. Always follow this 4-step 5D verification loop:\n\n",
            );
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::OpenCode => {
            out.push_str("---\n");
            out.push_str("description: CHRONOS 5D Blast-Radius, Cypher & Super-Mutant Verification Auditor\n");
            out.push_str("mode: subagent\n");
            out.push_str("tools:\n");
            out.push_str("  chronos-agent_*: true\n");
            out.push_str("  bash: true\n");
            out.push_str("  read: true\n");
            out.push_str("---\n\n");
            out.push_str(&format!("# OpenCode 5D Auditor — {project_name}\n\n"));
            out.push_str(
                "You are the **CHRONOS 5D Verification Subagent** for **OpenCode (`opencode`)**. Use the `chronos-agent` MCP tools configured in `opencode.json` to audit structural blast-radius, Git temporal co-changes, and mutation test gates.\n\n",
            );
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Mandatory 4-Step Verification Loop\n\n");
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::Cursor => {
            out.push_str("---\n");
            out.push_str(&format!(
                "description: CHRONOS 5D Knowledge Graph & Blast-Radius Guardrails for {project_name}\n"
            ));
            out.push_str("globs: **/*\n");
            out.push_str("alwaysApply: true\n");
            out.push_str("---\n\n");
            out.push_str(&format!("# Cursor 5D Code Intelligence Rules — {project_name}\n\n"));
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Cursor Agent MCP Protocol (`.cursor/mcp.json`)\n\n");
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::Windsurf => {
            out.push_str(&format!("# Windsurf Cascade 5D Rules — {project_name}\n\n"));
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Windsurf Cascade MCP Protocol (`.windsurf/mcp.json`)\n\n");
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::Copilot => {
            out.push_str(&format!("# GitHub Copilot Coding Agent Instructions — {project_name}\n\n"));
            out.push_str(
                "> Auto-synthesized by **CHRONOS-AGENT** for **GitHub Copilot Coding Agent** and **VS Code MCP (`.vscode/mcp.json`)**.\n\n",
            );
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Copilot Agent & VS Code MCP Protocol\n\n");
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
        AgentTarget::Cline => {
            out.push_str(&format!("# Cline & Roo Code 5D Rules (`.clinerules`) — {project_name}\n\n"));
            append_build_commands(&mut out, workspace);
            out.push_str("## 2. Cline / Roo Code MCP Protocol\n\n");
            append_4_step_protocol(&mut out);
            out.push_str(&core_body);
        }
    }

    out
}

/// Synthesize the Antigravity `.agents/skills/chronos-agent/SKILL.md` file with progressive-disclosure YAML frontmatter.
pub fn synthesize_antigravity_skill_md(
    graph: &ChronosGraph,
    workspace: &Path,
    project_name_override: Option<&str>,
) -> String {
    let project_name = resolve_project_name(workspace, project_name_override);
    let qe = GraphQueryEngine::new(graph);
    let arch = qe.get_architecture(None);

    let top_hubs: Vec<String> = arch
        .top_pagerank_hubs
        .iter()
        .take(5)
        .map(|h| format!("`{}` (`{:.4}`)", h.node.name, h.pagerank))
        .collect();
    let hubs_summary = if top_hubs.is_empty() {
        "Run `chronos_get_architecture` to populate".to_string()
    } else {
        top_hubs.join(", ")
    };

    format!(
        r#"---
name: chronos-agent
description: >-
  Use this skill whenever analyzing code architecture, checking blast-radius impact before refactoring,
  finding O(N^2) nested-loop bottlenecks, querying the 5D Knowledge Graph via OpenCypher or BM25,
  hydrating runtime telemetry profiles, or running Super-Mutant verification gates in {project_name}.
---

# CHRONOS-AGENT 5D Code Intelligence Skill ({project_name})

This skill connects **Google Antigravity (`agy`)**, **Claude Code**, **OpenAI Codex**, and **OpenCode** to the **CHRONOS 5D Living Code Graph** (`chronos-mcp`).

## Primary High-Centrality Hubs in `{project_name}`

- **Top PageRank Hubs:** {hubs_summary}

## Mandatory 4-Step Verification Workflow

1. **Graph Discovery First (Zero Context Bloat):**
   - Call `chronos_get_architecture` to inspect Label-Propagation community clusters, cohesion scores, and $O(N^2)$ loop hotspots.
   - Call `chronos_search_graph` (BM25 + regex + `min_loop_depth`) or `chronos_get_snippet` to read exact function spans (`start_line..=end_line`) with callers, callees, and covering tests.
   - Call `chronos_query_graph` for custom read-only OpenCypher queries over `ChronosNode` (`CALLS`, `CONTAINS`, `TESTS`, `IMPORTS`, `CO_CHANGES_WITH`).

2. **Pre-Edit 5D Blast-Radius Gate:**
   - Before modifying any public function, struct, or HTTP route, call `chronos_blast_radius` (`symbol_name`, `contract_breaking: true/false`, `detail: "summary"`).
   - Inspect the `confidence_interval`, `residual_unknowns`, and Git temporal co-change partners (`CO_CHANGES_WITH`).

3. **Semantic Contract Broker Check:**
   - After modifying function signatures or API endpoints, call `chronos_contract_status` to ensure no downstream callers were broken.

4. **Super-Mutant Verification Gate:**
   - Before completing a task on core business logic, run `chronos_mutation_gate` (`file_path`) or `chronos diff --staged --json --ci` to verify that unit tests kill synthetic code mutants.
"#
    )
}

/// Generate the complete set of native config, rule, skill, and slash-command files for `target`.
pub fn scaffold_client_files(
    graph: &ChronosGraph,
    workspace: &Path,
    target: AgentTarget,
    mcp_binary: &str,
    project_name_override: Option<&str>,
) -> Vec<ScaffoldedFile> {
    let project_name = resolve_project_name(workspace, project_name_override);
    let mut files = Vec::new();
    let include_all = target == AgentTarget::All;

    // 1. OpenAI Codex CLI & ChatGPT Codex
    if include_all || target == AgentTarget::Codex {
        files.push(ScaffoldedFile {
            relative_path: "AGENTS.md".to_string(),
            target_client: "OpenAI Codex CLI / ChatGPT Codex / Universal AGENTS.md".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Codex, Some(&project_name)),
        });
        files.push(ScaffoldedFile {
            relative_path: ".codex/config.toml".to_string(),
            target_client: "OpenAI Codex CLI (.codex/config.toml)".to_string(),
            content: format!(
                r#"# OpenAI Codex CLI Project Configuration — Auto-generated by CHRONOS-AGENT
[mcp_servers.chronos-agent]
command = "{}"
args = ["--workspace", "."]
"#,
                mcp_binary.replace('\\', "\\\\")
            ),
        });
    }

    // 2. Google Antigravity (agy CLI & Antigravity 2.0 IDE)
    if include_all || target == AgentTarget::Antigravity {
        files.push(ScaffoldedFile {
            relative_path: "GEMINI.md".to_string(),
            target_client: "Google Antigravity & Gemini CLI Rule (GEMINI.md)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Antigravity, Some(&project_name)),
        });
        files.push(ScaffoldedFile {
            relative_path: ".agents/skills/chronos-agent/SKILL.md".to_string(),
            target_client: "Google Antigravity Workspace Skill (.agents/skills/chronos-agent/SKILL.md)".to_string(),
            content: synthesize_antigravity_skill_md(graph, workspace, Some(&project_name)),
        });
        files.push(ScaffoldedFile {
            relative_path: ".agents/plugins/chronos-agent/plugin.json".to_string(),
            target_client: "Google Antigravity Native Plugin Manifest".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "name": "chronos-agent"
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".agents/plugins/chronos-agent/mcp_config.json".to_string(),
            target_client: "Google Antigravity Plugin MCP Config".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "mcpServers": {
                    "chronos-agent": {
                        "command": mcp_binary,
                        "args": ["--workspace", "."]
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
    }

    // 3. Anthropic Claude Code CLI & Claude Desktop
    if include_all || target == AgentTarget::ClaudeCode {
        files.push(ScaffoldedFile {
            relative_path: "CLAUDE.md".to_string(),
            target_client: "Anthropic Claude Code Project Instructions (CLAUDE.md)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::ClaudeCode, Some(&project_name)),
        });
        files.push(ScaffoldedFile {
            relative_path: ".mcp.json".to_string(),
            target_client: "Claude Code Project-Scoped MCP Config (.mcp.json)".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "mcpServers": {
                    "chronos-agent": {
                        "type": "stdio",
                        "command": mcp_binary,
                        "args": ["--workspace", "."]
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".claude/commands/chronos-audit.md".to_string(),
            target_client: "Claude Code Slash Command (/project:chronos-audit)".to_string(),
            content: r#"Run a full 5D CHRONOS architectural, blast-radius, and mutation audit on `$ARGUMENTS`:

1. Call `chronos_get_architecture` to inspect community clusters, PageRank hubs, and $O(N^2)$ loop hotspots.
2. Call `chronos_blast_radius` on the target symbol or file in `$ARGUMENTS` (`detail: "complete"`) and report the `RiskZone`, 95% `confidence_interval`, `residual_unknowns`, and Git `CO_CHANGES_WITH` partners.
3. Call `chronos_contract_status` to check for any breaking signature drift.
4. If code changes were made, call `chronos_mutation_gate` to verify semantic test coverage.
"#
            .to_string(),
        });
    }

    // 4. OpenCode CLI (opencode)
    if include_all || target == AgentTarget::OpenCode {
        files.push(ScaffoldedFile {
            relative_path: "opencode.json".to_string(),
            target_client: "OpenCode CLI Project Config (opencode.json)".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "$schema": "https://opencode.ai/config.json",
                "mcp": {
                    "chronos-agent": {
                        "type": "local",
                        "command": [mcp_binary, "--workspace", "."],
                        "enabled": true
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".opencode/agent/chronos-auditor.md".to_string(),
            target_client: "OpenCode Specialized 5D Auditor Subagent".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::OpenCode, Some(&project_name)),
        });
    }

    // 5. Cursor IDE & Cursor Agent CLI
    if include_all || target == AgentTarget::Cursor {
        files.push(ScaffoldedFile {
            relative_path: ".cursor/mcp.json".to_string(),
            target_client: "Cursor Project MCP Config (.cursor/mcp.json)".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "mcpServers": {
                    "chronos-agent": {
                        "command": mcp_binary,
                        "args": ["--workspace", "."]
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".cursor/rules/chronos-5d.mdc".to_string(),
            target_client: "Cursor 5D Knowledge Graph Rule (.cursor/rules/chronos-5d.mdc)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Cursor, Some(&project_name)),
        });
    }

    // 6. Windsurf Cascade
    if include_all || target == AgentTarget::Windsurf {
        files.push(ScaffoldedFile {
            relative_path: ".windsurf/mcp.json".to_string(),
            target_client: "Windsurf MCP Config (.windsurf/mcp.json)".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "mcpServers": {
                    "chronos-agent": {
                        "serverType": "stdio",
                        "command": mcp_binary,
                        "args": ["--workspace", "."]
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".windsurfrules".to_string(),
            target_client: "Windsurf Cascade Rules (.windsurfrules)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Windsurf, Some(&project_name)),
        });
    }

    // 7. GitHub Copilot Coding Agent & VS Code Native MCP
    if include_all || target == AgentTarget::Copilot {
        files.push(ScaffoldedFile {
            relative_path: ".vscode/mcp.json".to_string(),
            target_client: "VS Code & GitHub Copilot Agent MCP Config (.vscode/mcp.json)".to_string(),
            content: serde_json::to_string_pretty(&json!({
                "servers": {
                    "chronos-agent": {
                        "type": "stdio",
                        "command": mcp_binary,
                        "args": ["--workspace", "${workspaceFolder}"]
                    }
                }
            }))
            .unwrap_or_default()
                + "\n",
        });
        files.push(ScaffoldedFile {
            relative_path: ".github/copilot-instructions.md".to_string(),
            target_client: "GitHub Copilot Coding Agent Instructions (.github/copilot-instructions.md)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Copilot, Some(&project_name)),
        });
    }

    // 8. Cline & Roo Code
    if include_all || target == AgentTarget::Cline {
        files.push(ScaffoldedFile {
            relative_path: ".clinerules".to_string(),
            target_client: "Cline & Roo Code Rules (.clinerules)".to_string(),
            content: synthesize_target_context(graph, workspace, AgentTarget::Cline, Some(&project_name)),
        });
    }

    files
}

fn resolve_project_name(workspace: &Path, project_name_override: Option<&str>) -> String {
    project_name_override
        .map(|s| s.to_string())
        .or_else(|| {
            workspace
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "workspace".to_string())
}

fn append_build_commands(out: &mut String, workspace: &Path) {
    out.push_str("## 1. Build, Test & Verification Commands\n\n");
    let detected_cmds = detect_workspace_commands(workspace);
    for (label, cmd) in &detected_cmds {
        out.push_str(&format!("- **{label}:** `{cmd}`\n"));
    }
    out.push_str("- **5D Git Diff Blast-Radius Gate (CI / Pre-PR):** `chronos diff --staged --json --ci`\n");
    out.push_str("- **Refresh Multi-Agent Context Files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`):** `chronos init --client all`\n\n");
}

fn append_4_step_protocol(out: &mut String) {
    out.push_str(
        "1. **Locate & Read Without Context Bloat:** Call `chronos_get_architecture`, `chronos_search_graph` (BM25 + regex + loop depth), or `chronos_get_snippet` instead of dumping entire files.\n",
    );
    out.push_str(
        "2. **Pre-Edit Blast-Radius Check:** Before modifying any function listed in **High-Centrality Hubs** below, call `chronos_blast_radius` (`symbol_name`, `contract_breaking`) to inspect transitive callers, 95% confidence bounds, and Git co-change partners.\n",
    );
    out.push_str(
        "3. **Contract Broker Gate:** After editing public signatures or routes, call `chronos_contract_status` to drain breaking-change alerts across callers.\n",
    );
    out.push_str(
        "4. **Super-Mutant Verification Gate:** Before finalizing a PR on critical logic, run `chronos_mutation_gate` (`file_path`) to verify unit tests kill synthetic code mutants.\n\n",
    );
}

fn render_5d_knowledge_sections(graph: &ChronosGraph, workspace: &Path) -> String {
    let qe = GraphQueryEngine::new(graph);
    let arch = qe.get_architecture(None);
    let status = graph.status_report();

    let mut out = String::with_capacity(6144);

    // 3. 5D Graph Telemetry & Topology Summary
    out.push_str("## 3. 5D Knowledge Graph Snapshot\n\n");
    let mut langs: Vec<(&String, &usize)> = arch.languages.iter().collect();
    langs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let lang_summary = if langs.is_empty() {
        "None indexed".to_string()
    } else {
        langs
            .iter()
            .map(|(k, v)| format!("{k} ({v})"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    out.push_str(&format!(
        "- **Indexed Symbols:** `{}` nodes (`{}` functions, `{}` tests) | **Directed Edges:** `{}` (`{}` resolved calls, `{}` unresolved dynamic calls)\n",
        status.total_nodes,
        status.function_nodes,
        status.test_nodes,
        status.total_edges,
        status.resolved_calls,
        status.unresolved_calls
    ));
    out.push_str(&format!("- **Language Distribution:** {lang_summary}\n"));
    out.push_str(&format!(
        "- **Runtime Telemetry Hydrated:** `{}` (`{}` hot paths, `{}` bottlenecks)\n\n",
        if status.hydrated_nodes > 0 { "Yes" } else { "Cold-Start Static + PageRank Mode" },
        status.hot_paths,
        status.bottlenecks
    ));

    // 4. Community Architecture Clusters
    if !arch.clusters.is_empty() {
        out.push_str("## 4. Architectural Modules (Label-Propagation Communities)\n\n");
        out.push_str("| Module Cluster | Symbols | Internal Cohesion | Primary Symbols |\n");
        out.push_str("| :--- | :---: | :---: | :--- |\n");
        for cluster in arch.clusters.iter().take(12) {
            let top = if cluster.top_nodes.is_empty() {
                "—".to_string()
            } else {
                cluster
                    .top_nodes
                    .iter()
                    .take(4)
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!(
                "| `{}` | {} | {:.0}% | {} |\n",
                cluster.label,
                cluster.member_count,
                cluster.cohesion_score * 100.0,
                top
            ));
        }
        out.push('\n');
    }

    // 5. High-Centrality Hub Symbols (PageRank)
    if !arch.top_pagerank_hubs.is_empty() {
        out.push_str("## 5. High-Centrality Hub Symbols (Blast-Radius Sensitive)\n\n");
        out.push_str(
            "Modifying these structural hubs affects the largest transitive caller trees. **Always run `chronos_blast_radius` before changing their signatures or invariants:**\n\n",
        );
        out.push_str("| Symbol | Kind | File | PageRank | Fan-In | Fan-Out | Transitive Loop Depth |\n");
        out.push_str("| :--- | :---: | :--- | :---: | :---: | :---: | :---: |\n");
        for hub in arch.top_pagerank_hubs.iter().take(10) {
            let rel_file = relativize_path(&hub.node.file, workspace);
            out.push_str(&format!(
                "| `{}` | `{}` | `{}:{}` | `{:.4}` | {} | {} | {} |\n",
                hub.node.name,
                hub.node.kind,
                rel_file,
                hub.node.line,
                hub.pagerank,
                hub.node.fan_in,
                hub.node.fan_out,
                hub.node.transitive_loop_depth
            ));
        }
        out.push('\n');
    }

    // 6. Algorithmic Complexity Hotspots
    if !arch.algorithmic_hotspots.is_empty() {
        out.push_str("## 6. Algorithmic Complexity & Loop Hotspots\n\n");
        out.push_str(
            "Functions with interprocedural nested loops (`transitive_loop_depth >= 2`), linear scans inside loops ($O(N^2)$ risk), or recursion:\n\n",
        );
        for h in arch.algorithmic_hotspots.iter().take(10) {
            let rel_file = relativize_path(&h.file, workspace);
            out.push_str(&format!(
                "- **`{}`** (`{}:{}`) — `transitive_loop_depth={}`, `linear_scan_in_loop={}`, `alloc_in_loop={}`, `recursive={}`\n",
                h.name,
                rel_file,
                h.line,
                h.transitive_loop_depth,
                h.linear_scan_in_loop,
                h.alloc_in_loop,
                h.is_recursive
            ));
        }
        out.push('\n');
    }

    // 7. Cross-Boundary HTTP API Routes
    let routes = graph.nodes_by_kind(&NodeKind::ApiRoute);
    if !routes.is_empty() {
        out.push_str("## 7. Cross-Boundary API Routes (Frontend <-> Backend Stitched)\n\n");
        for route in routes.iter().take(15) {
            let callers: Vec<String> = graph
                .callers_of(&route.id)
                .into_iter()
                .map(|c| format!("`{}`", c.name))
                .collect();
            let caller_str = if callers.is_empty() {
                "no stitched callers".to_string()
            } else {
                format!("called by {}", callers.join(", "))
            };
            let rel_file = relativize_path(&route.location.file.to_string_lossy(), workspace);
            out.push_str(&format!(
                "- **`{}`** (`{}:{}`) — {}\n",
                route.name,
                rel_file,
                route.location.start_line,
                caller_str
            ));
        }
        out.push('\n');
    }

    // 8. 5th Dimension: Git Temporal Co-Change Coupling
    let mut co_change_pairs: Vec<(String, String, usize)> = Vec::new();
    for (src_id, dst_id, edge) in graph.all_edges() {
        if matches!(edge.kind, EdgeKind::CoChangesWith) {
            if let (Some(s), Some(d)) = (graph.get_node(&src_id), graph.get_node(&dst_id)) {
                let conf = edge.call_site_line.unwrap_or(50);
                co_change_pairs.push((
                    relativize_path(&s.name, workspace),
                    relativize_path(&d.name, workspace),
                    conf,
                ));
            }
        }
    }
    if co_change_pairs.is_empty() {
        let git_couplings = crate::graph::git_coupling::mine_git_co_changes(workspace, 200);
        for c in git_couplings {
            co_change_pairs.push((
                c.file_a,
                c.file_b,
                c.confidence_pct,
            ));
        }
    }
    co_change_pairs.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    co_change_pairs.dedup();

    if !co_change_pairs.is_empty() {
        out.push_str("## 8. Git Temporal Co-Change Coupling (`CO_CHANGES_WITH`)\n\n");
        out.push_str(
            "Files that historically change together in Git commits. When modifying the left file, verify whether the right partner also requires updates:\n\n",
        );
        for (a, b, conf) in co_change_pairs.iter().take(12) {
            out.push_str(&format!("- `{a}` $\\rightarrow$ `{b}` (**{conf}%** co-change confidence)\n"));
        }
        out.push('\n');
    }

    out
}

fn detect_workspace_commands(workspace: &Path) -> Vec<(&'static str, &'static str)> {
    let mut cmds = Vec::new();
    if workspace.join("Cargo.toml").exists() {
        cmds.push(("Rust Build", "cargo build --workspace -j 4"));
        cmds.push(("Rust Test Suite", "cargo test --workspace -j 4"));
    }
    if workspace.join("package.json").exists() {
        cmds.push(("Node / TypeScript Tests", "npm test"));
    }
    if workspace.join("pyproject.toml").exists() || workspace.join("pytest.ini").exists() {
        cmds.push(("Python Test Suite", "pytest"));
    }
    if workspace.join("go.mod").exists() {
        cmds.push(("Go Test Suite", "go test ./..."));
    }
    if cmds.is_empty() {
        cmds.push(("CHRONOS Audit", "chronos arch"));
    }
    cmds
}

fn relativize_path(raw: &str, workspace: &Path) -> String {
    let norm_raw = raw.replace('\\', "/");
    let norm_ws = workspace
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if !norm_ws.is_empty() && norm_ws != "." {
        if let Some(stripped) = norm_raw.strip_prefix(&norm_ws) {
            let clean = stripped.trim_start_matches('/');
            if !clean.is_empty() {
                return clean.to_string();
            }
        }
    }
    norm_raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::{ChronosEdge, ChronosNode, EdgeKind, NodeId, NodeKind, SourceLocation};
    use std::path::PathBuf;

    fn sample_graph() -> ChronosGraph {
        let mut graph = ChronosGraph::new();
        let fn_a = NodeId::new("src/auth.rs", "verify_token", &NodeKind::Function);
        let fn_b = NodeId::new("src/api.rs", "login_handler", &NodeKind::Function);

        let mut node_a = ChronosNode::new(
            fn_a.clone(),
            NodeKind::Function,
            "verify_token",
            "src/auth.rs::verify_token",
            SourceLocation {
                file: PathBuf::from("src/auth.rs"),
                start_line: 10,
                start_col: 0,
                end_line: 40,
                end_col: 0,
            },
        );
        node_a.heuristics.loop_depth = 2;
        node_a.heuristics.transitive_loop_depth = 2;
        node_a.heuristics.linear_scan_in_loop = 1;

        let node_b = ChronosNode::new(
            fn_b.clone(),
            NodeKind::Function,
            "login_handler",
            "src/api.rs::login_handler",
            SourceLocation {
                file: PathBuf::from("src/api.rs"),
                start_line: 5,
                start_col: 0,
                end_line: 25,
                end_col: 0,
            },
        );

        graph.add_node(node_a);
        graph.add_node(node_b);
        graph
            .add_edge(&fn_b, &fn_a, ChronosEdge::new(EdgeKind::Calls))
            .unwrap();

        let mut co_edge = ChronosEdge::new(EdgeKind::CoChangesWith);
        co_edge.call_site_line = Some(88);
        graph.add_edge(&fn_b, &fn_a, co_edge).unwrap();
        graph
    }

    #[test]
    fn test_synthesize_agents_md_contains_clusters_hubs_and_co_changes() {
        let graph = sample_graph();
        let md = synthesize_agents_md(&graph, Path::new("."), Some("demo-repo"));
        assert!(md.contains("# AGENTS.md — demo-repo"));
        assert!(md.contains("verify_token"));
        assert!(md.contains("High-Centrality Hub Symbols"));
        assert!(md.contains("Algorithmic Complexity & Loop Hotspots"));
        assert!(md.contains("88%"));
    }

    #[test]
    fn test_synthesize_multi_agent_targets_and_scaffolding() {
        let graph = sample_graph();

        let claude = synthesize_target_context(&graph, Path::new("."), AgentTarget::ClaudeCode, Some("demo-repo"));
        assert!(claude.contains("# CLAUDE.md — demo-repo"));
        assert!(claude.contains("verify_token"));

        let agy = synthesize_target_context(&graph, Path::new("."), AgentTarget::Antigravity, Some("demo-repo"));
        assert!(agy.contains("# GEMINI.md — demo-repo"));
        assert!(agy.contains(".agents/skills/chronos-agent/SKILL.md"));

        let cursor = synthesize_target_context(&graph, Path::new("."), AgentTarget::Cursor, Some("demo-repo"));
        assert!(cursor.contains("alwaysApply: true"));

        let files = scaffold_client_files(&graph, Path::new("."), AgentTarget::All, "chronos-mcp", Some("demo-repo"));
        assert!(files.iter().any(|f| f.relative_path == "AGENTS.md"));
        assert!(files.iter().any(|f| f.relative_path == "CLAUDE.md"));
        assert!(files.iter().any(|f| f.relative_path == "GEMINI.md"));
        assert!(files.iter().any(|f| f.relative_path == ".mcp.json"));
        assert!(files.iter().any(|f| f.relative_path == ".agents/skills/chronos-agent/SKILL.md"));
        assert!(files.iter().any(|f| f.relative_path == ".agents/plugins/chronos-agent/mcp_config.json"));
        assert!(files.iter().any(|f| f.relative_path == "opencode.json"));
        assert!(files.iter().any(|f| f.relative_path == ".cursor/rules/chronos-5d.mdc"));
    }
}
