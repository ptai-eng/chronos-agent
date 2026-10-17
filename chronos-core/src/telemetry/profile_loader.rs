//! # Universal Offline Telemetry Profile Loader (`chronos hydrate`)
//!
//! Hydrates the 4th Dimension (runtime CPU/latency telemetry) of the `ChronosGraph`
//! from standard profiler artifacts on Windows, macOS, and Linux — without requiring
//! live root `eBPF` probes or interactive `node --inspect` sessions.
//!
//! ## Supported Formats (Auto-Detected)
//! 1. **Chrome / V8 `.cpuprofile` JSON** (`node --cpu-prof`, Deno, Chrome DevTools)
//! 2. **Speedscope / `py-spy` JSON** (`py-spy record -f speedscope`, `rbspy`, `speedscope.app`)
//! 3. **Folded Stack Flamegraph Text** (`cargo flamegraph`, `perf`, `go tool pprof`, `async-profiler`)
//! 4. **OpenTelemetry (OTLP) Trace JSON** (`resourceSpans -> scopeSpans -> spans`)

use crate::telemetry::adapter::{TelemetryData, TelemetrySession};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Load a telemetry profile from a file path, auto-detecting the format.
pub fn load_profile_from_file(path: &Path) -> Result<TelemetrySession> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read profile file {}: {}", path.display(), e))?;
    parse_profile_str(&raw)
}

/// Auto-detect and parse a telemetry profile string into a `TelemetrySession`.
pub fn parse_profile_str(raw: &str) -> Result<TelemetrySession> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Telemetry profile input is empty"));
    }

    if trimmed.starts_with('{') {
        let json: Value = serde_json::from_str(trimmed)
            .map_err(|e| anyhow!("Invalid JSON telemetry profile: {}", e))?;

        // 1. Speedscope JSON (`"$schema": "...speedscope..."` or `"shared": { "frames": ... }`)
        if json.get("shared").and_then(|s| s.get("frames")).is_some()
            || json.get("profiles").is_some()
        {
            return parse_speedscope_json(&json);
        }

        // 2. OpenTelemetry (OTLP) Trace JSON (`"resourceSpans": [...]`)
        if json.get("resourceSpans").is_some() || json.get("spans").is_some() {
            return parse_otlp_trace_json(&json);
        }

        // 3. Chrome / V8 `.cpuprofile` JSON (`"nodes": [...]`)
        if json.get("nodes").is_some() {
            return parse_v8_cpuprofile_json(&json);
        }

        return Err(anyhow!(
            "Unrecognized JSON profile schema (expected V8 .cpuprofile, Speedscope, or OTLP resourceSpans)"
        ));
    }

    // 4. Folded Stack Flamegraph text (`frame1;frame2;leaf_fn 123`)
    parse_folded_stacks(trimmed)
}

/// Parse Chrome / Node.js V8 `.cpuprofile` JSON.
fn parse_v8_cpuprofile_json(json: &Value) -> Result<TelemetrySession> {
    let nodes = json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("Missing `nodes` array in .cpuprofile"))?;

    // Map node_id -> (fn_name, url, line, hit_count)
    let mut node_meta: HashMap<i64, (String, Option<String>, Option<usize>, u64)> = HashMap::new();
    for node in nodes {
        let id = node.get("id").and_then(|v| v.as_i64()).unwrap_or(-1);
        let hit_count = node.get("hitCount").and_then(|v| v.as_u64()).unwrap_or(0);
        if let Some(cf) = node.get("callFrame") {
            let fn_name = cf
                .get("functionName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let url = cf
                .get("url")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let line = cf
                .get("lineNumber")
                .and_then(|v| v.as_i64())
                .filter(|&l| l >= 0)
                .map(|l| (l as usize) + 1);
            if id >= 0 && !fn_name.is_empty() && fn_name != "(root)" && fn_name != "(idle)" && fn_name != "(program)" {
                node_meta.insert(id, (fn_name, url, line, hit_count));
            }
        }
    }

    // Accumulate per-node time from `samples` + `timeDeltas` if present, else fallback to `hitCount * 1000us`
    let mut node_time_us: HashMap<i64, u64> = HashMap::new();
    let mut node_samples: HashMap<i64, Vec<f64>> = HashMap::new();

    if let (Some(samples), Some(deltas)) = (
        json.get("samples").and_then(|v| v.as_array()),
        json.get("timeDeltas").and_then(|v| v.as_array()),
    ) {
        for (sample_val, delta_val) in samples.iter().zip(deltas.iter()) {
            if let (Some(id), Some(delta_us)) = (sample_val.as_i64(), delta_val.as_i64()) {
                let d_us = delta_us.max(1) as u64;
                *node_time_us.entry(id).or_insert(0) += d_us;
                node_samples
                    .entry(id)
                    .or_default()
                    .push(d_us as f64 / 1000.0);
            }
        }
    }

    let mut by_fn: HashMap<String, TelemetryData> = HashMap::new();
    let mut total_cpu_us: u64 = 0;

    for (id, (fn_name, url, line, hit_count)) in node_meta {
        let sampled_us = node_time_us.get(&id).copied().unwrap_or(hit_count * 1000);
        let sample_vec = node_samples.remove(&id).unwrap_or_else(|| {
            if hit_count > 0 {
                vec![sampled_us as f64 / 1000.0; hit_count.min(50) as usize]
            } else {
                vec![]
            }
        });
        let invocations = hit_count.max(sample_vec.len() as u64);
        if invocations == 0 && sampled_us == 0 {
            continue;
        }
        total_cpu_us += sampled_us;

        let entry = by_fn.entry(fn_name.clone()).or_insert_with(|| TelemetryData {
            function_name: fn_name,
            execution_times_ms: Vec::new(),
            alloc_delta_kb: 0.0,
            invocation_count: 0,
            cpu_self_us: 0,
            session_cpu_total_us: 0,
            source_url: url,
            line_number: line,
        });
        entry.invocation_count += invocations;
        entry.cpu_self_us += sampled_us;
        entry.execution_times_ms.extend(sample_vec);
    }

    let total_cpu_us = total_cpu_us.max(1);
    let mut functions: Vec<TelemetryData> = by_fn
        .into_values()
        .map(|mut d| {
            d.session_cpu_total_us = total_cpu_us;
            d
        })
        .collect();
    functions.sort_by(|a, b| b.cpu_self_us.cmp(&a.cpu_self_us));

    let mut session = TelemetrySession::new("v8-cpuprofile-offline");
    session.total_duration_ms = total_cpu_us as f64 / 1000.0;
    session.functions = functions;
    Ok(session)
}

/// Parse Speedscope / `py-spy` JSON profile.
fn parse_speedscope_json(json: &Value) -> Result<TelemetrySession> {
    let frames = json
        .get("shared")
        .and_then(|s| s.get("frames"))
        .and_then(|f| f.as_array())
        .ok_or_else(|| anyhow!("Missing `shared.frames` in Speedscope JSON"))?;

    let mut frame_meta: Vec<(String, Option<String>, Option<usize>)> = Vec::with_capacity(frames.len());
    for frame in frames {
        let name = frame
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let file = frame
            .get("file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let line = frame
            .get("line")
            .and_then(|v| v.as_u64())
            .map(|l| l as usize);
        frame_meta.push((name, file, line));
    }

    let mut by_fn: HashMap<String, TelemetryData> = HashMap::new();
    let mut total_cpu_us: u64 = 0;

    if let Some(profiles) = json.get("profiles").and_then(|p| p.as_array()) {
        for profile in profiles {
            if let Some(samples) = profile.get("samples").and_then(|s| s.as_array()) {
                let weights = profile.get("weights").and_then(|w| w.as_array());
                for (idx, stack_val) in samples.iter().enumerate() {
                    let Some(stack) = stack_val.as_array() else {
                        continue;
                    };
                    let Some(leaf_idx) = stack.last().and_then(|v| v.as_u64()) else {
                        continue;
                    };
                    let Some((fn_name, file, line)) = frame_meta.get(leaf_idx as usize) else {
                        continue;
                    };
                    if fn_name.is_empty() {
                        continue;
                    }

                    // Weight in microseconds (default 10_000us = 10ms sample tick if omitted)
                    let weight_us = weights
                        .and_then(|w| w.get(idx))
                        .and_then(|v| v.as_f64())
                        .map(|w| {
                            if w < 100.0 {
                                (w * 1000.0).max(100.0) as u64
                            } else {
                                w as u64
                            }
                        })
                        .unwrap_or(10_000);

                    total_cpu_us += weight_us;
                    let entry = by_fn.entry(fn_name.clone()).or_insert_with(|| TelemetryData {
                        function_name: fn_name.clone(),
                        execution_times_ms: Vec::new(),
                        alloc_delta_kb: 0.0,
                        invocation_count: 0,
                        cpu_self_us: 0,
                        session_cpu_total_us: 0,
                        source_url: file.clone(),
                        line_number: *line,
                    });
                    entry.invocation_count += 1;
                    entry.cpu_self_us += weight_us;
                    entry.execution_times_ms.push(weight_us as f64 / 1000.0);
                }
            }
        }
    }

    let total_cpu_us = total_cpu_us.max(1);
    let mut functions: Vec<TelemetryData> = by_fn
        .into_values()
        .map(|mut d| {
            d.session_cpu_total_us = total_cpu_us;
            d
        })
        .collect();
    functions.sort_by(|a, b| b.cpu_self_us.cmp(&a.cpu_self_us));

    let mut session = TelemetrySession::new("speedscope-offline");
    session.total_duration_ms = total_cpu_us as f64 / 1000.0;
    session.functions = functions;
    Ok(session)
}

/// Parse Folded Stack Flamegraph text (`foo;bar;baz 42`).
fn parse_folded_stacks(raw: &str) -> Result<TelemetrySession> {
    let mut by_fn: HashMap<String, TelemetryData> = HashMap::new();
    let mut total_cpu_us: u64 = 0;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Split at last whitespace: `<stack_frames> <sample_count>`
        let Some((stack_part, count_part)) = trimmed.rsplit_once(char::is_whitespace) else {
            continue;
        };
        let Ok(samples) = count_part.trim().parse::<u64>() else {
            continue;
        };
        if samples == 0 {
            continue;
        }
        let leaf_fn = stack_part
            .rsplit(';')
            .next()
            .unwrap_or(stack_part)
            .trim()
            .to_string();
        if leaf_fn.is_empty() {
            continue;
        }

        // Each folded sample represents 1ms (1000us) by convention
        let cpu_us = samples * 1000;
        total_cpu_us += cpu_us;

        let entry = by_fn.entry(leaf_fn.clone()).or_insert_with(|| TelemetryData {
            function_name: leaf_fn,
            execution_times_ms: Vec::new(),
            alloc_delta_kb: 0.0,
            invocation_count: 0,
            cpu_self_us: 0,
            session_cpu_total_us: 0,
            source_url: None,
            line_number: None,
        });
        entry.invocation_count += samples;
        entry.cpu_self_us += cpu_us;
        entry.execution_times_ms.push(samples as f64);
    }

    if by_fn.is_empty() {
        return Err(anyhow!("No valid folded stack lines (`foo;bar 10`) found in profile"));
    }

    let total_cpu_us = total_cpu_us.max(1);
    let mut functions: Vec<TelemetryData> = by_fn
        .into_values()
        .map(|mut d| {
            d.session_cpu_total_us = total_cpu_us;
            d
        })
        .collect();
    functions.sort_by(|a, b| b.cpu_self_us.cmp(&a.cpu_self_us));

    let mut session = TelemetrySession::new("flamegraph-folded-offline");
    session.total_duration_ms = total_cpu_us as f64 / 1000.0;
    session.functions = functions;
    Ok(session)
}

/// Parse OpenTelemetry (OTLP) Trace JSON (`resourceSpans -> scopeSpans -> spans`).
fn parse_otlp_trace_json(json: &Value) -> Result<TelemetrySession> {
    let mut span_list: Vec<&Value> = Vec::new();

    if let Some(res_spans) = json.get("resourceSpans").and_then(|v| v.as_array()) {
        for rs in res_spans {
            if let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) {
                for ss in scope_spans {
                    if let Some(spans) = ss.get("spans").and_then(|v| v.as_array()) {
                        span_list.extend(spans.iter());
                    }
                }
            }
        }
    } else if let Some(spans) = json.get("spans").and_then(|v| v.as_array()) {
        span_list.extend(spans.iter());
    }

    if span_list.is_empty() {
        return Err(anyhow!("No OTLP spans found in JSON"));
    }

    let mut by_fn: HashMap<String, TelemetryData> = HashMap::new();
    let mut total_cpu_us: u64 = 0;

    for span in span_list {
        let Some(name) = span.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let start_ns = parse_unix_nano(span.get("startTimeUnixNano")).unwrap_or(0);
        let end_ns = parse_unix_nano(span.get("endTimeUnixNano")).unwrap_or(start_ns + 1_000_000);
        let dur_us = end_ns.saturating_sub(start_ns) / 1000;
        let dur_ms = dur_us as f64 / 1000.0;

        total_cpu_us += dur_us.max(1);
        let entry = by_fn.entry(name.to_string()).or_insert_with(|| TelemetryData {
            function_name: name.to_string(),
            execution_times_ms: Vec::new(),
            alloc_delta_kb: 0.0,
            invocation_count: 0,
            cpu_self_us: 0,
            session_cpu_total_us: 0,
            source_url: None,
            line_number: None,
        });
        entry.invocation_count += 1;
        entry.cpu_self_us += dur_us.max(1);
        entry.execution_times_ms.push(dur_ms);
    }

    let total_cpu_us = total_cpu_us.max(1);
    let mut functions: Vec<TelemetryData> = by_fn
        .into_values()
        .map(|mut d| {
            d.session_cpu_total_us = total_cpu_us;
            d
        })
        .collect();
    functions.sort_by(|a, b| b.cpu_self_us.cmp(&a.cpu_self_us));

    let mut session = TelemetrySession::new("otlp-trace-offline");
    session.total_duration_ms = total_cpu_us as f64 / 1000.0;
    session.functions = functions;
    Ok(session)
}

fn parse_unix_nano(val: Option<&Value>) -> Option<u64> {
    let v = val?;
    if let Some(u) = v.as_u64() {
        Some(u)
    } else if let Some(s) = v.as_str() {
        s.parse::<u64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_v8_cpuprofile() {
        let raw = r#"{
            "nodes": [
                { "id": 1, "callFrame": { "functionName": "(root)", "url": "", "lineNumber": -1 }, "hitCount": 0 },
                { "id": 2, "callFrame": { "functionName": "verifyToken", "url": "src/auth.ts", "lineNumber": 14 }, "hitCount": 25 }
            ],
            "samples": [2, 2, 2],
            "timeDeltas": [120000, 150000, 180000]
        }"#;
        let session = parse_profile_str(raw).expect("Should parse V8 .cpuprofile");
        assert_eq!(session.backend, "v8-cpuprofile-offline");
        let f = session.find_function("verifyToken").expect("Should find verifyToken");
        assert_eq!(f.invocation_count, 25);
        assert!(f.p99_ms() >= 150.0);
    }

    #[test]
    fn test_parse_speedscope_json() {
        let raw = r#"{
            "shared": {
                "frames": [
                    { "name": "main", "file": "app.py", "line": 1 },
                    { "name": "compute_embeddings", "file": "ml.py", "line": 42 }
                ]
            },
            "profiles": [
                {
                    "type": "sampled",
                    "samples": [[0, 1], [0, 1], [0]],
                    "weights": [250000, 300000, 10000]
                }
            ]
        }"#;
        let session = parse_profile_str(raw).expect("Should parse Speedscope JSON");
        assert_eq!(session.backend, "speedscope-offline");
        let f = session.find_function("compute_embeddings").expect("Should find compute_embeddings");
        assert_eq!(f.invocation_count, 2);
        assert!(f.cpu_percent_self() > 90.0);
    }

    #[test]
    fn test_parse_folded_stacks() {
        let raw = "\
main;http_handler;UserService::getUser 180
main;http_handler;OrderService::createOrder 45
";
        let session = parse_profile_str(raw).expect("Should parse folded stacks");
        assert_eq!(session.backend, "flamegraph-folded-offline");
        let f = session.find_function("getUser").expect("Should find getUser");
        assert_eq!(f.invocation_count, 180);
        assert!(f.cpu_percent_self() > 75.0);
    }

    #[test]
    fn test_parse_otlp_trace_json() {
        let raw = r#"{
            "resourceSpans": [{
                "scopeSpans": [{
                    "spans": [
                        {
                            "name": "CheckoutService::chargeCard",
                            "startTimeUnixNano": "1700000000000000000",
                            "endTimeUnixNano": "1700000000250000000"
                        }
                    ]
                }]
            }]
        }"#;
        let session = parse_profile_str(raw).expect("Should parse OTLP trace JSON");
        assert_eq!(session.backend, "otlp-trace-offline");
        let f = session.find_function("chargeCard").expect("Should find chargeCard");
        assert_eq!(f.invocation_count, 1);
        assert!((f.p50_ms() - 250.0).abs() < 1.0);
    }
}
