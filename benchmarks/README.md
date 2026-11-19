# CHRONOS-AGENT Reproducible Benchmark Suite

Measures real stdio JSON-RPC round-trips against `chronos-mcp` vs. a standard grep + full-file read agent loop across TypeScript, Python, and Rust fixtures.

## Usage

```bash
# Build chronos-mcp once
cargo build --release --bin chronos-mcp

# Run benchmark (writes benchmarks/results.json and prints summary table)
uv run python benchmarks/run_benchmarks.py
```

## Measured Local Results (`chronos-mcp` debug binary, Windows x64)

Measured with `uv run python benchmarks/run_benchmarks.py --bin target/debug/chronos-mcp.exe` (`chars / 4.0` token approximation, 5 warm iterations per fixture):

| Fixture | Naive Calls | CHRONOS Calls | Naive ~Tokens | CHRONOS ~Tokens | Token Delta | CHRONOS Median |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| TypeScript (Express-style) | 4 | 1 | 3265.5 (13062 chars) | 915.2 (3661 chars) | **-72.0%** | 5.20 ms |
| Python (FastAPI-style) | 4 | 1 | 2902.2 (11609 chars) | 887.8 (3551 chars) | **-69.4%** | 3.96 ms |
| Rust (Multi-module) | 3 | 1 | 2297.0 (9188 chars) | 646.2 (2585 chars) | **-71.9%** | 3.62 ms |

### Methodology
- **Token estimation:** Explicit character-to-token approximation `chars / 4.0` alongside exact raw payload character counts.
- **Latency:** Warm median over 5 invocations per fixture (`time.perf_counter()`).
- **Honesty:** On very small single-function toy files (`<30` lines total), `chronos_explore` still returns structured JSON metadata (`coverage`, `blast_radius`, `verification_surfaces`, `_meta`), so token reduction shows up on realistic medium/large multi-file modules (`150+` lines per file) while round-trip calls drop 3x–4x across all sizes.
