//! # Signature Extractor — Public Contract Fingerprinting
//!
//! Extracts the "public contract" of a TypeScript module —
//! the surface area that other agents depend on.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use xxhash_rust::xxh64::xxh64;

/// The public interface fingerprint of a module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSignature {
    pub file_path: String,
    /// Hash of the entire public interface (for quick change detection)
    pub interface_hash: u64,
    /// Individual exported function signatures
    pub exports: HashMap<String, FunctionSignature>,
    /// API routes exposed by this module
    pub api_routes: Vec<RouteSignature>,
}

/// A single function's public signature
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub name: String,
    /// Normalized parameter list: "(id: string, options?: Options): Promise<User>"
    pub signature_text: String,
    /// Hash of the signature for fast comparison
    pub signature_hash: u64,
    pub is_async: bool,
    pub is_exported: bool,
}

/// An API route signature
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteSignature {
    pub method: String,
    pub path: String,
    pub handler_name: Option<String>,
}

impl ModuleSignature {
    /// Extract public signature from TypeScript source code
    pub fn from_source(file_path: &str, source: &str) -> Self {
        let exports = extract_exported_functions(source);
        let api_routes = extract_api_routes(source);

        // Compute overall interface hash from all export signatures
        let sig_content: String = {
            let mut sigs: Vec<String> = exports.values()
                .map(|s| s.signature_text.clone())
                .collect();
            sigs.sort();
            let mut routes: Vec<String> = api_routes.iter()
                .map(|r| format!("{} {}", r.method, r.path))
                .collect();
            routes.sort();
            sigs.extend(routes);
            sigs.join("\n")
        };
        let interface_hash = xxh64(sig_content.as_bytes(), 0);

        ModuleSignature {
            file_path: file_path.to_string(),
            interface_hash,
            exports,
            api_routes,
        }
    }

    /// Compare two signatures and return a diff description.
    /// Returns None if signatures are identical.
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

        // Check removed exports
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

        // Check added exports
        for name in new_sig.exports.keys() {
            if !self.exports.contains_key(name) {
                diff.added_exports.push(name.clone());
                // Adding is not breaking (but still noteworthy)
            }
        }

        // Check routes
        let old_routes: std::collections::HashSet<String> = self.api_routes.iter()
            .map(|r| {
                let mut s = String::with_capacity(r.method.len() + r.path.len() + 1);
                s.push_str(&r.method);
                s.push(' ');
                s.push_str(&r.path);
                s
            })
            .collect();
        let new_routes: std::collections::HashSet<String> = new_sig.api_routes.iter()
            .map(|r| {
                let mut s = String::with_capacity(r.method.len() + r.path.len() + 1);
                s.push_str(&r.method);
                s.push(' ');
                s.push_str(&r.path);
                s
            })
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
}

/// The diff between two versions of a module's public interface
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

/// A change in an export's signature
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportChange {
    pub name: String,
    pub old_signature: String,
    pub new_signature: String,
}

impl SignatureDiff {
    /// Generate the agent notification message for Pre-Tool Gate injection
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

/// Extract exported function signatures from TypeScript source
fn extract_exported_functions(source: &str) -> HashMap<String, FunctionSignature> {
    let mut exports = HashMap::new();

    for line in source.lines() {
        let trimmed = line.trim();
        
        // Pattern: export [async] function name(...)
        if trimmed.starts_with("export") && trimmed.contains("function") {
            if let Some(sig) = parse_function_signature(trimmed) {
                exports.insert(sig.name.clone(), sig);
            }
        }

        // Pattern: export const name = [async] (...) =>
        if trimmed.starts_with("export const") || trimmed.starts_with("export let") {
            if trimmed.contains("=>") || trimmed.contains("function") {
                if let Some(sig) = parse_arrow_export_signature(trimmed) {
                    exports.insert(sig.name.clone(), sig);
                }
            }
        }
    }

    exports
}

fn parse_function_signature(line: &str) -> Option<FunctionSignature> {
    let is_async = line.contains("async ");
    
    let after_export = line.trim_start_matches("export ")
        .trim_start_matches("async ")
        .trim_start_matches("function ");

    let paren_pos = after_export.find('(')?;
    let name = after_export[..paren_pos].trim().to_string();
    if name.is_empty() { return None; }

    // Signature: everything up to the opening brace
    let sig_end = after_export.find('{').unwrap_or(after_export.len());
    let signature_text = format!("function {}(...)", &after_export[..sig_end.min(after_export.len())].trim());
    let signature_hash = xxh64(signature_text.as_bytes(), 0);

    Some(FunctionSignature {
        name,
        signature_text,
        signature_hash,
        is_async,
        is_exported: true,
    })
}

fn parse_arrow_export_signature(line: &str) -> Option<FunctionSignature> {
    let rest = line.trim_start_matches("export ")
        .trim_start_matches("const ")
        .trim_start_matches("let ");

    let eq_pos = rest.find('=')?;
    let name = rest[..eq_pos].trim().to_string();
    if name.is_empty() || name.contains(' ') { return None; }

    let after_eq = rest[eq_pos + 1..].trim();
    let arrow_pos = after_eq.find("=>")?;
    let signature_text = format!("{} = {}=>", name, &after_eq[..arrow_pos].trim());
    let signature_hash = xxh64(signature_text.as_bytes(), 0);
    let is_async = after_eq.trim_start().starts_with("async");

    Some(FunctionSignature {
        name,
        signature_text,
        signature_hash,
        is_async,
        is_exported: true,
    })
}

/// Extract API routes from Express/Fastify patterns
fn extract_api_routes(source: &str) -> Vec<RouteSignature> {
    let mut routes = vec![];
    
    for line in source.lines() {
        let trimmed = line.trim();
        // Pattern: router.METHOD('path', ...) or app.METHOD('path', ...)
        for method in &["get", "post", "put", "patch", "delete", "use"] {
            let patterns = [
                format!("router.{}(", method),
                format!("app.{}(", method),
            ];
            for pattern in &patterns {
                if trimmed.contains(pattern.as_str()) {
                    // Extract path
                    if let Some(start) = trimmed.find(pattern.as_str()) {
                        let after_paren = &trimmed[start + pattern.len()..];
                        let end = after_paren.find(',').or_else(|| after_paren.find(')'))
                            .unwrap_or(after_paren.len());
                        let path = after_paren[..end].trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                        if !path.is_empty() {
                            routes.push(RouteSignature {
                                method: method.to_uppercase(),
                                path,
                                handler_name: None,
                            });
                        }
                    }
                }
            }
        }
    }
    
    routes
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

    #[test]
    fn test_extract_function_signatures() {
        let sig = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        println!("Exports: {:?}", sig.exports.keys().collect::<Vec<_>>());
        assert!(sig.exports.contains_key("getUser"), "Should find getUser export");
    }

    #[test]
    fn test_detect_breaking_change() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2 = ModuleSignature::from_source("src/user.ts", V2_BREAKING);

        let diff = v1.diff(&v2);
        assert!(diff.is_some(), "Should detect diff between v1 and v2");
        let diff = diff.unwrap();
        assert!(diff.is_breaking, "Change should be breaking");
    }

    #[test]
    fn test_no_diff_on_identical() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        assert!(v1.diff(&v2).is_none(), "Identical sources should produce no diff");
    }

    #[test]
    fn test_agent_notification_format() {
        let v1 = ModuleSignature::from_source("src/user.ts", V1_SOURCE);
        let v2 = ModuleSignature::from_source("src/user.ts", V2_BREAKING);
        let diff = v1.diff(&v2).unwrap();
        let notification = diff.agent_notification();
        assert!(notification.contains("CONTRACT ALERT"), "Should contain alert header");
        assert!(notification.contains("BREAKING"), "Should mention breaking changes");
    }

    #[test]
    fn test_route_extraction() {
        let sig = ModuleSignature::from_source("src/routes.ts", V1_SOURCE);
        println!("Routes: {:?}", sig.api_routes);
        // Should find at least one route
        assert!(!sig.api_routes.is_empty(), "Should extract API routes");
    }
}
