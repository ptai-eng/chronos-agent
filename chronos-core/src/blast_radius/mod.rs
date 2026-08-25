//! # Visual Blast-Radius Heatmap
//!
//! Pillar 4: Automatic impact analysis and visual risk reporting.
//!
//! ## 30-Second Glance (Ratified Spec)
//! - Default output: Markdown table <= 5 lines with Xanh/Vàng/Đỏ badges
//! - Only generates full HTML dashboard when risk >= HIGH/CRITICAL
//!
//! ## Impact Score Formula
//! V_impact = w1 * CallDepth + w2 * TelemetryHotness + w3 * ContractSeverity

pub mod impact_matrix;
pub mod markdown_glance;
pub mod visualizer;

pub use impact_matrix::{BlastRadiusReport, ImpactAnalyzer, RiskZone};
