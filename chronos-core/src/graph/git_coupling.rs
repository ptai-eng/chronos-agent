//! # 5th Dimension: Git Temporal Co-Change Coupling (`EdgeKind::CoChangesWith`)
//!
//! Mines repository commit history (`git log --name-only`) to discover files that
//! historically change together (e.g., `.sql` migrations + ORM models, config schemas +
//! parsers, frontend types + backend DTOs) even when zero static AST call/import edges exist.
//!
//! Computes conditional probability:
//! $$P(B \mid A) = \frac{|C(A) \cap C(B)|}{|C(A)|}$$
//! where $|C(A) \cap C(B)| \ge 2$ and $P(B \mid A) \ge 0.40$ (stored as integer percentage `call_count = 40..100`).

use crate::graph::schema::{ChronosEdge, ChronosGraph, EdgeKind, NodeId, NodeKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

/// A discovered evolutionary coupling pair between two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoChangePair {
    pub file_a: String,
    pub file_b: String,
    pub shared_commits: usize,
    pub confidence_pct: usize,
}

/// Parse raw `git log --name-only --pretty=format:COMMIT:%H` output into commit file sets.
/// Skips mega-commits touching more than `max_files_per_commit` files (e.g. formatting sweeps).
pub fn parse_git_log_co_changes(
    raw_log: &str,
    min_shared_commits: usize,
    min_confidence_pct: usize,
    max_files_per_commit: usize,
) -> Vec<CoChangePair> {
    let mut commits: Vec<Vec<String>> = Vec::new();
    let mut current_files: HashSet<String> = HashSet::new();

    for line in raw_log.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("COMMIT:") {
            if !current_files.is_empty() {
                if current_files.len() <= max_files_per_commit {
                    let mut sorted: Vec<String> = current_files.drain().collect();
                    sorted.sort();
                    commits.push(sorted);
                } else {
                    current_files.clear();
                }
            }
        } else {
            let normalized = trimmed.replace('\\', "/");
            current_files.insert(normalized);
        }
    }
    if !current_files.is_empty() && current_files.len() <= max_files_per_commit {
        let mut sorted: Vec<String> = current_files.drain().collect();
        sorted.sort();
        commits.push(sorted);
    }

    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();

    for files in &commits {
        for f in files {
            *file_counts.entry(f.clone()).or_insert(0) += 1;
        }
        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                let key = (files[i].clone(), files[j].clone());
                *pair_counts.entry(key).or_insert(0) += 1;
            }
        }
    }

    let mut results = Vec::new();
    for ((file_a, file_b), shared) in pair_counts {
        if shared < min_shared_commits {
            continue;
        }
        let count_a = file_counts.get(&file_a).copied().unwrap_or(1).max(1);
        let count_b = file_counts.get(&file_b).copied().unwrap_or(1).max(1);

        let conf_ab = (shared * 100) / count_a;
        if conf_ab >= min_confidence_pct {
            results.push(CoChangePair {
                file_a: file_a.clone(),
                file_b: file_b.clone(),
                shared_commits: shared,
                confidence_pct: conf_ab.min(100),
            });
        }

        let conf_ba = (shared * 100) / count_b;
        if conf_ba >= min_confidence_pct {
            results.push(CoChangePair {
                file_a: file_b,
                file_b: file_a,
                shared_commits: shared,
                confidence_pct: conf_ba.min(100),
            });
        }
    }

    results.sort_by(|a, b| b.confidence_pct.cmp(&a.confidence_pct).then_with(|| a.file_a.cmp(&b.file_a)));
    results
}

/// Run `git log` in `workspace_root`, compute evolutionary co-change pairs, and inject
/// `EdgeKind::CoChangesWith` edges into `ChronosGraph`.
pub fn ingest_git_co_changes(
    graph: &mut ChronosGraph,
    workspace_root: &Path,
    max_commits: usize,
) -> usize {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("log")
        .arg(format!("-n{}", max_commits.max(10)))
        .arg("--name-only")
        .arg("--pretty=format:COMMIT:%H")
        .output();

    let raw_log = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return 0,
    };

    let pairs = parse_git_log_co_changes(&raw_log, 2, 40, 25);
    if pairs.is_empty() {
        return 0;
    }

    // Build suffix lookup of File nodes in the graph so relative git paths match absolute graph paths
    let file_nodes: Vec<(NodeId, String)> = graph
        .all_nodes()
        .into_iter()
        .filter(|n| n.kind == NodeKind::File)
        .map(|n| {
            let norm = n.location.file.to_string_lossy().replace('\\', "/");
            (n.id.clone(), norm)
        })
        .collect();

    let resolve_file_node = |rel_path: &str| -> Option<NodeId> {
        let clean = rel_path.trim_start_matches("./").replace('\\', "/");
        file_nodes
            .iter()
            .find(|(_, full)| full == &clean || full.ends_with(&format!("/{}", clean)))
            .map(|(id, _)| id.clone())
    };

    let mut edges_added = 0usize;
    for pair in pairs {
        if let (Some(id_a), Some(id_b)) = (resolve_file_node(&pair.file_a), resolve_file_node(&pair.file_b)) {
            if id_a != id_b {
                let mut edge = ChronosEdge::new(EdgeKind::CoChangesWith);
                edge.call_site_line = Some(pair.confidence_pct);
                if graph.add_edge(&id_a, &id_b, edge).is_ok() {
                    edges_added += 1;
                }
            }
        }
    }

    edges_added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_log_co_changes_computes_confidence() {
        let log = "\
COMMIT:111111
src/schema.sql
src/models/user.rs
COMMIT:222222
src/schema.sql
src/models/user.rs
src/api/routes.rs
COMMIT:333333
src/schema.sql
src/models/user.rs
COMMIT:444444
src/unrelated.rs
";
        let pairs = parse_git_log_co_changes(log, 2, 50, 25);
        assert!(!pairs.is_empty(), "Should detect co-change coupling between schema.sql and user.rs");
        let schema_to_user = pairs
            .iter()
            .find(|p| p.file_a == "src/schema.sql" && p.file_b == "src/models/user.rs")
            .expect("Expected src/schema.sql -> src/models/user.rs coupling");
        assert_eq!(schema_to_user.shared_commits, 3);
        assert_eq!(schema_to_user.confidence_pct, 100);
    }
}
