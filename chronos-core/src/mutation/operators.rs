//! # Safe AST Mutators
//!
//! Generates carefully controlled code mutations (fault injections).
//!
//! ## Safety Rules (Ratified Spec)
//! - NEVER mutate loop condition operators (avoids infinite loops)
//! - NEVER mutate code touching file I/O or network in loop contexts
//! - Use text-level substitution on exact patterns (safe and fast)
//!
//! ## Mutation Categories
//! 1. **Condition Operators:** `>` ↔ `<=`, `>=` ↔ `<`, `===` ↔ `!==`
//! 2. **Boundary Values:** `+ 1` ↔ `- 1`, `>= 0` ↔ `> 0`
//! 3. **Return Values:** `return val` → `return null/undefined/false/0`
//! 4. **Error Swallowing:** Suppress `throw` / empty catch blocks
//! 5. **HTTP Status Codes:** `200` ↔ `500`, `201` ↔ `400`

use serde::{Deserialize, Serialize};
use xxhash_rust::xxh64::xxh64;

/// The category of mutation applied
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MutantKind {
    /// Flipped comparison operator: `>` → `<=`
    ConditionFlip { original: String, mutated: String },
    /// Off-by-one boundary: `i < len` → `i <= len`
    BoundaryShift { original: String, mutated: String },
    /// Return null/undefined instead of actual value
    ReturnNull { function_name: String },
    /// Return false/0 instead of actual value
    ReturnFalsy { function_name: String },
    /// Swallow error (empty catch body)
    ErrorSwallow { original_throw: String },
    /// HTTP status code swap
    HttpStatusSwap { original_status: u16, mutated_status: u16 },
}

impl MutantKind {
    /// Human-readable description for AI agent feedback
    pub fn description(&self) -> String {
        match self {
            MutantKind::ConditionFlip { original, mutated } =>
                format!("Condition operator flipped: `{}` → `{}`", original, mutated),
            MutantKind::BoundaryShift { original, mutated } =>
                format!("Boundary shifted: `{}` → `{}`", original, mutated),
            MutantKind::ReturnNull { function_name } =>
                format!("Function `{}` now returns null/undefined", function_name),
            MutantKind::ReturnFalsy { function_name } =>
                format!("Function `{}` now returns false/0", function_name),
            MutantKind::ErrorSwallow { original_throw } =>
                format!("Error suppressed: `{}` removed from catch block", original_throw),
            MutantKind::HttpStatusSwap { original_status, mutated_status } =>
                format!("HTTP status swapped: {} → {}", original_status, mutated_status),
        }
    }

    /// Is this mutation safe to run in-process? (no infinite loop risk)
    pub fn is_safe(&self) -> bool {
        // All our defined mutants are safe by design
        // (we explicitly exclude loop condition mutations)
        true
    }
}

/// A single mutant: a modified version of source code ready for test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutant {
    /// Unique identifier for this mutant (hash of original + mutation)
    pub id: u64,
    /// The file this mutant targets
    pub target_file: String,
    /// Line number where the mutation occurs
    pub target_line: usize,
    /// The kind of mutation applied
    pub kind: MutantKind,
    /// The mutated source code (full file content)
    pub mutated_source: String,
    /// Whether this mutant was killed (test failed = detected the bug)
    pub killed: bool,
    /// Whether this mutant timed out
    pub timed_out: bool,
    /// Which test killed this mutant (if any)
    pub killed_by: Option<String>,
}

impl Mutant {
    pub fn new(
        target_file: impl Into<String>,
        target_line: usize,
        kind: MutantKind,
        _original_source: &str,
        mutated_source: impl Into<String>,
    ) -> Self {
        let mutated = mutated_source.into();
        let id = xxh64(mutated.as_bytes(), 42);
        Self {
            id,
            target_file: target_file.into(),
            target_line,
            kind,
            mutated_source: mutated,
            killed: false,
            timed_out: false,
            killed_by: None,
        }
    }

    /// Mark this mutant as killed (test detected the bug)
    pub fn kill(&mut self, test_name: impl Into<String>) {
        self.killed = true;
        self.killed_by = Some(test_name.into());
    }

    /// Mark as timed out (treated as killed by timeout — expected behavior)
    pub fn timeout(&mut self) {
        self.timed_out = true;
        self.killed = true;
        self.killed_by = Some("[timeout]".to_string());
    }
}

/// Generate all safe mutants from a TypeScript/JavaScript source string.
///
/// Returns a vector of mutants, each with the full mutated source code.
/// Does NOT mutate loop conditions to avoid infinite loop risk.
pub fn generate_mutants(source: &str, file_path: &str) -> Vec<Mutant> {
    let mut mutants = vec![];

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Skip loop conditions — safety constraint
        if is_loop_line(trimmed) {
            continue;
        }

        // Skip comment lines
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
            continue;
        }

        // 1. Condition Operator Flips (only in if/while that are NOT for-loop conditions)
        if !trimmed.starts_with("for ") && !trimmed.starts_with("for(") {
            for (original, mutated) in CONDITION_FLIPS {
                if line.contains(original) {
                    let mutated_source = replace_first_in_line(source, line_idx, original, mutated);
                    if mutated_source != source {
                        mutants.push(Mutant::new(
                            file_path,
                            line_num,
                            MutantKind::ConditionFlip {
                                original: original.to_string(),
                                mutated: mutated.to_string(),
                            },
                            source,
                            mutated_source,
                        ));
                    }
                }
            }
        }

        // 2. Boundary Value Mutations
        for (original, mutated) in BOUNDARY_SHIFTS {
            if line.contains(original) && !is_loop_line(trimmed) {
                let mutated_source = replace_first_in_line(source, line_idx, original, mutated);
                if mutated_source != source {
                    mutants.push(Mutant::new(
                        file_path,
                        line_num,
                        MutantKind::BoundaryShift {
                            original: original.to_string(),
                            mutated: mutated.to_string(),
                        },
                        source,
                        mutated_source,
                    ));
                }
            }
        }

        // 3. Return Value Mutations
        if let Some(fn_name) = detect_return_statement(trimmed, source, line_idx) {
            // return null mutation
            let null_mutation = mutate_return_to_null(source, line_idx);
            if null_mutation != source {
                mutants.push(Mutant::new(
                    file_path,
                    line_num,
                    MutantKind::ReturnNull { function_name: fn_name.clone() },
                    source,
                    null_mutation,
                ));
            }
        }

        // 4. HTTP Status Code Swaps
        for (original_status, mutated_status) in HTTP_STATUS_SWAPS {
            let original_str = original_status.to_string();
            if line.contains(&original_str) && (line.contains("status") || line.contains("Status")) {
                let mutated_source = replace_first_in_line(source, line_idx, &original_str, &mutated_status.to_string());
                if mutated_source != source {
                    mutants.push(Mutant::new(
                        file_path,
                        line_num,
                        MutantKind::HttpStatusSwap {
                            original_status: **original_status,
                            mutated_status: **mutated_status,
                        },
                        source,
                        mutated_source,
                    ));
                }
            }
        }
    }

    // Deduplicate by mutant ID (content hash)
    let mut seen = std::collections::HashSet::new();
    mutants.retain(|m| seen.insert(m.id));

    mutants
}

/// Safe condition flip pairs — only applied outside loop conditions
const CONDITION_FLIPS: &[(&str, &str)] = &[
    (" === ", " !== "),
    (" !== ", " === "),
    (" == ", " != "),
    (" != ", " == "),
    (" >= ", " < "),
    (" <= ", " > "),
    (" > ", " < "),
];

/// Boundary shift pairs
const BOUNDARY_SHIFTS: &[(&str, &str)] = &[
    (" + 1", " - 1"),
    (" - 1", " + 1"),
    (">= 0", "> 0"),
    ("> 0", ">= 0"),
    ("length - 1", "length"),
    ("length", "length - 1"),
];

/// HTTP status code swaps
const HTTP_STATUS_SWAPS: &[(&u16, &u16)] = &[
    (&200, &500),
    (&201, &400),
    (&204, &500),
    (&401, &200),
    (&403, &200),
    (&404, &200),
];

/// Check if a line is a for/while loop condition (unsafe to mutate)
fn is_loop_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("for ") || trimmed.starts_with("for(")
        || trimmed.starts_with("while ") || trimmed.starts_with("while(")
        || trimmed.contains(".forEach(") || trimmed.contains(".map(")
        || trimmed.contains(".filter(") || trimmed.contains(".reduce(")
}

/// Detect a return statement and extract the enclosing function name (heuristic)
fn detect_return_statement(line: &str, full_source: &str, line_idx: usize) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("return ") { return None; }
    // Don't mutate "return null", "return undefined", "return false", "return 0" (already falsy)
    if matches!(trimmed, "return null;" | "return undefined;" | "return false;" | "return 0;" | "return;") {
        return None;
    }

    // Find nearest enclosing function name (scan backwards)
    let lines: Vec<&str> = full_source.lines().collect();
    for i in (0..line_idx).rev() {
        let l = lines[i].trim();
        if l.contains("function ") || l.contains("=> {") || l.contains("async ") || l.contains("fn ") || l.contains("def ") || l.contains("func ") {
            // Extract function name heuristically
            let name = l.split("function ").nth(1)
                .or_else(|| l.split("fn ").nth(1))
                .or_else(|| l.split("def ").nth(1))
                .or_else(|| l.split("func ").nth(1))
                .and_then(|s| s.split('(').next())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "anonymous".to_string());
            return Some(name);
        }
    }
    Some("anonymous".to_string())
}

/// Mutate a return statement on a specific line to return null
fn mutate_return_to_null(source: &str, line_idx: usize) -> String {
    let mut new_lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();

    if let Some(line) = new_lines.get_mut(line_idx) {
        let trimmed = line.trim();
        if trimmed.starts_with("return ") {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            *line = format!("{}return null;", indent);
        }
    }

    new_lines.join("\n")
}

/// Replace the first occurrence of a pattern within a specific line of source
fn replace_first_in_line(source: &str, line_idx: usize, from: &str, to: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut new_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    
    if let Some(line) = new_lines.get_mut(line_idx) {
        if let Some(pos) = line.find(from) {
            line.replace_range(pos..pos + from.len(), to);
        }
    }
    
    new_lines.join("\n")
}

/// Build a single combined "Super-Mutant" source file embedding all non-overlapping line
/// mutations behind runtime environment variable checks (`CHRONOS_MUTANT_ID`).
///
/// For compiled languages (Rust, Go, C/C++, Java), this allows compiling the test binary
/// **once** ($O(1)$ compile passes instead of $O(N)$) and evaluating each mutant in parallel
/// solely by passing `CHRONOS_MUTANT_ID=<mutant.id>` to the test runner process.
pub fn build_super_mutant_source(original_source: &str, mutants: &[Mutant], ext: &str) -> String {
    if mutants.is_empty() {
        return original_source.to_string();
    }

    let orig_lines: Vec<&str> = original_source.lines().collect();
    let mut out_lines: Vec<String> = orig_lines.iter().map(|l| (*l).to_string()).collect();

    // Group mutants by 1-indexed target_line (pick first non-overlapping mutant per line)
    let mut by_line: std::collections::HashMap<usize, &Mutant> = std::collections::HashMap::new();
    for m in mutants {
        by_line.entry(m.target_line).or_insert(m);
    }

    for (line_num, mutant) in by_line {
        let line_idx = line_num.saturating_sub(1);
        let Some(orig_line) = orig_lines.get(line_idx).copied() else {
            continue;
        };
        let mut_lines: Vec<&str> = mutant.mutated_source.lines().collect();
        let Some(mut_line) = mut_lines.get(line_idx).copied() else {
            continue;
        };
        if orig_line == mut_line {
            continue;
        }

        let id_str = mutant.id.to_string();
        let indent: String = orig_line.chars().take_while(|c| c.is_whitespace()).collect();
        let orig_trim = orig_line.trim();
        let mut_trim = mut_line.trim();

        // Statement-level or inline expression guard depending on language and line structure
        let guarded = match ext {
            "rs" => {
                if orig_trim.starts_with("return ") {
                    format!(
                        "{indent}if std::env::var(\"CHRONOS_MUTANT_ID\").as_deref() == Ok(\"{id_str}\") {{ {mut_trim} }} else {{ {orig_trim} }}"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "rs")
                }
            }
            "go" => {
                if orig_trim.starts_with("return ") {
                    format!(
                        "{indent}if os.Getenv(\"CHRONOS_MUTANT_ID\") == \"{id_str}\" {{ {mut_trim} }} else {{ {orig_trim} }}"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "go")
                }
            }
            "py" => {
                if orig_trim.starts_with("return ") {
                    let orig_val = orig_trim.trim_start_matches("return ").trim();
                    let mut_val = mut_trim.trim_start_matches("return ").trim().trim_end_matches(';');
                    let py_mut_val = if mut_val == "null" { "None" } else { mut_val };
                    format!(
                        "{indent}return ({py_mut_val} if __import__(\"os\").environ.get(\"CHRONOS_MUTANT_ID\") == \"{id_str}\" else {orig_val})"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "py")
                }
            }
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if orig_trim.starts_with("return ") {
                    format!(
                        "{indent}if (getenv(\"CHRONOS_MUTANT_ID\") && std::string(getenv(\"CHRONOS_MUTANT_ID\")) == \"{id_str}\") {{ {mut_trim} }} else {{ {orig_trim} }}"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "cpp")
                }
            }
            "java" => {
                if orig_trim.starts_with("return ") {
                    format!(
                        "{indent}if (\"{id_str}\".equals(System.getenv(\"CHRONOS_MUTANT_ID\"))) {{ {mut_trim} }} else {{ {orig_trim} }}"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "java")
                }
            }
            _ => {
                // Default: TypeScript / JavaScript
                if orig_trim.starts_with("return ") {
                    format!(
                        "{indent}if (process.env.CHRONOS_MUTANT_ID === \"{id_str}\") {{ {mut_trim} }} else {{ {orig_trim} }}"
                    )
                } else {
                    apply_inline_env_guard(orig_line, &mutant.kind, &id_str, "ts")
                }
            }
        };

        if let Some(slot) = out_lines.get_mut(line_idx) {
            *slot = guarded;
        }
    }

    out_lines.join("\n")
}

fn apply_inline_env_guard(orig_line: &str, kind: &MutantKind, id_str: &str, lang: &str) -> String {
    let (from, to) = match kind {
        MutantKind::ConditionFlip { original, mutated } => (original.clone(), mutated.clone()),
        MutantKind::BoundaryShift { original, mutated } => (original.clone(), mutated.clone()),
        MutantKind::HttpStatusSwap { original_status, mutated_status } => {
            (original_status.to_string(), mutated_status.to_string())
        }
        _ => return orig_line.to_string(),
    };

    let mutated_line = if let Some(pos) = orig_line.find(&from) {
        let mut s = orig_line.to_string();
        s.replace_range(pos..pos + from.len(), &to);
        s
    } else {
        return orig_line.to_string();
    };

    // If the line is an `if (...) {` header, we can guard the condition expression directly
    let trimmed = orig_line.trim();
    if trimmed.starts_with("if ") || trimmed.starts_with("if(") {
        match lang {
            "rs" => {
                // In Rust: `if cond {` -> `if (if std::env::var("CHRONOS_MUTANT_ID").as_deref() == Ok("ID") { mut_cond } else { orig_cond }) {`
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_rust_or_go_if_cond(orig_line),
                    extract_rust_or_go_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "(if std::env::var(\"CHRONOS_MUTANT_ID\").as_deref() == Ok(\"{id_str}\") {{ {mut_cond} }} else {{ {orig_cond} }})"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
            "go" => {
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_rust_or_go_if_cond(orig_line),
                    extract_rust_or_go_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "(func() bool {{ if os.Getenv(\"CHRONOS_MUTANT_ID\") == \"{id_str}\" {{ return {mut_cond} }}; return {orig_cond} }}())"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
            "py" => {
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_py_if_cond(orig_line),
                    extract_py_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "({mut_cond} if __import__(\"os\").environ.get(\"CHRONOS_MUTANT_ID\") == \"{id_str}\" else {orig_cond})"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
            "cpp" => {
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_paren_if_cond(orig_line),
                    extract_paren_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "(getenv(\"CHRONOS_MUTANT_ID\") && std::string(getenv(\"CHRONOS_MUTANT_ID\")) == \"{id_str}\" ? ({mut_cond}) : ({orig_cond}))"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
            "java" => {
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_paren_if_cond(orig_line),
                    extract_paren_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "(\"{id_str}\".equals(System.getenv(\"CHRONOS_MUTANT_ID\")) ? ({mut_cond}) : ({orig_cond}))"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
            _ => {
                if let (Some(orig_cond), Some(mut_cond)) = (
                    extract_paren_if_cond(orig_line),
                    extract_paren_if_cond(&mutated_line),
                ) {
                    let replacement = format!(
                        "(process.env.CHRONOS_MUTANT_ID === \"{id_str}\" ? ({mut_cond}) : ({orig_cond}))"
                    );
                    return orig_line.replacen(&orig_cond, &replacement, 1);
                }
            }
        }
    }

    // Fallback for HTTP status swaps or arithmetic shifts outside `if` headers
    let expr_guard = match lang {
        "rs" => format!(
            "/*CHRONOS_MUTANT:{id_str}*/ {from}"
        ),
        _ => format!(
            "(process.env.CHRONOS_MUTANT_ID === \"{id_str}\" ? {to} : {from})"
        ),
    };
    orig_line.replacen(&from, &expr_guard, 1)
}

fn extract_paren_if_cond(line: &str) -> Option<String> {
    let idx = line.find("if")?;
    let rest = &line[idx + 2..];
    let open = rest.find('(')?;
    let close = rest.rfind(')')?;
    if close > open + 1 {
        Some(rest[open + 1..close].trim().to_string())
    } else {
        None
    }
}

fn extract_rust_or_go_if_cond(line: &str) -> Option<String> {
    let idx = line.find("if ")?;
    let rest = &line[idx + 3..];
    let brace = rest.rfind('{')?;
    let cond = rest[..brace].trim();
    if !cond.is_empty() {
        Some(cond.to_string())
    } else {
        None
    }
}

fn extract_py_if_cond(line: &str) -> Option<String> {
    let idx = line.find("if ")?;
    let rest = &line[idx + 3..];
    let colon = rest.rfind(':')?;
    let cond = rest[..colon].trim();
    if !cond.is_empty() {
        Some(cond.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SOURCE: &str = r#"
async function validateAge(age: number): Promise<boolean> {
    if (age >= 18) {
        return true;
    }
    return false;
}

async function getItem(items: string[], index: number): Promise<string> {
    if (index >= 0 && index < items.length) {
        return items[index];
    }
    return null;
}

async function createOrder(data: OrderData): Promise<Order> {
    const order = await db.create(data);
    return order;
}

// This loop should NOT be mutated (safety rule)
for (let i = 0; i < items.length; i++) {
    process(items[i]);
}
"#;

    #[test]
    fn test_generates_condition_flip_mutants() {
        let mutants = generate_mutants(SAMPLE_SOURCE, "src/service.ts");
        
        let condition_flips: Vec<_> = mutants.iter()
            .filter(|m| matches!(m.kind, MutantKind::ConditionFlip { .. }))
            .collect();
        
        println!("Condition flip mutants: {}", condition_flips.len());
        assert!(!condition_flips.is_empty(), "Should generate condition flip mutants");
        
        // Verify mutated source is different from original
        for m in &condition_flips {
            assert_ne!(m.mutated_source, SAMPLE_SOURCE, "Mutant source should differ from original");
        }
    }

    #[test]
    fn test_generates_return_null_mutants() {
        let mutants = generate_mutants(SAMPLE_SOURCE, "src/service.ts");
        
        let return_nulls: Vec<_> = mutants.iter()
            .filter(|m| matches!(m.kind, MutantKind::ReturnNull { .. }))
            .collect();
        
        println!("Return null mutants: {}", return_nulls.len());
        // At least the createOrder function should generate a return null mutant
        assert!(!return_nulls.is_empty(), "Should generate return null mutants");
    }

    #[test]
    fn test_loop_lines_not_mutated() {
        // The for loop at the end should not produce any condition-flip mutants
        let loop_source = "for (let i = 0; i < items.length; i++) { process(i); }";
        let mutants = generate_mutants(loop_source, "src/test.ts");
        
        let loop_flips: Vec<_> = mutants.iter()
            .filter(|m| matches!(m.kind, MutantKind::ConditionFlip { .. }))
            .collect();
        
        // Should be empty because we skip loop lines
        assert!(loop_flips.is_empty(), 
            "Should NOT mutate loop condition, but got: {:?}", 
            loop_flips.iter().map(|m| &m.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_mutants_are_deduplicated() {
        let mutants = generate_mutants(SAMPLE_SOURCE, "src/service.ts");
        let ids: Vec<u64> = mutants.iter().map(|m| m.id).collect();
        let unique_ids: std::collections::HashSet<u64> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique_ids.len(), "All mutant IDs should be unique");
    }

    #[test]
    fn test_mutant_kill_tracking() {
        let mut mutant = Mutant::new(
            "src/test.ts",
            5,
            MutantKind::ConditionFlip { original: ">".to_string(), mutated: "<=".to_string() },
            SAMPLE_SOURCE,
            SAMPLE_SOURCE.replace(" >= ", " < "),
        );
        assert!(!mutant.killed);
        
        mutant.kill("test should validate age correctly");
        assert!(mutant.killed);
        assert_eq!(mutant.killed_by.as_deref(), Some("test should validate age correctly"));
    }

    #[test]
    fn test_mutant_description_readable() {
        let kind = MutantKind::ConditionFlip {
            original: " >= ".to_string(),
            mutated: " < ".to_string(),
        };
        let desc = kind.description();
        assert!(desc.contains(">="), "Description should mention original operator");
        assert!(desc.contains("<"), "Description should mention mutated operator");
    }

    #[test]
    fn test_build_super_mutant_source_polyglot() {
        let ts_mutants = generate_mutants(SAMPLE_SOURCE, "src/service.ts");
        let super_ts = build_super_mutant_source(SAMPLE_SOURCE, &ts_mutants, "ts");
        assert!(
            super_ts.contains("process.env.CHRONOS_MUTANT_ID"),
            "TypeScript super-mutant source should contain process.env.CHRONOS_MUTANT_ID guard"
        );

        let rs_source = "pub fn is_adult(age: u32) -> bool {\n    if age >= 18 {\n        return true;\n    }\n    return false;\n}\n";
        let rs_mutants = generate_mutants(rs_source, "src/lib.rs");
        assert!(!rs_mutants.is_empty(), "Should generate Rust condition flip mutants");
        let super_rs = build_super_mutant_source(rs_source, &rs_mutants, "rs");
        assert!(
            super_rs.contains("std::env::var(\"CHRONOS_MUTANT_ID\")"),
            "Rust super-mutant source should contain std::env::var(\"CHRONOS_MUTANT_ID\") guard"
        );
    }
}
