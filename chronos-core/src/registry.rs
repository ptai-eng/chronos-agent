//! # ToolRegistry — Single Canonical Tool Schema & Response Budgeting Registry
//!
//! Single source of truth for all CHRONOS tool definitions shared between:
//! - `chronos-mcp` (`tools/list`, dispatch guardrails, `_meta`, `max_answer_chars` budgeting)
//! - `chronos-cli` (`chronos schema --format <openai|responses|anthropic|gemini|codex-toml|mcp>`)
//!
//! Eliminates schema drift (BUG-04) across every supported AI runtime.

use anyhow::Result;
use serde_json::{json, Map, Value};

/// Default upper character budget applied to every MCP tool response unless overridden
/// via `max_answer_chars`.
pub const DEFAULT_MAX_ANSWER_CHARS: usize = 24_000;

/// Supported external tool-schema export targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormat {
    OpenAi,
    Responses,
    Anthropic,
    Gemini,
    Mcp,
    CodexToml,
}

impl SchemaFormat {
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "responses" => Self::Responses,
            "anthropic" | "claude" => Self::Anthropic,
            "gemini" | "antigravity" | "google" => Self::Gemini,
            "mcp" => Self::Mcp,
            "codex-toml" | "toml" => Self::CodexToml,
            _ => Self::OpenAi,
        }
    }
}

/// Typed static default value for MCP/UI parameter hints.
#[derive(Debug, Clone, Copy)]
pub enum DefaultVal {
    Int(i64),
    Bool(bool),
    Str(&'static str),
}

impl DefaultVal {
    pub fn to_json(self) -> Value {
        match self {
            Self::Int(i) => Value::from(i),
            Self::Bool(b) => Value::from(b),
            Self::Str(s) => Value::from(s),
        }
    }
}

/// Canonical parameter specification used to derive MCP & OpenAI/Anthropic/Gemini schemas.
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub name: &'static str,
    pub json_type: &'static str,
    pub description: Option<&'static str>,
    pub enum_values: Option<&'static [&'static str]>,
    pub default_val: Option<DefaultVal>,
    pub required_in_mcp: bool,
    pub nullable_in_strict: bool,
}

const MAX_ANSWER_CHARS_PARAM: ParamSpec = ParamSpec {
    name: "max_answer_chars",
    json_type: "integer",
    description: Some(
        "Optional maximum character budget for the response (default: 24000; <= 0 disables truncation)",
    ),
    enum_values: None,
    default_val: Some(DefaultVal::Int(DEFAULT_MAX_ANSWER_CHARS as i64)),
    required_in_mcp: false,
    nullable_in_strict: true,
};

/// MCP `ToolAnnotations` behavioral metadata attached to every canonical tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolAnnotationsSpec {
    pub title: &'static str,
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

impl ToolAnnotationsSpec {
    /// Read-only, deterministic query against local workspace/graph state.
    pub const fn read_only(title: &'static str) -> Self {
        Self {
            title,
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        }
    }
}

/// Canonical definition of a single CHRONOS tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// True if tool never modifies files inside user source directories directly.
    pub readonly: bool,
    pub annotations: ToolAnnotationsSpec,
    pub params: &'static [ParamSpec],
}

impl ToolSpec {
    /// Render MCP `annotations` metadata JSON object.
    pub fn mcp_annotations(&self) -> Value {
        json!({
            "title": self.annotations.title,
            "readOnlyHint": self.annotations.read_only_hint,
            "destructiveHint": self.annotations.destructive_hint,
            "idempotentHint": self.annotations.idempotent_hint,
            "openWorldHint": self.annotations.open_world_hint,
        })
    }

    /// Standard MCP `inputSchema` (`required` contains only mandatory arguments).
    pub fn to_mcp_schema(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for p in self.params {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String(p.json_type.to_string()));
            if p.json_type == "array" {
                obj.insert("items".to_string(), json!({ "type": "string" }));
            }
            if let Some(desc) = p.description {
                obj.insert("description".to_string(), Value::String(desc.to_string()));
            }
            if let Some(enums) = p.enum_values {
                obj.insert(
                    "enum".to_string(),
                    Value::Array(enums.iter().map(|e| Value::String((*e).to_string())).collect()),
                );
            }
            if let Some(def) = p.default_val {
                obj.insert("default".to_string(), def.to_json());
            }
            props.insert(p.name.to_string(), Value::Object(obj));
            if p.required_in_mcp {
                required.push(Value::String(p.name.to_string()));
            }
        }
        json!({
            "type": "object",
            "properties": Value::Object(props),
            "required": Value::Array(required),
        })
    }

    /// OpenAI `strict: true` JSON Schema (every property is listed in `required`, optional params allow `"null"`).
    pub fn to_strict_openai_schema(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for p in self.params {
            let mut obj = Map::new();
            let ty = if p.nullable_in_strict {
                json!([p.json_type, "null"])
            } else {
                Value::String(p.json_type.to_string())
            };
            obj.insert("type".to_string(), ty);
            if p.json_type == "array" {
                obj.insert("items".to_string(), json!({ "type": "string" }));
            }
            if let Some(desc) = p.description {
                obj.insert("description".to_string(), Value::String(desc.to_string()));
            }
            if let Some(enums) = p.enum_values {
                obj.insert(
                    "enum".to_string(),
                    Value::Array(enums.iter().map(|e| Value::String((*e).to_string())).collect()),
                );
            }
            props.insert(p.name.to_string(), Value::Object(obj));
            required.push(Value::String(p.name.to_string()));
        }
        json!({
            "type": "object",
            "properties": Value::Object(props),
            "required": Value::Array(required),
            "additionalProperties": false,
        })
    }

    /// Anthropic Claude `input_schema`.
    pub fn to_anthropic_schema(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for p in self.params {
            let mut obj = Map::new();
            let ty = if p.nullable_in_strict {
                json!([p.json_type, "null"])
            } else {
                Value::String(p.json_type.to_string())
            };
            obj.insert("type".to_string(), ty);
            if p.json_type == "array" {
                obj.insert("items".to_string(), json!({ "type": "string" }));
            }
            if let Some(desc) = p.description {
                obj.insert("description".to_string(), Value::String(desc.to_string()));
            }
            if let Some(enums) = p.enum_values {
                obj.insert(
                    "enum".to_string(),
                    Value::Array(enums.iter().map(|e| Value::String((*e).to_string())).collect()),
                );
            }
            props.insert(p.name.to_string(), Value::Object(obj));
            if p.required_in_mcp {
                required.push(Value::String(p.name.to_string()));
            }
        }
        json!({
            "type": "object",
            "properties": Value::Object(props),
            "required": Value::Array(required),
        })
    }

    /// Google Gemini / Vertex AI `parameters` (single OpenAPI scalar `"type"`, optional `"nullable": true`).
    pub fn to_gemini_schema(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for p in self.params {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String(p.json_type.to_string()));
            if p.json_type == "array" {
                obj.insert("items".to_string(), json!({ "type": "string" }));
            }
            if p.nullable_in_strict {
                obj.insert("nullable".to_string(), Value::Bool(true));
            }
            if let Some(desc) = p.description {
                obj.insert("description".to_string(), Value::String(desc.to_string()));
            }
            if let Some(enums) = p.enum_values {
                obj.insert(
                    "enum".to_string(),
                    Value::Array(enums.iter().map(|e| Value::String((*e).to_string())).collect()),
                );
            }
            props.insert(p.name.to_string(), Value::Object(obj));
            if p.required_in_mcp {
                required.push(Value::String(p.name.to_string()));
            }
        }
        json!({
            "type": "object",
            "properties": Value::Object(props),
            "required": Value::Array(required),
        })
    }
}

const HOTSPOT_FILTERS: &[&str] = &["bottlenecks", "hot_paths", "high_complexity", "all"];
const DETAIL_MODES: &[&str] = &["summary", "complete"];
const EDIT_OPERATIONS: &[&str] = &["replace_body", "insert_before", "insert_after"];
const REFACTOR_ACTIONS: &[&str] = &["preview_usages", "rename", "safe_delete"];
const AGENT_TARGETS: &[&str] = &[
    "codex",
    "antigravity",
    "claude",
    "opencode",
    "cursor",
    "windsurf",
    "copilot",
    "cline",
    "all",
];

const ANALYZE_FILE_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "file_path",
        json_type: "string",
        description: Some("Absolute or workspace-relative path to the source file to analyze"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const QUERY_HOTSPOTS_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "top_n",
        json_type: "integer",
        description: Some("Number of hotspots to return (default: 10)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(10)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "filter",
        json_type: "string",
        description: Some("Filter type (default: all)"),
        enum_values: Some(HOTSPOT_FILTERS),
        default_val: Some(DefaultVal::Str("all")),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const BLAST_RADIUS_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "symbol_name",
        json_type: "string",
        description: Some("The function/class/name_path symbol to analyze (e.g. 'getUser', 'OrderService/create')"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "file_path",
        json_type: "string",
        description: Some("File containing the symbol (helps disambiguate same-name symbols)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "contract_breaking",
        json_type: "boolean",
        description: Some("Set true if this change modifies the public API signature"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "detail",
        json_type: "string",
        description: Some("Output verbosity: 'summary' (compact 30s glance) or 'complete' (writes full HTML dashboard)"),
        enum_values: Some(DETAIL_MODES),
        default_val: Some(DefaultVal::Str("summary")),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const MUTATION_GATE_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "file_path",
        json_type: "string",
        description: Some("Path to the source file to mutation-test"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "test_command",
        json_type: "string",
        description: Some("Test runner command (default: 'npx jest --passWithNoTests')"),
        enum_values: None,
        default_val: Some(DefaultVal::Str("npx jest --passWithNoTests")),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "strict",
        json_type: "boolean",
        description: Some("Use 90% threshold (release gate). Default false = 80% dev gate."),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const CONTRACT_STATUS_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "watch_dir",
        json_type: "string",
        description: Some("Directory to index for contract changes (optional)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const QUERY_GRAPH_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "query",
        json_type: "string",
        description: Some("Read-only Cypher query (e.g. 'MATCH (f:ChronosNode) WHERE f.transitive_loop_depth >= 2 RETURN f.name, f.file, f.transitive_loop_depth')"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "max_rows",
        json_type: "integer",
        description: Some("Maximum rows to return (default: 500, ceiling: 100000)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(500)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const SEARCH_GRAPH_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "query",
        json_type: "string",
        description: Some("Natural-language or keyword query ranked by BM25 + structural boost + PageRank"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "name_pattern",
        json_type: "string",
        description: Some("Regex pattern to match symbol name or qualified_name"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "file_pattern",
        json_type: "string",
        description: Some("Substring or regex filter on file path"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "kind",
        json_type: "string",
        description: Some("Filter by node kind: Function | Class | ApiRoute | TestCase"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "min_degree",
        json_type: "integer",
        description: Some("Minimum total degree (fan_in + fan_out)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "min_loop_depth",
        json_type: "integer",
        description: Some("Minimum interprocedural nested loop depth (transitive_loop_depth)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "limit",
        json_type: "integer",
        description: Some("Maximum results to return (default: 50)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(50)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "offset",
        json_type: "integer",
        description: Some("Pagination offset (default: 0)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(0)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const GET_SNIPPET_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "symbol_name",
        json_type: "string",
        description: Some("Function, method, class name, or 'Class/method' name_path to retrieve"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "file_path",
        json_type: "string",
        description: Some("Optional file path filter to disambiguate same-name symbols"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const GET_ARCHITECTURE_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "path",
        json_type: "string",
        description: Some("Optional directory prefix to scope architecture analysis"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "auto_index",
        json_type: "boolean",
        description: Some("Automatically scan and incrementally index workspace files (default: true)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(true)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const HYDRATE_TELEMETRY_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "profile_path",
        json_type: "string",
        description: Some("Path to the offline profile file (.cpuprofile, .speedscope.json, .folded, .otlp.json)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "profile_content",
        json_type: "string",
        description: Some("Optional inline profile content string (alternative to profile_path)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const GENERATE_AGENTS_MD_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "target",
        json_type: "string",
        description: Some("Target AI CLI/IDE ecosystem: codex | antigravity | claude | opencode | cursor | windsurf | copilot | cline | all (default: 'codex')"),
        enum_values: Some(AGENT_TARGETS),
        default_val: Some(DefaultVal::Str("codex")),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "project_name",
        json_type: "string",
        description: Some("Optional project title override for the synthesized header"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "write_to_disk",
        json_type: "boolean",
        description: Some("If true, writes the synthesized file(s) directly into the workspace (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "scaffold_configs",
        json_type: "boolean",
        description: Some("If true (with write_to_disk=true), also scaffolds native MCP configs, skills, and slash commands for the target client"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "with_hooks",
        json_type: "boolean",
        description: Some("If true (with write_to_disk=true), also scaffolds <20ms lifecycle hook configs (.agents/hooks.json, .claude/settings.json)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "output_path",
        json_type: "string",
        description: Some("Optional relative workspace path override when writing a single context file"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "auto_index",
        json_type: "boolean",
        description: Some("Automatically scan and incrementally index workspace files first (default: true)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(true)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const EXPLORE_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "query",
        json_type: "string",
        description: Some("Natural-language question, symbol name, 'Class/method' or 'Class/*' wildcard name_path, or regex pattern to explore"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "queries",
        json_type: "array",
        description: Some("Optional batch list of symbol names or 'Class/*' paths to explore in a single round-trip"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "file_filter",
        json_type: "string",
        description: Some("Optional file path substring or regex filter"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "max_symbols",
        json_type: "integer",
        description: Some("Maximum number of symbol bundles to return (default: 3, ceiling: 20)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(3)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "include_body",
        json_type: "boolean",
        description: Some("When false, returns only signature + line/byte spans without full source body to save tokens (default: true)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(true)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "depth",
        json_type: "integer",
        description: Some("Class/module outline expansion depth (default: 1; >1 expands child methods for matched classes)"),
        enum_values: None,
        default_val: Some(DefaultVal::Int(1)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "include_blast_radius",
        json_type: "boolean",
        description: Some("When true, attaches compact inline RiskZone + caller counts per explored symbol (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "script",
        json_type: "string",
        description: Some("Optional bounded ReadOnly Rhai 5D graph query script (no write functions available in MCP profile)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "auto_index",
        json_type: "boolean",
        description: Some("Automatically scan and incrementally index workspace on cold start (default: true)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(true)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const EDIT_SYMBOL_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "symbol",
        json_type: "string",
        description: Some("Target symbol name or 'Class/method' path to edit (resolved via fresh disk AST parse)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "file",
        json_type: "string",
        description: Some("Relative or absolute path within workspace to the target source file"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "operation",
        json_type: "string",
        description: Some("Edit operation: replace_body | insert_before | insert_after"),
        enum_values: Some(EDIT_OPERATIONS),
        default_val: Some(DefaultVal::Str("replace_body")),
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "new_code",
        json_type: "string",
        description: Some("Replacement or inserted code block (indentation and CRLF/LF + BOM are auto-preserved)"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "dry_run",
        json_type: "boolean",
        description: Some("Validate syntax and contract impact without modifying disk (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "force",
        json_type: "boolean",
        description: Some("Override Stage 2 contract blast-radius block when changing a signature with callers (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

const REFACTOR_SYMBOL_PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "symbol",
        json_type: "string",
        description: Some("Target symbol name or 'Class/method' path to analyze, rename, or safely delete"),
        enum_values: None,
        default_val: None,
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "action",
        json_type: "string",
        description: Some("Refactoring action: preview_usages | rename | safe_delete"),
        enum_values: Some(REFACTOR_ACTIONS),
        default_val: Some(DefaultVal::Str("preview_usages")),
        required_in_mcp: true,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "new_name",
        json_type: "string",
        description: Some("New identifier name (required when action == 'rename')"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "file",
        json_type: "string",
        description: Some("Optional defining source file path for symbol disambiguation"),
        enum_values: None,
        default_val: None,
        required_in_mcp: false,
        nullable_in_strict: true,
    },
    ParamSpec {
        name: "dry_run",
        json_type: "boolean",
        description: Some("Preview multi-file diff and candidate sites without writing disk (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "force",
        json_type: "boolean",
        description: Some("Override ambiguity/homonym or dependent guards when applying rename/safe_delete (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    ParamSpec {
        name: "use_lsp",
        json_type: "boolean",
        description: Some("Opt-in hybrid compiler LSP reference resolution to disambiguate candidate_sites; falls back cleanly to AST if unavailable (default: false)"),
        enum_values: None,
        default_val: Some(DefaultVal::Bool(false)),
        required_in_mcp: false,
        nullable_in_strict: false,
    },
    MAX_ANSWER_CHARS_PARAM,
];

/// Single canonical ordered registry of all 14 CHRONOS MCP tools.
pub const CANONICAL_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "chronos_analyze_file",
        description: "Parse a source file (.ts, .tsx, .js, .py, .rs, .go, .c, .cpp, .java) with CHRONOS's polyglot AST engine and insert all symbols, call edges, cross-language HTTP routes, test edges, and loop/complexity metrics into the 5D Code Graph.",
        readonly: true,
        annotations: ToolAnnotationsSpec {
            title: "Analyze File (AST + 5D Graph)",
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        },
        params: ANALYZE_FILE_PARAMS,
    },
    ToolSpec {
        name: "chronos_query_hotspots",
        description: "Query the 5D Code Graph for the top N performance bottlenecks and hot-path functions (using live telemetry or PageRank + O(N^2) loop fallback).",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Query Performance Hotspots"),
        params: QUERY_HOTSPOTS_PARAMS,
    },
    ToolSpec {
        name: "chronos_blast_radius",
        description: "Analyze the blast-radius (change impact) of modifying a specific function or symbol. Returns a risk zone (SAFE/MEDIUM/HIGH/CRITICAL), PageRank centrality, Git temporal co-change partners, confidence interval, coverage completeness, and residual unknowns.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Analyze Symbol Blast Radius"),
        params: BLAST_RADIUS_PARAMS,
    },
    ToolSpec {
        name: "chronos_mutation_gate",
        description: "Run targeted mutation testing on a specific file using CHRONOS's Mutation Arena. Generates mutants, selects covering tests via the 5D Code Graph, runs them in parallel sandboxes, and returns the mutation score.",
        readonly: true,
        annotations: ToolAnnotationsSpec {
            title: "Run Mutation Gate",
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        },
        params: MUTATION_GATE_PARAMS,
    },
    ToolSpec {
        name: "chronos_contract_status",
        description: "Drain the Semantic Contract Broker's Pre-Tool Execution Gate. Returns any pending contract violation alerts detected since the last call.",
        readonly: true,
        annotations: ToolAnnotationsSpec {
            title: "Drain Contract Alerts",
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: false,
            open_world_hint: false,
        },
        params: CONTRACT_STATUS_PARAMS,
    },
    ToolSpec {
        name: "chronos_query_graph",
        description: "Execute a Read-Only Cypher query directly against the embedded KuzuDB 5D Code Graph. Every ChronosNode has properties: id, name, qualified_name, kind, file, start_line, end_line, start_byte, end_byte, cognitive_load, cyclomatic, fan_in, fan_out, param_count, sloc, loop_count, loop_depth, transitive_loop_depth, linear_scan_in_loop, alloc_in_loop, is_recursive, is_hydrated, exec_time_p50, exec_time_p99, invocation_count, cpu_percent, is_hot_path, is_bottleneck. Relationship tables: CALLS, CONTAINS, TESTS, IMPORTS, CO_CHANGES_WITH.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Query 5D Graph (Read-Only Cypher)"),
        params: QUERY_GRAPH_PARAMS,
    },
    ToolSpec {
        name: "chronos_search_graph",
        description: "Search the 5D Code Graph using BM25 ranked identifier tokenization (splits camelCase/snake_case), regex pattern matching, degree filtering, and nested-loop complexity filtering.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Search 5D Code Graph"),
        params: SEARCH_GRAPH_PARAMS,
    },
    ToolSpec {
        name: "chronos_get_snippet",
        description: "Read the verbatim source code (start_line..=end_line) of a function/class from disk along with its callers, callees, covering tests, PageRank, and AST complexity metrics.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Read Verbatim Symbol Snippet"),
        params: GET_SNIPPET_PARAMS,
    },
    ToolSpec {
        name: "chronos_get_architecture",
        description: "Get a high-level architecture overview: community clusters (Label Propagation + Cohesion score), language breakdown, entry points, top PageRank hubs, and hidden O(N^2) algorithmic hotspots.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Get Architecture Overview"),
        params: GET_ARCHITECTURE_PARAMS,
    },
    ToolSpec {
        name: "chronos_hydrate_telemetry",
        description: "Hydrate the 4th Dimension (runtime telemetry) of the ChronosGraph from an offline profiler artifact (V8 .cpuprofile JSON, Speedscope/py-spy JSON, Folded Stack Flamegraph text, or OpenTelemetry OTLP trace JSON).",
        readonly: true,
        annotations: ToolAnnotationsSpec {
            title: "Hydrate Runtime Telemetry",
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        },
        params: HYDRATE_TELEMETRY_PARAMS,
    },
    ToolSpec {
        name: "chronos_generate_agents_md",
        description: "Synthesize live, data-driven context rules, skills, and MCP configs from the 5D Code Graph (AGENTS.md, CLAUDE.md, GEMINI.md, .agents/skills/chronos-agent/SKILL.md, .cursor/rules/chronos-5d.mdc) for OpenAI Codex CLI, Google Antigravity, Claude Code, OpenCode, Cursor, Windsurf, Copilot, and Cline.",
        readonly: true,
        annotations: ToolAnnotationsSpec {
            title: "Generate Agent Context & Rules",
            read_only_hint: false,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        },
        params: GENERATE_AGENTS_MD_PARAMS,
    },
    ToolSpec {
        name: "chronos_explore",
        description: "1-Call Subagent Context Engine (<15ms). Answers code exploration questions in a single call by returning the top BM25/regex matching symbols, wildcard Class/* outlines, signature-only views (include_body=false), inline blast-radius, read-only Rhai 5D scripts, caller/callee chains, verification surfaces, and 4-angle review diagnostics.",
        readonly: true,
        annotations: ToolAnnotationsSpec::read_only("Explore Codebase (1-Call Context)"),
        params: EXPLORE_PARAMS,
    },
    ToolSpec {
        name: "chronos_edit_symbol",
        description: "Perform surgical AST-bounded edits (replace_body, insert_before, insert_after) against a fresh disk parse with leading trivia deduplication, indentation/CRLF/BOM preservation, and a 2-stage Tree-sitter syntax + contract blast-radius gate (blocked_requires_force on breaking signature changes with callers unless force=true).",
        readonly: false,
        annotations: ToolAnnotationsSpec {
            title: "Edit Symbol (AST Gated)",
            read_only_hint: false,
            destructive_hint: true,
            idempotent_hint: false,
            open_world_hint: false,
        },
        params: EDIT_SYMBOL_PARAMS,
    },
    ToolSpec {
        name: "chronos_refactor_symbol",
        description: "Perform graph-edge-verified multi-file refactoring (preview_usages, rename, safe_delete). Separates verified CALLS/TESTS/import edits from ambiguous member-access/shadowed candidate sites, enforces multi-file syntax validation + atomic rollback, and guards public/entrypoint/interface-impl deletions.",
        readonly: false,
        annotations: ToolAnnotationsSpec {
            title: "Refactor Symbol (Edge-Verified)",
            read_only_hint: false,
            destructive_hint: true,
            idempotent_hint: false,
            open_world_hint: false,
        },
        params: REFACTOR_SYMBOL_PARAMS,
    },
];

/// Canonical registry API shared by `chronos-mcp` and `chronos-cli`.
pub struct ToolRegistry;

impl ToolRegistry {
    /// Return tool definitions filtered by `include_write_tools`.
    pub fn specs(include_write_tools: bool) -> Vec<&'static ToolSpec> {
        CANONICAL_TOOLS
            .iter()
            .filter(|t| include_write_tools || t.readonly)
            .collect()
    }

    /// Look up a single tool specification by name.
    pub fn find(name: &str) -> Option<&'static ToolSpec> {
        CANONICAL_TOOLS.iter().find(|t| t.name == name)
    }

    /// Build MCP `tools/list` JSON values.
    pub fn mcp_tools_json(include_write_tools: bool) -> Vec<Value> {
        Self::specs(include_write_tools)
            .into_iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.to_mcp_schema(),
                    "annotations": t.mcp_annotations(),
                })
            })
            .collect()
    }

    /// Export schemas in any supported AI SDK / CLI format.
    pub fn export_schema(format: SchemaFormat) -> Result<String> {
        let specs = Self::specs(true);
        match format {
            SchemaFormat::CodexToml => Ok(
                "# ~/.codex/config.toml — OpenAI Codex CLI Configuration for CHRONOS-AGENT\n\
                 [mcp_servers.chronos-agent]\n\
                 command = \"chronos-mcp\"\n\
                 args = [\"--workspace\", \".\"]\n"
                    .to_string(),
            ),
            SchemaFormat::Responses => {
                let mapped: Vec<Value> = specs
                    .into_iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "name": t.name,
                            "description": t.description,
                            "strict": true,
                            "parameters": t.to_strict_openai_schema(),
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&mapped)?)
            }
            SchemaFormat::Anthropic => {
                let mapped: Vec<Value> = specs
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.to_anthropic_schema(),
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&mapped)?)
            }
            SchemaFormat::Gemini => {
                let declarations: Vec<Value> = specs
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.to_gemini_schema(),
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&json!([{
                    "functionDeclarations": declarations
                }]))?)
            }
            SchemaFormat::Mcp => {
                let mapped = Self::mcp_tools_json(true);
                Ok(serde_json::to_string_pretty(&json!({ "tools": mapped }))?)
            }
            SchemaFormat::OpenAi => {
                let mapped: Vec<Value> = specs
                    .into_iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "strict": true,
                                "parameters": t.to_strict_openai_schema(),
                            }
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&mapped)?)
            }
        }
    }
}

/// Resolve effective `max_answer_chars` from optional tool arguments.
/// Returns `None` if explicitly set to `<= 0` (unlimited).
pub fn resolve_max_answer_chars(arguments: Option<&Value>) -> Option<usize> {
    match arguments
        .and_then(|a| a.get("max_answer_chars"))
        .and_then(|v| v.as_i64())
    {
        Some(n) if n <= 0 => None,
        Some(n) => Some(n as usize),
        None => Some(DEFAULT_MAX_ANSWER_CHARS),
    }
}

/// Truncate text safely on a UTF-8 character boundary within `max_answer_chars` characters,
/// preferring a nearby newline boundary and appending an explicit truncation notice.
pub fn truncate_to_budget(text: &str, max_answer_chars: usize) -> String {
    if max_answer_chars == 0 {
        return text.to_string();
    }
    let total_chars = text.chars().count();
    if total_chars <= max_answer_chars {
        return text.to_string();
    }

    // Reserve room for notice when budget allows so total length stays well-bounded.
    let notice_reserve = if max_answer_chars > 320 { 220 } else { 0 };
    let target_chars = max_answer_chars.saturating_sub(notice_reserve).max(16).min(max_answer_chars);

    let mut cutoff_byte = text.len();
    for (idx, (byte_pos, _)) in text.char_indices().enumerate() {
        if idx == target_chars {
            cutoff_byte = byte_pos;
            break;
        }
    }

    // Snap backward to a nearby newline if it preserves >= 75% of kept slice
    if let Some(nl) = text[..cutoff_byte].rfind('\n') {
        if nl * 4 >= cutoff_byte * 3 {
            cutoff_byte = nl;
        }
    }

    let kept = &text[..cutoff_byte];
    let kept_chars = kept.chars().count();
    let notice = format!(
        "\n\n... [TRUNCATED by CHRONOS ToolRegistry: showed {}/{} chars (max_answer_chars={}). Narrow query/file_path or pass higher max_answer_chars.]",
        kept_chars, total_chars, max_answer_chars
    );

    if notice_reserve == 0 && kept_chars + notice.chars().count() > max_answer_chars {
        // Extremely tiny budget: still mark cleanly
        format!("{kept}...")
    } else {
        format!("{kept}{notice}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn prop_keys(schema: &Value) -> BTreeSet<String> {
        schema["properties"]
            .as_object()
            .expect("properties object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn test_zero_schema_drift_across_mcp_openai_responses_anthropic_and_gemini() {
        let mcp_json: Value = serde_json::from_str(&ToolRegistry::export_schema(SchemaFormat::Mcp).unwrap()).unwrap();
        let openai_json: Value = serde_json::from_str(&ToolRegistry::export_schema(SchemaFormat::OpenAi).unwrap()).unwrap();
        let resp_json: Value = serde_json::from_str(&ToolRegistry::export_schema(SchemaFormat::Responses).unwrap()).unwrap();
        let anth_json: Value = serde_json::from_str(&ToolRegistry::export_schema(SchemaFormat::Anthropic).unwrap()).unwrap();
        let gem_json: Value = serde_json::from_str(&ToolRegistry::export_schema(SchemaFormat::Gemini).unwrap()).unwrap();

        let mcp_tools = mcp_json["tools"].as_array().unwrap();
        let openai_tools = openai_json.as_array().unwrap();
        let resp_tools = resp_json.as_array().unwrap();
        let anth_tools = anth_json.as_array().unwrap();
        let gem_tools = gem_json[0]["functionDeclarations"].as_array().unwrap();

        assert_eq!(mcp_tools.len(), 14);
        assert_eq!(openai_tools.len(), 14);
        assert_eq!(resp_tools.len(), 14);
        assert_eq!(anth_tools.len(), 14);
        assert_eq!(gem_tools.len(), 14);

        // Read-only filtering hides write tools (#13 & #14) cleanly
        assert_eq!(ToolRegistry::specs(false).len(), 12);
        assert_eq!(ToolRegistry::specs(true).len(), 14);

        for i in 0..14 {
            let name = mcp_tools[i]["name"].as_str().unwrap();
            assert_eq!(openai_tools[i]["function"]["name"], name);
            assert_eq!(resp_tools[i]["name"], name);
            assert_eq!(anth_tools[i]["name"], name);
            assert_eq!(gem_tools[i]["name"], name);

            let mcp_keys = prop_keys(&mcp_tools[i]["inputSchema"]);
            assert_eq!(prop_keys(&openai_tools[i]["function"]["parameters"]), mcp_keys, "OpenAI drift for {name}");
            assert_eq!(prop_keys(&resp_tools[i]["parameters"]), mcp_keys, "Responses drift for {name}");
            assert_eq!(prop_keys(&anth_tools[i]["input_schema"]), mcp_keys, "Anthropic drift for {name}");
            assert_eq!(prop_keys(&gem_tools[i]["parameters"]), mcp_keys, "Gemini drift for {name}");

            // OpenAI strict invariant: every property must appear in required
            let strict_req: BTreeSet<String> = openai_tools[i]["function"]["parameters"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            assert_eq!(strict_req, mcp_keys, "OpenAI strict required mismatch for {name}");
        }

        // BUG-04 explicit regression checks
        let search = ToolRegistry::find("chronos_search_graph").unwrap().to_strict_openai_schema();
        assert!(search["properties"].get("offset").is_some(), "BUG-04: offset must exist in strict schema");
        let agents_md = ToolRegistry::find("chronos_generate_agents_md").unwrap().to_strict_openai_schema();
        assert_eq!(agents_md["properties"]["output_path"]["type"], json!(["string", "null"]));

        // v1.8.1 MCP annotations + 5D consistency checks
        for t in mcp_tools {
            let name = t["name"].as_str().unwrap();
            let desc = t["description"].as_str().unwrap();
            assert!(!desc.contains("4D"), "stale 4D reference in description for {name}");
            let ann = t["annotations"].as_object().expect("annotations present on MCP tool");
            assert!(ann.contains_key("title"));
            assert!(ann.contains_key("readOnlyHint"));
            assert!(ann.contains_key("destructiveHint"));
            assert!(ann.contains_key("idempotentHint"));
            assert!(ann.contains_key("openWorldHint"));
        }
        assert_eq!(
            ToolRegistry::find("chronos_explore").unwrap().annotations.read_only_hint,
            true
        );
        assert_eq!(
            ToolRegistry::find("chronos_edit_symbol").unwrap().annotations.destructive_hint,
            true
        );
    }

    #[test]
    fn test_truncate_to_budget_utf8_safe_and_explicit_notice() {
        let sample = format!("{}\n{}", "á".repeat(500), "b".repeat(500));
        let truncated = truncate_to_budget(&sample, 450);
        assert!(truncated.contains("[TRUNCATED by CHRONOS ToolRegistry"));
        assert!(truncated.chars().count() <= 450);

        let unlimited = resolve_max_answer_chars(Some(&json!({ "max_answer_chars": -1 })));
        assert_eq!(unlimited, None);
        assert_eq!(resolve_max_answer_chars(None), Some(DEFAULT_MAX_ANSWER_CHARS));
    }
}
