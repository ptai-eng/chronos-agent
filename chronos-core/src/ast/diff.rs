//! # AST Differ — Fast Incremental Change Detection
//!
//! Computes structural diff between two versions of a module's AST.
//! Used by the Semantic Contract Broker to determine if a change is:
//! 1. **Internal (implementation change)** → no alert needed
//! 2. **Public contract mutation** → alert dependent agents immediately
//!
//! ## Performance target
//! < 5ms for files up to 1000 lines (tree-sitter parses incrementally)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The type of contract change detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractChangeKind {
    /// Function signature changed (parameters, return type)
    FunctionSignatureChanged {
        function_name: String,
        old_signature: String,
        new_signature: String,
    },
    /// A function was added to the public interface
    FunctionAdded { function_name: String },
    /// A function was removed from the public interface
    FunctionRemoved { function_name: String },
    /// An API route's method or path changed
    ApiRouteChanged {
        old_route: String,
        new_route: String,
    },
    /// An API route was added
    ApiRouteAdded { route: String },
    /// An API route was removed
    ApiRouteRemoved { route: String },
    /// Internal implementation change (no contract impact)
    InternalChange { description: String },
}

impl ContractChangeKind {
    /// Returns true if this change affects the public contract (requires agent notification)
    pub fn is_contract_breaking(&self) -> bool {
        !matches!(self, ContractChangeKind::InternalChange { .. })
    }

    /// Human-readable description for agent notification
    pub fn description(&self) -> String {
        match self {
            ContractChangeKind::FunctionSignatureChanged { function_name, old_signature, new_signature } => {
                format!(
                    "⚠️ CONTRACT MUTATION: `{}` signature changed\n  Before: {}\n  After:  {}",
                    function_name, old_signature, new_signature
                )
            }
            ContractChangeKind::FunctionAdded { function_name } => {
                format!("✨ New function added to public interface: `{}`", function_name)
            }
            ContractChangeKind::FunctionRemoved { function_name } => {
                format!("🗑️ BREAKING: Function `{}` removed from public interface", function_name)
            }
            ContractChangeKind::ApiRouteChanged { old_route, new_route } => {
                format!(
                    "🌐 API ROUTE CHANGED: `{}` → `{}`",
                    old_route, new_route
                )
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

/// The full diff result between two module snapshots
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
        if matches!(&change,
            ContractChangeKind::FunctionRemoved { .. }
            | ContractChangeKind::ApiRouteRemoved { .. }
            | ContractChangeKind::FunctionSignatureChanged { .. }
            | ContractChangeKind::ApiRouteChanged { .. }
        ) {
            self.has_breaking_changes = true;
        }
        self.changes.push(change);
    }

    /// Generate a compact notification message for agent injection
    pub fn agent_notification(&self) -> Option<String> {
        if !self.has_contract_changes {
            return None;
        }
        let contract_changes: Vec<String> = self.changes.iter()
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

/// Lightweight signature representation for comparison
#[derive(Debug, Clone, PartialEq)]
struct FunctionSignature {
    name: String,
    /// Simplified signature string: "name(param1: T1, param2: T2): ReturnT"
    signature: String,
    is_exported: bool,
}

/// Compute the diff between two text snapshots of a TypeScript file.
/// Uses tree-sitter to parse both and compare function signatures.
///
/// This is a simplified implementation for Phase 1; full bidirectional
/// signature parsing will be enhanced in Phase 4 when Contract Broker is built.
pub fn compute_module_diff(
    file_path: &str,
    old_source: &str,
    new_source: &str,
) -> anyhow::Result<ModuleDiff> {
    let mut diff = ModuleDiff::new(file_path);

    let old_sigs = extract_signatures(old_source);
    let new_sigs = extract_signatures(new_source);

    let old_names: HashSet<&str> = old_sigs.keys().map(|s| s.as_str()).collect();
    let new_names: HashSet<&str> = new_sigs.keys().map(|s| s.as_str()).collect();

    // Functions removed
    for removed in old_names.difference(&new_names) {
        diff.add_change(ContractChangeKind::FunctionRemoved {
            function_name: removed.to_string(),
        });
    }

    // Functions added
    for added in new_names.difference(&old_names) {
        diff.add_change(ContractChangeKind::FunctionAdded {
            function_name: added.to_string(),
        });
    }

    // Functions changed signature
    for name in old_names.intersection(&new_names) {
        let old_sig = &old_sigs[*name];
        let new_sig = &new_sigs[*name];
        if old_sig != new_sig {
            diff.add_change(ContractChangeKind::FunctionSignatureChanged {
                function_name: name.to_string(),
                old_signature: old_sig.clone(),
                new_signature: new_sig.clone(),
            });
        }
    }

    // If no contract changes, mark as internal
    if diff.changes.is_empty() && old_source != new_source {
        diff.add_change(ContractChangeKind::InternalChange {
            description: "Implementation modified, no public interface changes".to_string(),
        });
    }

    Ok(diff)
}

/// Extract simplified function signatures from source text.
/// Uses regex-style line scanning for speed (tree-sitter is used in the full broker).
fn extract_signatures(source: &str) -> std::collections::HashMap<String, String> {
    let mut sigs = std::collections::HashMap::new();
    
    for line in source.lines() {
        let trimmed = line.trim();
        
        // Match: export function foo(...) / export async function foo(...) / function foo(...)
        if let Some(sig) = try_extract_function_signature(trimmed) {
            sigs.insert(sig.0, sig.1);
        }
        
        // Match: export const foo = (...) => / export const foo = async (...) =>
        if let Some(sig) = try_extract_arrow_signature(trimmed) {
            sigs.insert(sig.0, sig.1);
        }
    }
    
    sigs
}

fn try_extract_function_signature(line: &str) -> Option<(String, String)> {
    // Pattern: [export] [async] function name(params...)[: ReturnType]
    let stripped = line
        .trim_start_matches("export ")
        .trim_start_matches("async ");
    
    if !stripped.starts_with("function ") { return None; }
    
    let rest = &stripped["function ".len()..];
    // Find the name (up to '(')
    let paren_pos = rest.find('(')?;
    let name = rest[..paren_pos].trim().to_string();
    if name.is_empty() { return None; }
    
    // Extract everything up to the opening brace as the signature
    let sig_end = rest.find('{').unwrap_or(rest.len());
    let signature = rest[..sig_end].trim().to_string();
    
    Some((name, signature))
}

fn try_extract_arrow_signature(line: &str) -> Option<(String, String)> {
    // Pattern: [export] const name = [async] (params) =>
    let stripped = line.trim_start_matches("export ");
    if !stripped.starts_with("const ") && !stripped.starts_with("let ") { return None; }
    
    let rest = stripped.trim_start_matches("const ").trim_start_matches("let ");
    // Find the variable name
    let eq_pos = rest.find('=')?;
    let name = rest[..eq_pos].trim().to_string();
    if name.is_empty() { return None; }
    
    let after_eq = rest[eq_pos + 1..].trim();
    let is_arrow = after_eq.contains("=>");
    if !is_arrow { return None; }
    
    let sig_end = after_eq.find("=>").map(|p| p + 2).unwrap_or(after_eq.len());
    let signature = format!("{} = {}", name, after_eq[..sig_end].trim());
    
    Some((name, signature))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_SOURCE: &str = r#"
export async function getUser(id: string): Promise<User> {
    return db.find(id);
}

export const createUser = async (email: string): Promise<User> => {
    return db.create({ email });
};
"#;

    const NEW_SOURCE_BREAKING: &str = r#"
export async function getUser(id: string, tenantId: string): Promise<User> {
    return db.find(id, tenantId);
}

export const createUser = async (email: string): Promise<User> => {
    return db.create({ email });
};

export async function deleteUser(id: string): Promise<void> {
    return db.delete(id);
}
"#;

    const NEW_SOURCE_INTERNAL: &str = r#"
export async function getUser(id: string): Promise<User> {
    // Optimized: added caching
    const cached = await cache.get(id);
    if (cached) return cached;
    const user = await db.find(id);
    await cache.set(id, user);
    return user;
}

export const createUser = async (email: string): Promise<User> => {
    return db.create({ email });
};
"#;

    #[test]
    fn test_detects_signature_change() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, NEW_SOURCE_BREAKING)
            .expect("Diff should succeed");
        
        assert!(diff.has_contract_changes, "Should detect contract changes");
        let sig_changes: Vec<_> = diff.changes.iter()
            .filter(|c| matches!(c, ContractChangeKind::FunctionSignatureChanged { .. }))
            .collect();
        assert!(!sig_changes.is_empty(), "Should detect signature change for getUser");
    }

    #[test]
    fn test_detects_new_function() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, NEW_SOURCE_BREAKING)
            .expect("Diff should succeed");
        
        let added: Vec<_> = diff.changes.iter()
            .filter(|c| matches!(c, ContractChangeKind::FunctionAdded { .. }))
            .collect();
        assert!(!added.is_empty(), "Should detect new function deleteUser");
    }

    #[test]
    fn test_internal_change_no_contract_mutation() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, NEW_SOURCE_INTERNAL)
            .expect("Diff should succeed");
        
        // Internal change only — no contract breaking
        assert!(!diff.has_breaking_changes, "Internal change should NOT be breaking");
    }

    #[test]
    fn test_notification_generated_for_breaking() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, NEW_SOURCE_BREAKING)
            .expect("Diff should succeed");
        
        let notification = diff.agent_notification();
        assert!(notification.is_some(), "Should generate notification for contract change");
        let msg = notification.unwrap();
        assert!(msg.contains("CONTRACT ALERT"), "Should mention CONTRACT ALERT");
    }

    #[test]
    fn test_no_notification_for_internal_change() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, NEW_SOURCE_INTERNAL)
            .expect("Diff should succeed");
        
        assert!(diff.agent_notification().is_none(), "No notification for internal change");
    }

    #[test]
    fn test_identical_files_no_changes() {
        let diff = compute_module_diff("src/user.ts", OLD_SOURCE, OLD_SOURCE)
            .expect("Diff should succeed");
        
        assert!(!diff.has_contract_changes);
        assert!(!diff.has_breaking_changes);
    }
}
