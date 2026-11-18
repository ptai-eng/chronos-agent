//! # Polyglot Contract Signature Extractor & Differ
//!
//! Extracts normalized function, method, class, and API route signatures across
//! all 7 supported languages using Tree-sitter (`LanguageDispatcher` + `ParsedModule` byte spans),
//! replacing duplicate TypeScript-only line scanners.

use crate::ast::dispatcher::LanguageDispatcher;
use crate::ast::ts_parser::ParsedModule;
use crate::graph::schema::{ChronosNode, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use xxhash_rust::xxh64::xxh64;

/// The public interface fingerprint of a module (7-language AST-backed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSignature {
    pub file_path: String,
    /// Hash of the entire public interface (for quick change detection)
    pub interface_hash: u64,
    /// Individual callable/class signatures
    pub exports: HashMap<String, FunctionSignature>,
    /// API routes exposed by this module
    pub api_routes: Vec<RouteSignature>,
}

/// A single symbol's normalized contract signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub name: String,
    pub signature_text: String,
    pub signature_hash: u64,
    pub is_async: bool,
    pub is_exported: bool,
}

/// An API route signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteSignature {
    pub method: String,
    pub path: String,
    pub handler_name: Option<String>,
}

/// Detailed contract diff classification (consolidated from removed `ast::diff`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractChangeKind {
    FunctionSignatureChanged {
        function_name: String,
        old_signature: String,
        new_signature: String,
    },
    FunctionAdded {
        function_name: String,
    },
    FunctionRemoved {
        function_name: String,
    },
    ApiRouteChanged {
        old_route: String,
        new_route: String,
    },
    ApiRouteAdded {
        route: String,
    },
    ApiRouteRemoved {
        route: String,
    },
    InternalChange {
        description: String,
    },
}

impl ContractChangeKind {
    pub fn is_contract_breaking(&self) -> bool {
        !matches!(self, ContractChangeKind::InternalChange { .. })
    }

    pub fn description(&self) -> String {
        match self {
            ContractChangeKind::FunctionSignatureChanged {
                function_name,
                old_signature,
                new_signature,
            } => format!(
                "⚠️ CONTRACT MUTATION: `{}` signature changed\n  Before: {}\n  After:  {}",
                function_name, old_signature, new_signature
            ),
            ContractChangeKind::FunctionAdded { function_name } => {
                format!("✨ New function added to public interface: `{}`", function_name)
            }
            ContractChangeKind::FunctionRemoved { function_name } => {
                format!(
                    "🗑️ BREAKING: Function `{}` removed from public interface",
                    function_name
                )
            }
            ContractChangeKind::ApiRouteChanged { old_route, new_route } => {
                format!("🌐 API ROUTE CHANGED: `{}` → `{}`", old_route, new_route)
            }
            ContractChangeKind::ApiRouteAdded { route } => {
                format!("✨ New API route: `{}`", route)
            }
            ContractChangeKind::ApiRouteRemoved { route } => {
                format!("🗑️ BREAKING: API route `{}` removed", route)
            }
            ContractChangeKind::InternalChange { description } => {
                format!("📝 Internal change: {}", description)
            }
        }
    }
}

/// Consolidated module diff between two source snapshots.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleDiff {
    pub file_path: String,
    pub changes: Vec<ContractChangeKind>,
    pub has_contract_changes: bool,
    pub has_breaking_changes: bool,
}

impl ModuleDiff {
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            changes: vec![],
            has_contract_changes: false,
            has_breaking_changes: false,
        }
    }

    pub fn add_change(&mut self, change: ContractChangeKind) {
        if change.is_contract_breaking() {
            self.has_contract_changes = true;
        }
        if matches!(
            &change,
            ContractChangeKind::FunctionRemoved { .. }
                | ContractChangeKind::ApiRouteRemoved { .. }
                | ContractChangeKind::FunctionSignatureChanged { .. }
                | ContractChangeKind::ApiRouteChanged { .. }
        ) {
            self.has_breaking_changes = true;
        }
        self.changes.push(change);
    }

    pub fn agent_notification(&self) -> Option<String> {
        if !self.has_contract_changes {
            return None;
        }
        let contract_changes: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.is_contract_breaking())
            .map(|c| c.description())
            .collect();

        Some(format!(
            "🚨 CHRONOS CONTRACT ALERT — {}\n{}\n\nAction required: Update your code to match the new contract before proceeding.",
            self.file_path,
            contract_changes.join("\n")
        ))
    }
}

/// Extract a normalized single-line contract header for an AST node across any of the 7 languages.
pub fn extract_header_signature(node: &ChronosNode, source: &str) -> String {
    if let Some(ref existing) = node.contract_signature {
        if !existing.is_empty() && !existing.starts_with("ROUTE::") {
            return collapse_ws(existing);
        }
    }

    let bytes = source.as_bytes();
    let slice = if node.location.end_byte > node.location.start_byte
        && node.location.end_byte <= bytes.len()
    {
        &source[node.location.start_byte..node.location.end_byte]
    } else {
        // Fallback by 1-indexed line span
        let start = node.location.start_line.saturating_sub(1);
        let lines: Vec<&str> = source.lines().skip(start).take(5).collect();
        return extract_header_from_text(&lines.join(" "));
    };

    extract_header_from_text(slice)
}

fn extract_header_from_text(text: &str) -> String {
    let trimmed = text.trim();
    // Python def/class header ends at first line's `:` before body newline, or before `{` / `=>` in brace langs
    let mut header = trimmed;
    if let Some(brace_idx) = header.find('{') {
        header = &header[..brace_idx];
    }
    if let Some(arrow_idx) = header.find("=>") {
        header = &header[..arrow_idx];
    }
    // For Python multiline or single-line `def foo(a: int) -> int:`, cut after `):` if followed by newline/stmt
    if trimmed.starts_with("def ") || trimmed.starts_with("async def ") || trimmed.starts_with("class ") {
        if let Some(colon_nl) = header.find(":\n").or_else(|| header.find(":\r\n")) {
            header = &header[..colon_nl];
        } else if let Some(line_end) = header.find('\n') {
            header = &header[..line_end];
        }
        header = header.trim_end_matches(':');
    }
    collapse_ws(header)
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl ModuleSignature {
    /// Extract a polyglot `ModuleSignature` from an already-parsed `ParsedModule` + raw source.
    pub fn from_parsed_module(parsed: &ParsedModule, source: &str) -> Self {
        let mut exports = HashMap::new();
        let mut api_routes = Vec::new();

        for node in &parsed.functions {
            let header = extract_header_signature(node, source);
            let is_async = header.starts_with("async ") || header.contains(" async ");
            let sig_hash = xxh64(header.as_bytes(), 0);
            exports.insert(
                node.name.clone(),
                FunctionSignature {
                    name: node.name.clone(),
                    signature_text: header,
                    signature_hash: sig_hash,
                    is_async,
                    is_exported: true,
                },
            );
        }

        for route in &parsed.api_routes {
            let mut parts = route.name.splitn(2, ' ');
            let method = parts.next().unwrap_or("ANY").to_uppercase();
            let path = parts
                .next()
                .map(|p| p.to_string())
                .or_else(|| {
                    route
                        .contract_signature
                        .as_deref()
                        .and_then(|s| s.strip_prefix("ROUTE::"))
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| route.name.clone());
            api_routes.push(RouteSignature {
                method,
                path,
                handler_name: None,
            });
        }

        let mut sigs: Vec<String> = exports.values().map(|s| s.signature_text.clone()).collect();
        sigs.sort();
        let mut routes: Vec<String> = api_routes
            .iter()
            .map(|r| format!("{} {}", r.method, r.path))
            .collect();
        routes.sort();
        sigs.extend(routes);
        let interface_hash = xxh64(sigs.join("\n").as_bytes(), 0);

        ModuleSignature {
            file_path: parsed.file_path.to_string_lossy().to_string(),
            interface_hash,
            exports,
            api_routes,
        }
    }

    /// Extract a polyglot `ModuleSignature` directly from file extension + source buffer via `LanguageDispatcher`.
    pub fn from_source(file_path: &str, source: &str) -> Self {
        if let Ok(mut dispatcher) = LanguageDispatcher::new() {
            if let Ok(parsed) = dispatcher.parse_source(Path::new(file_path), source) {
                return Self::from_parsed_module(&parsed, source);
            }
        }
        ModuleSignature {
            file_path: file_path.to_string(),
            interface_hash: 0,
            exports: HashMap::new(),
            api_routes: Vec::new(),
        }
    }

    /// Compare two signatures and return a diff description, or `None` when interfaces match.
    pub fn diff(&self, new_sig: &ModuleSignature) -> Option<SignatureDiff> {
        if self.interface_hash == new_sig.interface_hash {
            return None;
        }

        let mut diff = SignatureDiff {
            file_path: self.file_path.clone(),
            added_exports: vec![],
            removed_exports: vec![],
            changed_exports: vec![],
            added_routes: vec![],
            removed_routes: vec![],
            is_breaking: false,
        };

        for (name, old_sig) in &self.exports {
            if !new_sig.exports.contains_key(name) {
                diff.removed_exports.push(name.clone());
                diff.is_breaking = true;
            } else if let Some(new_fn_sig) = new_sig.exports.get(name) {
                if old_sig.signature_hash != new_fn_sig.signature_hash {
                    diff.changed_exports.push(ExportChange {
                        name: name.clone(),
                        old_signature: old_sig.signature_text.clone(),
                        new_signature: new_fn_sig.signature_text.clone(),
                    });
                    diff.is_breaking = true;
                }
            }
        }

        for name in new_sig.exports.keys() {
            if !self.exports.contains_key(name) {
                diff.added_exports.push(name.clone());
            }
        }

        let old_routes: HashSet<String> = self
            .api_routes
            .iter()
            .map(|r| format!("{} {}", r.method, r.path))
            .collect();
        let new_routes: HashSet<String> = new_sig
            .api_routes
            .iter()
            .map(|r| format!("{} {}", r.method, r.path))
            .collect();

        for removed in old_routes.difference(&new_routes) {
            diff.removed_routes.push(removed.clone());
            diff.is_breaking = true;
        }
        for added in new_routes.difference(&old_routes) {
            diff.added_routes.push(added.clone());
        }

        if diff.added_exports.is_empty()
            && diff.removed_exports.is_empty()
            && diff.changed_exports.is_empty()
            && diff.added_routes.is_empty()
            && diff.removed_routes.is_empty()
        {
            return None;
        }

        Some(diff)
    }

    /// Convert to structured `ModuleDiff` across all 7 languages.
    pub fn diff_structured(&self, new_sig: &ModuleSignature) -> ModuleDiff {
        let mut md = ModuleDiff::new(&self.file_path);
        if let Some(d) = self.diff(new_sig) {
            for r in d.removed_exports {
                md.add_change(ContractChangeKind::FunctionRemoved { function_name: r });
            }
            for a in d.added_exports {
                md.add_change(ContractChangeKind::FunctionAdded { function_name: a });
            }
            for ch in d.changed_exports {
                md.add_change(ContractChangeKind::FunctionSignatureChanged {
                    function_name: ch.name,
                    old_signature: ch.old_signature,
                    new_signature: ch.new_signature,
                });
            }
            for rr in d.removed_routes {
                md.add_change(ContractChangeKind::ApiRouteRemoved { route: rr });
            }
            for ar in d.added_routes {
                md.add_change(ContractChangeKind::ApiRouteAdded { route: ar });
            }
        }
        md
    }
}

/// Polyglot AST-backed module diff between two source snapshots (replaces legacy `ast::diff`).
pub fn compute_module_diff(
    file_path: &str,
    old_source: &str,
    new_source: &str,
) -> anyhow::Result<ModuleDiff> {
    let old_sig = ModuleSignature::from_source(file_path, old_source);
    let new_sig = ModuleSignature::from_source(file_path, new_source);
    let mut diff = old_sig.diff_structured(&new_sig);
    if diff.changes.is_empty() && old_source != new_source {
        diff.add_change(ContractChangeKind::InternalChange {
            description: "Implementation modified, no public interface changes".to_string(),
        });
    }
    Ok(diff)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignatureDiff {
    pub file_path: String,
    pub added_exports: Vec<String>,
    pub removed_exports: Vec<String>,
    pub changed_exports: Vec<ExportChange>,
    pub added_routes: Vec<String>,
    pub removed_routes: Vec<String>,
    pub is_breaking: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportChange {
    pub name: String,
    pub old_signature: String,
    pub new_signature: String,
}

impl SignatureDiff {
    pub fn agent_notification(&self) -> String {
        let mut lines = vec![
            format!("🚨 CHRONOS CONTRACT ALERT — `{}`", self.file_path),
            if self.is_breaking {
                "⚠️  BREAKING CHANGES DETECTED — Update your code before proceeding!".to_string()
            } else {
                "ℹ️  Non-breaking interface changes:".to_string()
            },
        ];

        for change in &self.changed_exports {
            lines.push(format!(
                "  • `{}` signature changed:\n    Before: {}\n    After:  {}",
                change.name, change.old_signature, change.new_signature
            ));
        }
        for removed in &self.removed_exports {
            lines.push(format!("  • `{}` REMOVED from public interface", removed));
        }
        for added in &self.added_exports {
            lines.push(format!("  • `{}` added to public interface", added));
        }
        for route in &self.removed_routes {
            lines.push(format!("  • API route `{}` REMOVED", route));
        }
        for route in &self.added_routes {
            lines.push(format!("  • API route `{}` added", route));
        }

        lines.push("\n🎯 Action: Update all call sites and re-validate before committing.".to_string());
        lines.join("\n")
    }
}

#[allow(dead_code)]
fn _node_is_contract_relevant(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Function | NodeKind::Class | NodeKind::ApiRoute)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1_SOURCE: &str = r#"
export async function getUser(id: string): Promise<User> {
    return db.find(id);
}

export const createUser = async (email: string): Promise<User> => {
    return db.create({ email });
};

router.get('/users/:id', handler);
router.post('/users', createHandler);
"#;

    const V2_BREAKING: &str = r#"
export async function getUser(id: string, tenantId: string): Promise<User> {
    return db.find(id, tenantId);
}

export const createUser = async (email: string, role: string): Promise<User> => {
    return db.create({ email, role });
};

export async function deleteUser(id: string): Promise<void> {
    await db.delete(id);
}

router.get('/users/:id', handler);
router.post('/users', createHandler);
"#;

    const V1_INTERNAL: &str = r#"
export async function getUser(id: string): Promise<User> {
    const cached = cache.get(id);
    if (cached) return cached;
    return db.find(id);
}

export const createUser = async (email: string): Promise<User> => {
    return db.create({ email });
};

router.get('/users/:id', handler);
router.post('/users', createHandler);
"#;

    #[test]
    fn test_extract_function_signatures() {
        let sig = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        assert!(sig.exports.contains_key("getUser"));
        assert!(sig.exports.contains_key("createUser"));
    }

    #[test]
    fn test_detect_breaking_change() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2 = ModuleSignature::from_source("src/user.ts", V2_BREAKING);

        let diff = v1.diff(&v2).expect("Should detect diff between v1 and v2");
        assert!(diff.is_breaking);
    }

    #[test]
    fn test_no_diff_on_identical_or_internal_body_edit() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2_same = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        assert!(v1.diff(&v2_same).is_none());

        let v2_internal = ModuleSignature::from_source("src/user.ts", V1_INTERNAL);
        assert!(v1.diff(&v2_internal).is_none(), "Body-only edits must not break contract");
    }

    #[test]
    fn test_agent_notification_format() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2 = ModuleSignature::from_source("src/user.ts", V2_BREAKING);
        let diff = v1.diff(&v2).unwrap();
        let notification = diff.agent_notification();
        assert!(notification.contains("CONTRACT ALERT"));
        assert!(notification.contains("BREAKING"));
    }

    #[test]
    fn test_route_extraction() {
        let sig = ModuleSignature::from_source("src/routes.ts", V1_SOURCE);
        assert!(!sig.api_routes.is_empty());
    }

    #[test]
    fn test_polyglot_python_and_rust_contract_diffs() {
        let py_v1 = "def process_order(order_id: int) -> bool:\n    return True\n";
        let py_v2 = "def process_order(order_id: int, strict: bool) -> bool:\n    return strict\n";
        let py_diff = compute_module_diff("orders.py", py_v1, py_v2).unwrap();
        assert!(py_diff.has_breaking_changes, "Python signature change must trigger breaking diff");

        let rs_v1 = "pub fn verify_token(token: &str) -> bool { token.len() > 5 }\n";
        let rs_internal = "pub fn verify_token(token: &str) -> bool { !token.is_empty() && token.len() > 5 }\n";
        let rs_same = compute_module_diff("auth.rs", rs_v1, rs_internal).unwrap();
        assert!(!rs_same.has_breaking_changes, "Rust body-only edit must not trigger breaking diff");
    }
}
