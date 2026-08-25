//! # Impact Matrix — Blast-Radius Risk Engine
//!
//! Computes the impact score for a code change (git diff or node mutation).
//!
//! ## Risk Zones (3-tier)
//! - 🔴 Zone 1 CRITICAL: Direct callers + API routes with contract changes
//! - 🟡 Zone 2 HIGH: Transitive callers through hot-path functions  
//! - 🟠 Zone 3 MEDIUM: Performance-sensitive paths that may be affected

use crate::graph::schema::{ChronosGraph, ChronosNode, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Weight constants for impact score calculation
const W1_CALL_DEPTH: f32 = 0.4;
const W2_TELEMETRY_HOTNESS: f32 = 0.4;
const W3_CONTRACT_SEVERITY: f32 = 0.2;

/// Risk zone classification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskZone {
    /// 🟢 Safe — No blast radius detected
    Safe,
    /// 🟡 Medium — Some indirect impact possible
    Medium,
    /// 🟠 High — Hot path affected or many callers
    High,
    /// 🔴 Critical — Direct API contract change + hot path
    Critical,
}

impl RiskZone {
    pub fn emoji(&self) -> &'static str {
        match self {
            RiskZone::Safe => "🟢",
            RiskZone::Medium => "🟡",
            RiskZone::High => "🟠",
            RiskZone::Critical => "🔴",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RiskZone::Safe => "SAFE",
            RiskZone::Medium => "MEDIUM",
            RiskZone::High => "HIGH",
            RiskZone::Critical => "CRITICAL",
        }
    }

    pub fn from_score(score: f32) -> Self {
        if score >= 0.75 { RiskZone::Critical }
        else if score >= 0.50 { RiskZone::High }
        else if score >= 0.25 { RiskZone::Medium }
        else { RiskZone::Safe }
    }
}

/// A single affected node in the blast radius
#[derive(Debug, Serialize, Deserialize)]
pub struct AffectedNode {
    pub node_name: String,
    pub file: String,
    pub line: usize,
    pub risk_zone: RiskZone,
    pub reason: String,
    pub impact_score: f32,
}

/// The full blast-radius analysis report
#[derive(Debug, Serialize, Deserialize)]
pub struct BlastRadiusReport {
    /// The changed function(s) that triggered the analysis
    pub changed_nodes: Vec<String>,
    /// Overall risk zone for this change
    pub overall_risk: RiskZone,
    /// Overall impact score (0.0..=1.0)
    pub impact_score: f32,
    /// All affected nodes, organized by risk zone
    pub affected_nodes: Vec<AffectedNode>,
    /// API routes that will be impacted
    pub affected_routes: Vec<String>,
    /// Number of direct callers impacted
    pub direct_callers_count: usize,
    /// Number of transitive callers impacted
    pub transitive_callers_count: usize,
    /// Whether any hot-path nodes are in the blast radius
    pub hot_path_affected: bool,
    /// Whether any contract-breaking changes were detected
    pub contract_breaking: bool,
}

impl BlastRadiusReport {
    /// Whether this report warrants a full HTML dashboard
    pub fn needs_visual_dashboard(&self) -> bool {
        matches!(self.overall_risk, RiskZone::High | RiskZone::Critical)
    }
}

/// The impact analysis engine
pub struct ImpactAnalyzer<'g> {
    graph: &'g ChronosGraph,
}

impl<'g> ImpactAnalyzer<'g> {
    pub fn new(graph: &'g ChronosGraph) -> Self {
        Self { graph }
    }

    /// Analyze the blast radius of changing a specific node.
    pub fn analyze(&self, changed_node_id: &NodeId, contract_breaking: bool) -> BlastRadiusReport {
        let changed_node = match self.graph.get_node(changed_node_id) {
            Some(n) => n,
            None => return self.empty_report(changed_node_id, contract_breaking),
        };

        let mut affected_nodes: Vec<AffectedNode> = vec![];
        let mut affected_routes: Vec<String> = vec![];
        let mut hot_path_affected = false;

        // Zone 1: Direct callers (most immediate impact)
        let direct_callers = self.graph.callers_of(changed_node_id);
        let direct_count = direct_callers.len();

        for caller in &direct_callers {
            let is_hot = caller.telemetry.as_ref().map_or(false, |t| t.is_hot_path);
            if is_hot { hot_path_affected = true; }

            let zone = if is_hot || contract_breaking {
                RiskZone::Critical
            } else {
                RiskZone::High
            };

            affected_nodes.push(AffectedNode {
                node_name: caller.name.clone(),
                file: caller.location.file.to_string_lossy().to_string(),
                line: caller.location.start_line,
                risk_zone: zone,
                reason: if contract_breaking {
                    "Direct caller + contract breaking change".to_string()
                } else {
                    "Direct caller".to_string()
                },
                impact_score: if contract_breaking { 0.9 } else { 0.7 },
            });

            if matches!(caller.kind, NodeKind::ApiRoute) {
                affected_routes.push(caller.name.clone());
            }
        }

        // Zone 2: Transitive callers (indirect impact)
        let mut seen: HashSet<String> = direct_callers.iter()
            .map(|n| n.id.to_string())
            .collect();
        seen.insert(changed_node_id.to_string());

        let mut transitive_count = 0;
        for caller in &direct_callers {
            let transitive = self.graph.callers_of(&caller.id);
            for trans_caller in transitive {
                let id_str = trans_caller.id.to_string();
                if seen.contains(&id_str) { continue; }
                seen.insert(id_str);
                transitive_count += 1;

                let is_hot = trans_caller.telemetry.as_ref().map_or(false, |t| t.is_hot_path);
                if is_hot { hot_path_affected = true; }

                affected_nodes.push(AffectedNode {
                    node_name: trans_caller.name.clone(),
                    file: trans_caller.location.file.to_string_lossy().to_string(),
                    line: trans_caller.location.start_line,
                    risk_zone: if is_hot { RiskZone::High } else { RiskZone::Medium },
                    reason: "Transitive caller".to_string(),
                    impact_score: if is_hot { 0.55 } else { 0.35 },
                });
            }
        }

        // Zone 3: Performance risk from changed node itself
        let self_impact = compute_node_impact(changed_node, contract_breaking);

        // Overall impact score
        let impact_score = compute_overall_impact(
            direct_count,
            transitive_count,
            hot_path_affected,
            contract_breaking,
            self_impact,
        );

        let overall_risk = RiskZone::from_score(impact_score);

        BlastRadiusReport {
            changed_nodes: vec![changed_node.name.clone()],
            overall_risk,
            impact_score,
            affected_nodes,
            affected_routes,
            direct_callers_count: direct_count,
            transitive_callers_count: transitive_count,
            hot_path_affected,
            contract_breaking,
        }
    }

    /// Analyze blast radius of multiple changed nodes (git diff scenario)
    pub fn analyze_diff(&self, changed_ids: &[&NodeId]) -> BlastRadiusReport {
        if changed_ids.is_empty() {
            return BlastRadiusReport {
                changed_nodes: vec![],
                overall_risk: RiskZone::Safe,
                impact_score: 0.0,
                affected_nodes: vec![],
                affected_routes: vec![],
                direct_callers_count: 0,
                transitive_callers_count: 0,
                hot_path_affected: false,
                contract_breaking: false,
            };
        }

        // Merge all individual reports
        let reports: Vec<BlastRadiusReport> = changed_ids.iter()
            .map(|id| self.analyze(id, false))
            .collect();

        // Compound risk: N simultaneous changes raise risk beyond any individual score.
        // Formula: base = max individual score; compound = base + sum_of_rest * 0.15 (capped at 1.0)
        // This prevents "10 medium-risk changes = still medium" false comfort.
        let mut scores: Vec<f32> = reports.iter().map(|r| r.impact_score).collect();
        scores.sort_by(|a, b| a.total_cmp(b));
        
        let max_score = scores.pop().unwrap_or(0.0);
        let compound_bonus: f32 = scores.iter()
            .map(|&s| s * 0.15)
            .sum::<f32>()
            .min(0.40); // cap bonus at +40 pp so single-file changes aren't punished
        let compound_score = (max_score + compound_bonus).min(1.0);
        let total_direct: usize = reports.iter().map(|r| r.direct_callers_count).sum();
        let total_transitive: usize = reports.iter().map(|r| r.transitive_callers_count).sum();
        let any_hot_path = reports.iter().any(|r| r.hot_path_affected);
        let any_contract = reports.iter().any(|r| r.contract_breaking);
        let changed_names: Vec<String> = reports.iter()
            .flat_map(|r| r.changed_nodes.iter().cloned())
            .collect();

        let mut all_nodes: Vec<AffectedNode> = reports.into_iter()
            .flat_map(|r| r.affected_nodes)
            .collect();
        all_nodes.sort_by(|a, b| b.impact_score.partial_cmp(&a.impact_score).unwrap());

        BlastRadiusReport {
            changed_nodes: changed_names,
            overall_risk: RiskZone::from_score(compound_score),
            impact_score: compound_score,
            affected_nodes: all_nodes,
            affected_routes: vec![], // Simplified
            direct_callers_count: total_direct,
            transitive_callers_count: total_transitive,
            hot_path_affected: any_hot_path,
            contract_breaking: any_contract,
        }
    }

    fn empty_report(&self, id: &NodeId, contract_breaking: bool) -> BlastRadiusReport {
        BlastRadiusReport {
            changed_nodes: vec![id.to_string()],
            overall_risk: if contract_breaking { RiskZone::Medium } else { RiskZone::Safe },
            impact_score: if contract_breaking { 0.3 } else { 0.0 },
            affected_nodes: vec![],
            affected_routes: vec![],
            direct_callers_count: 0,
            transitive_callers_count: 0,
            hot_path_affected: false,
            contract_breaking,
        }
    }
}

/// Compute impact contribution from the changed node itself
fn compute_node_impact(node: &ChronosNode, contract_breaking: bool) -> f32 {
    let telemetry_hotness = if let Some(t) = &node.telemetry {
        if t.is_bottleneck { 1.0 }
        else if t.is_hot_path { 0.6 }
        else { t.cpu_percent_self / 100.0 }
    } else {
        node.heuristics.cognitive_load_score / 100.0 * 0.5
    };

    let contract_score = if contract_breaking { 1.0 } else { 0.0 };
    let call_depth_score = (node.heuristics.fan_in as f32 / 20.0).min(1.0);

    W1_CALL_DEPTH * call_depth_score
        + W2_TELEMETRY_HOTNESS * telemetry_hotness
        + W3_CONTRACT_SEVERITY * contract_score
}

/// Compute the overall impact score for the blast radius
fn compute_overall_impact(
    direct_callers: usize,
    transitive_callers: usize,
    hot_path_affected: bool,
    contract_breaking: bool,
    self_impact: f32,
) -> f32 {
    // 1 direct caller → 0.25 (Medium), 2+ → approaching High
    let caller_score = ((direct_callers as f32 * 0.25) + (transitive_callers as f32 * 0.05)).min(0.5);
    let hot_path_bonus = if hot_path_affected { 0.25 } else { 0.0 };
    let contract_bonus = if contract_breaking { 0.25 } else { 0.0 };

    (self_impact + caller_score + hot_path_bonus + contract_bonus).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::*;
    use std::path::PathBuf;

    fn make_simple_graph() -> (ChronosGraph, NodeId, NodeId) {
        let mut graph = ChronosGraph::new();

        let fn_id = NodeId::new("src/user.ts", "getUser", &NodeKind::Function);
        let fn_node = ChronosNode::new(
            fn_id.clone(),
            NodeKind::Function,
            "getUser",
            "UserService::getUser",
            SourceLocation { file: PathBuf::from("src/user.ts"), start_line: 5, start_col: 0, end_line: 15, end_col: 1 },
        );
        graph.add_node(fn_node);

        let caller_id = NodeId::new("src/api.ts", "handleRequest", &NodeKind::Function);
        let caller_node = ChronosNode::new(
            caller_id.clone(),
            NodeKind::Function,
            "handleRequest",
            "api::handleRequest",
            SourceLocation { file: PathBuf::from("src/api.ts"), start_line: 20, start_col: 0, end_line: 30, end_col: 1 },
        );
        graph.add_node(caller_node);

        graph.add_edge(&caller_id, &fn_id, ChronosEdge::new(EdgeKind::Calls)).ok();

        (graph, fn_id, caller_id)
    }

    #[test]
    fn test_blast_radius_with_callers() {
        let (graph, fn_id, _) = make_simple_graph();
        let analyzer = ImpactAnalyzer::new(&graph);
        let report = analyzer.analyze(&fn_id, false);

        assert_eq!(report.direct_callers_count, 1);
        assert!(!report.affected_nodes.is_empty());
        assert!(matches!(report.overall_risk, RiskZone::High | RiskZone::Medium | RiskZone::Critical));
    }

    #[test]
    fn test_contract_breaking_raises_risk() {
        let (graph, fn_id, _) = make_simple_graph();
        let analyzer = ImpactAnalyzer::new(&graph);
        let report_no_contract = analyzer.analyze(&fn_id, false);
        let report_with_contract = analyzer.analyze(&fn_id, true);

        assert!(
            report_with_contract.impact_score >= report_no_contract.impact_score,
            "Contract breaking should raise impact score"
        );
    }

    #[test]
    fn test_safe_report_for_isolated_function() {
        let mut graph = ChronosGraph::new();
        let fn_id = NodeId::new("src/isolated.ts", "unusedHelper", &NodeKind::Function);
        let fn_node = ChronosNode::new(
            fn_id.clone(),
            NodeKind::Function,
            "unusedHelper",
            "isolated::unusedHelper",
            SourceLocation { file: PathBuf::from("src/isolated.ts"), start_line: 1, start_col: 0, end_line: 5, end_col: 1 },
        );
        graph.add_node(fn_node);

        let analyzer = ImpactAnalyzer::new(&graph);
        let report = analyzer.analyze(&fn_id, false);

        assert_eq!(report.direct_callers_count, 0);
        assert!(matches!(report.overall_risk, RiskZone::Safe | RiskZone::Medium));
    }

    #[test]
    fn test_risk_zone_from_score() {
        assert_eq!(RiskZone::from_score(0.9), RiskZone::Critical);
        assert_eq!(RiskZone::from_score(0.6), RiskZone::High);
        assert_eq!(RiskZone::from_score(0.3), RiskZone::Medium);
        assert_eq!(RiskZone::from_score(0.1), RiskZone::Safe);
    }
}
