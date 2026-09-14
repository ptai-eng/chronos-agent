//! # Telemetry Adapter — Trait and Data Types
//!
//! Defines the pluggable telemetry backend trait.
//! Any backend (V8 CDP, eBPF, OpenTelemetry, py-spy) implements this trait.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Raw telemetry data for a single function gathered during one profiling session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryData {
    /// Fully-qualified function name or identifier
    pub function_name: String,
    /// All measured execution times in milliseconds (one per invocation sampled)
    pub execution_times_ms: Vec<f64>,
    /// Net memory allocation in KB during this function's calls
    pub alloc_delta_kb: f64,
    /// Total number of times this function was invoked in the session
    pub invocation_count: u64,
    /// CPU time in microseconds attributed to this function (self time only)
    pub cpu_self_us: u64,
    /// Total CPU time of the profiling session in microseconds
    pub session_cpu_total_us: u64,
    /// Source file path if available
    pub source_url: Option<String>,
    /// Line number of function start
    pub line_number: Option<usize>,
}

impl TelemetryData {
    /// Compute p50 (median) execution time
    pub fn p50_ms(&self) -> f64 {
        if self.execution_times_ms.is_empty() { return 0.0; }
        let mut sorted = self.execution_times_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        sorted[mid]
    }

    /// Compute p95 execution time
    pub fn p95_ms(&self) -> f64 {
        if self.execution_times_ms.is_empty() { return 0.0; }
        let mut sorted = self.execution_times_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = (sorted.len() as f64 * 0.95) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Compute p99 execution time
    pub fn p99_ms(&self) -> f64 {
        if self.execution_times_ms.is_empty() { return 0.0; }
        let mut sorted = self.execution_times_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Percentage of session CPU time for this function
    pub fn cpu_percent_self(&self) -> f32 {
        if self.session_cpu_total_us == 0 { return 0.0; }
        (self.cpu_self_us as f32 / self.session_cpu_total_us as f32) * 100.0
    }

    /// Is this function on a hot path?
    /// Heuristic: invoked frequently AND takes a notable chunk of CPU
    pub fn is_hot_path(&self) -> bool {
        self.invocation_count >= 10 && self.cpu_percent_self() >= 5.0
    }

    /// Is this function a bottleneck?
    /// Heuristic: hot path AND high p99 latency
    pub fn is_bottleneck(&self) -> bool {
        self.is_hot_path() && self.p99_ms() >= 100.0
    }

    /// Generate a bottleneck reason description if applicable
    pub fn bottleneck_reason(&self) -> Option<String> {
        if !self.is_bottleneck() { return None; }
        
        let mut reasons = vec![];
        let p99 = self.p99_ms();
        let cpu_percent = self.cpu_percent_self();
        
        if p99 >= 1000.0 {
            reasons.push(format!("Very high p99 latency ({:.0}ms)", p99));
        } else if p99 >= 100.0 {
            reasons.push(format!("High p99 latency ({:.0}ms)", p99));
        }
        
        if cpu_percent >= 30.0 {
            reasons.push(format!("Consumes {:.1}% of total CPU time", cpu_percent));
        }
        
        if self.invocation_count > 1000 {
            reasons.push(format!("Called {} times in session — check for N+1 pattern", self.invocation_count));
        }
        
        if reasons.is_empty() { return None; }
        Some(reasons.join("; "))
    }
}

/// A complete telemetry session's data: all functions measured together
#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetrySession {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub backend: String,
    pub functions: Vec<TelemetryData>,
    /// Total test run duration in ms
    pub total_duration_ms: f64,
}

impl TelemetrySession {
    pub fn new(backend: impl Into<String>) -> Self {
        Self {
            started_at: Utc::now(),
            ended_at: Utc::now(),
            backend: backend.into(),
            functions: vec![],
            total_duration_ms: 0.0,
        }
    }

    /// Find telemetry for a specific function by name (exact or partial match)
    pub fn find_function(&self, name: &str) -> Option<&TelemetryData> {
        self.functions.iter().find(|f| {
            f.function_name == name
                || f.function_name.ends_with(&format!("::{}", name))
                || f.function_name.contains(name)
        })
    }

    /// Get all bottleneck functions, sorted by CPU usage descending
    pub fn bottlenecks(&self) -> Vec<&TelemetryData> {
        let mut bns: Vec<&TelemetryData> = self.functions.iter()
            .filter(|f| f.is_bottleneck())
            .collect();
        bns.sort_by(|a, b| b.cpu_percent_self().total_cmp(&a.cpu_percent_self()));
        bns
    }

    /// Quick summary for CLI display
    pub fn summary(&self) -> String {
        let duration_s = (self.ended_at - self.started_at).num_milliseconds() as f64 / 1000.0;
        let bottlenecks = self.bottlenecks().len();
        let hot_paths = self.functions.iter().filter(|f| f.is_hot_path()).count();
        
        format!(
            "📡 Telemetry Session ({})\n\
             ⏱️  Duration: {:.2}s\n\
             📊 Functions profiled: {}\n\
             🔥 Hot paths: {} | ⚠️  Bottlenecks: {}",
            self.backend, duration_s,
            self.functions.len(), hot_paths, bottlenecks
        )
    }
}

/// Trait for telemetry backends.
/// Implement this to add new profiling backends (eBPF, py-spy, etc.)
#[async_trait::async_trait]
pub trait TelemetryAdapter: Send + Sync {
    /// Name of this backend (e.g., "cdp-v8", "ebpf-uprobe", "py-spy")
    fn backend_name(&self) -> &'static str;

    /// Check if this backend is available in the current environment
    async fn is_available(&self) -> bool;

    /// Start profiling and run the provided command/test suite.
    /// Returns a session containing all profiling data.
    async fn profile_command(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &std::path::Path,
    ) -> Result<TelemetrySession>;
}

#[cfg(target_os = "linux")]
use crate::telemetry::ebpf_adapter::linux::EbpfAdapter;

#[cfg(not(target_os = "linux"))]
use crate::telemetry::ebpf_adapter::linux::EbpfAdapter;

use crate::telemetry::cdp_client::CdpV8Adapter;

/// Runtime Profiler Selection — auto-detect platform
pub enum RuntimeAdapter {
    Cdp(CdpV8Adapter),
    Ebpf(EbpfAdapter),
}

impl RuntimeAdapter {
    pub async fn best_available(#[cfg_attr(not(target_os = "linux"), allow(unused_variables))] pid: u32) -> Self {
        #[cfg(target_os = "linux")]
        {
            if let Ok(ebpf) = EbpfAdapter::attach(pid) {
                return Self::Ebpf(ebpf);
            }
        }
        // Fallback to CDP
        Self::Cdp(CdpV8Adapter::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_telemetry(name: &str, times: Vec<f64>, cpu_us: u64, total_cpu: u64, count: u64) -> TelemetryData {
        TelemetryData {
            function_name: name.to_string(),
            execution_times_ms: times,
            alloc_delta_kb: 0.0,
            invocation_count: count,
            cpu_self_us: cpu_us,
            session_cpu_total_us: total_cpu,
            source_url: None,
            line_number: None,
        }
    }

    #[test]
    fn test_percentile_calculation() {
        let data = make_telemetry("test", vec![10.0, 20.0, 30.0, 40.0, 50.0, 100.0, 200.0, 500.0, 900.0, 1000.0], 0, 1000, 10);
        
        let p50 = data.p50_ms();
        let p99 = data.p99_ms();
        
        assert!(p50 > 0.0, "P50 should be positive");
        assert!(p99 >= p50, "P99 should be >= P50");
        assert!(p99 >= 900.0, "P99 should be at the high end");
    }

    #[test]
    fn test_hot_path_detection() {
        // Hot: many invocations + significant CPU
        let hot = make_telemetry("hotFunc", vec![50.0; 100], 50_000, 100_000, 100);
        assert!(hot.is_hot_path(), "Should be hot path");

        // Cold: few invocations
        let cold = make_telemetry("coldFunc", vec![5.0], 100, 100_000, 1);
        assert!(!cold.is_hot_path(), "Should NOT be hot path");
    }

    #[test]
    fn test_bottleneck_detection() {
        // Bottleneck: hot + high p99
        let bottleneck = make_telemetry(
            "bottleneckFunc",
            vec![50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0],
            60_000, 100_000, 50
        );
        assert!(bottleneck.is_bottleneck(), "Should be bottleneck");
        assert!(bottleneck.bottleneck_reason().is_some(), "Should have bottleneck reason");
    }

    #[test]
    fn test_cpu_percent() {
        let data = make_telemetry("func", vec![], 25_000, 100_000, 10);
        assert!((data.cpu_percent_self() - 25.0).abs() < 0.01, "CPU% should be 25%");
    }

    #[test]
    fn test_session_find_function() {
        let mut session = TelemetrySession::new("test");
        session.functions.push(make_telemetry("module::UserService::getUser", vec![10.0], 1000, 10000, 10));
        
        assert!(session.find_function("getUser").is_some());
        assert!(session.find_function("nonExistent").is_none());
    }
}
