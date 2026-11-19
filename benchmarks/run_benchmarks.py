"""Reproducible local benchmark comparing naive grep+read loops against CHRONOS-AGENT MCP tools.

Measures exact tool calls, response payload characters, estimated tokens (~chars / 4),
and wall-clock latency across self-contained medium-sized TypeScript, Python, and Rust workspaces.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent


def resolve_mcp_binary(explicit_path: str | None) -> Path:
    if explicit_path:
        p = Path(explicit_path)
        if not p.is_file():
            raise FileNotFoundError(f"MCP binary not found: {p}")
        return p

    exe = "chronos-mcp.exe" if os.name == "nt" else "chronos-mcp"
    for profile in ("release", "debug"):
        candidate = REPO_ROOT / "target" / profile / exe
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(
        "No compiled chronos-mcp binary found under target/{release,debug}/. "
        "Run `cargo build --bin chronos-mcp` or pass --bin."
    )


def build_fixtures(root: Path) -> list[dict[str, Any]]:
    # 1. Medium TypeScript Express-style multi-file service (~150+ lines per module)
    ts_dir = root / "ts_express" / "src"
    ts_dir.mkdir(parents=True, exist_ok=True)
    ts_repo_lines = [
        "export interface Invoice { id: string; cents: number; taxRate: number; currency: string; }\n",
        "export function calculateTotalCents(cents: number, taxRate: number): number {\n",
        "  const safeRate = Math.max(0, Math.min(taxRate, 0.5));\n",
        "  return Math.round(cents * (1 + safeRate));\n",
        "}\n",
    ]
    for i in range(1, 55):
        ts_repo_lines.append(
            f"export function pricingTierPolicy{i}(base: number, discount: number): number {{\n"
            f"  const net = Math.max(0, base - discount - {i});\n"
            f"  return net * 2;\n"
            f"}}\n"
        )
    (ts_dir / "pricing.ts").write_text("".join(ts_repo_lines), encoding="utf-8")

    checkout_padding = "".join(
        f"export function validateLineItem{i}(qty: number): boolean {{ return qty >= {i} && qty < 10000; }}\n"
        for i in range(1, 45)
    )
    (ts_dir / "checkout.ts").write_text(
        "import { calculateTotalCents } from './pricing';\n"
        "export function createOrder(cents: number): number {\n"
        "  const subtotal = cents > 0 ? cents : 0;\n"
        "  return calculateTotalCents(subtotal, 0.08);\n"
        "}\n" + checkout_padding,
        encoding="utf-8",
    )

    routes_padding = "".join(
        f"export function healthRouteV{i}(): string {{ return 'ok-v{i}'; }}\n"
        for i in range(1, 45)
    )
    (ts_dir / "routes.ts").write_text(
        "import { createOrder } from './checkout';\n"
        "export function postCheckoutRoute(reqBody: { cents: number }): number {\n"
        "  return createOrder(reqBody.cents);\n"
        "}\n" + routes_padding,
        encoding="utf-8",
    )
    (ts_dir / "pricing.test.ts").write_text(
        "import { calculateTotalCents } from './pricing';\n"
        "function testCalculateTotalCents() {\n"
        "  if (calculateTotalCents(100, 0.1) !== 110) throw new Error('mismatch');\n"
        "}\n",
        encoding="utf-8",
    )

    # 2. Medium Python FastAPI-style service
    py_dir = root / "py_fastapi" / "app"
    py_dir.mkdir(parents=True, exist_ok=True)
    py_lines = [
        "def verify_bearer_token(raw_header: str) -> str:\n",
        "    token = raw_header.strip()\n",
        "    if not token.startswith('Bearer '):\n",
        "        raise ValueError('Invalid token prefix')\n",
        "    return token[7:]\n\n",
    ]
    for i in range(1, 55):
        py_lines.append(
            f"def format_audit_tag_{i}(user_id: int, scope: str) -> str:\n"
            f"    normalized = scope.strip().lower()\n"
            f"    return f'u={{user_id}}:{i}:{{normalized}}'\n\n"
        )
    (py_dir / "auth.py").write_text("".join(py_lines), encoding="utf-8")

    py_user_pad = "".join(
        f"def load_profile_field_{i}(uid: int) -> str:\n    return f'field_{i}_{{uid}}'\n\n"
        for i in range(1, 45)
    )
    (py_dir / "users.py").write_text(
        "from app.auth import verify_bearer_token\n"
        "def get_current_user(header: str) -> dict:\n"
        "    tok = verify_bearer_token(header)\n"
        "    return {'token': tok}\n\n" + py_user_pad,
        encoding="utf-8",
    )
    (py_dir / "test_auth.py").write_text(
        "from app.auth import verify_bearer_token\n"
        "def test_verify_bearer_token():\n"
        "    assert verify_bearer_token('Bearer abc') == 'abc'\n",
        encoding="utf-8",
    )

    # 3. Medium Rust crate multi-module service
    rs_dir = root / "rs_service" / "src"
    rs_dir.mkdir(parents=True, exist_ok=True)
    rs_lines = [
        "pub fn compute_hash_bucket(key: &str, buckets: usize) -> usize {\n",
        "    let sum: usize = key.bytes().map(|b| b as usize).sum();\n",
        "    sum % buckets.max(1)\n",
        "}\n",
    ]
    for i in range(1, 55):
        rs_lines.append(
            f"pub fn metric_slot_{i}(v: usize, multiplier: usize) -> usize {{\n"
            f"    v.saturating_mul(multiplier).saturating_add({i})\n"
            f"}}\n"
        )
    (rs_dir / "shard.rs").write_text("".join(rs_lines), encoding="utf-8")

    rs_router_pad = "".join(
        f"pub fn route_fallback_{i}(id: usize) -> usize {{ id ^ {i} }}\n"
        for i in range(1, 45)
    )
    (rs_dir / "router.rs").write_text(
        "use crate::shard::compute_hash_bucket;\n"
        "pub fn route_key(key: &str) -> usize {\n"
        "    compute_hash_bucket(key, 16)\n"
        "}\n" + rs_router_pad,
        encoding="utf-8",
    )

    return [
        {
            "name": "TypeScript (Express-style)",
            "workspace": root / "ts_express",
            "files": [
                "src/pricing.ts",
                "src/checkout.ts",
                "src/routes.ts",
                "src/pricing.test.ts",
            ],
            "symbol": "calculateTotalCents",
        },
        {
            "name": "Python (FastAPI-style)",
            "workspace": root / "py_fastapi",
            "files": ["app/auth.py", "app/users.py", "app/test_auth.py"],
            "symbol": "verify_bearer_token",
        },
        {
            "name": "Rust (Multi-module)",
            "workspace": root / "rs_service",
            "files": ["src/shard.rs", "src/router.rs"],
            "symbol": "compute_hash_bucket",
        },
    ]


def measure_naive_grep_and_read(workspace: Path, symbol: str) -> dict[str, Any]:
    """Simulate a typical agent loop: 1 grep scan + reading every matching/related file in full."""
    t0 = time.perf_counter()
    pattern = re.compile(re.escape(symbol))
    tool_calls = 1
    grep_chars = 0
    matched_files: list[Path] = []

    for path in sorted(workspace.rglob("*")):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        hits = [
            f"{path.relative_to(workspace)}:{idx + 1}: {line}"
            for idx, line in enumerate(text.splitlines())
            if pattern.search(line)
        ]
        if hits:
            matched_files.append(path)
            grep_chars += sum(len(h) + 1 for h in hits)

    read_chars = 0
    for path in matched_files:
        tool_calls += 1
        read_chars += len(path.read_text(encoding="utf-8"))

    elapsed_ms = (time.perf_counter() - t0) * 1000.0
    total_chars = grep_chars + read_chars
    return {
        "tool_calls": tool_calls,
        "chars": total_chars,
        "est_tokens": round(total_chars / 4.0, 1),
        "elapsed_ms": round(elapsed_ms, 2),
    }


def rpc_call(
    proc: subprocess.Popen[str], req_id: int, method: str, params: dict[str, Any]
) -> dict[str, Any]:
    assert proc.stdin is not None and proc.stdout is not None
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
    )
    proc.stdin.write(payload + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError(f"EOF from chronos-mcp while awaiting response for {method}")
    return json.loads(line)


def measure_chronos_mcp(
    bin_path: Path, workspace: Path, files: list[str], symbol: str
) -> dict[str, Any]:
    proc = subprocess.Popen(
        [str(bin_path), "--workspace", str(workspace), "--no-watch"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        encoding="utf-8",
    )
    try:
        rpc_call(proc, 1, "initialize", {"protocolVersion": "2025-11-25"})
        assert proc.stdin is not None
        proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        proc.stdin.flush()

        req_id = 2
        for rel in files:
            rpc_call(
                proc,
                req_id,
                "tools/call",
                {"name": "chronos_analyze_file", "arguments": {"file_path": rel}},
            )
            req_id += 1

        latencies: list[float] = []
        payload_chars = 0
        for _ in range(5):
            t0 = time.perf_counter()
            resp = rpc_call(
                proc,
                req_id,
                "tools/call",
                {
                    "name": "chronos_explore",
                    "arguments": {
                        "query": symbol,
                        "include_body": False,
                        "include_blast_radius": True,
                        "max_symbols": 1,
                    },
                },
            )
            elapsed_ms = (time.perf_counter() - t0) * 1000.0
            latencies.append(elapsed_ms)
            req_id += 1
            text = resp["result"]["content"][0]["text"]
            payload_chars = len(text)

        return {
            "tool_calls": 1,
            "chars": payload_chars,
            "est_tokens": round(payload_chars / 4.0, 1),
            "median_ms": round(statistics.median(latencies), 2),
        }
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="CHRONOS-AGENT vs naive grep+read local benchmark"
    )
    parser.add_argument(
        "--bin", help="Explicit path to chronos-mcp binary", default=None
    )
    parser.add_argument(
        "--out", help="JSON report output path", default="benchmarks/results.json"
    )
    args = parser.parse_args()

    bin_path = resolve_mcp_binary(args.bin)
    with tempfile.TemporaryDirectory(prefix="chronos_bench_") as tmp:
        fixtures = build_fixtures(Path(tmp))
        rows = []
        for fx in fixtures:
            baseline = measure_naive_grep_and_read(fx["workspace"], fx["symbol"])
            chronos = measure_chronos_mcp(
                bin_path, fx["workspace"], fx["files"], fx["symbol"]
            )
            token_delta_pct = round(
                (
                    (chronos["est_tokens"] - baseline["est_tokens"])
                    / max(baseline["est_tokens"], 1.0)
                )
                * 100.0,
                1,
            )
            rows.append(
                {
                    "fixture": fx["name"],
                    "symbol": fx["symbol"],
                    "baseline_grep_read": baseline,
                    "chronos_explore_1call": chronos,
                    "token_delta_pct": token_delta_pct,
                    "tool_call_reduction": f"{baseline['tool_calls']} -> {chronos['tool_calls']}",
                }
            )

    out_path = (
        REPO_ROOT / args.out if not Path(args.out).is_absolute() else Path(args.out)
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "binary": str(bin_path),
        "token_formula": "chars / 4.0 (cl100k approximation)",
        "results": rows,
    }
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(
        "| Fixture | Naive Calls | CHRONOS Calls | Naive ~Tokens | CHRONOS ~Tokens | Delta | CHRONOS Median |"
    )
    print("| :--- | :---: | :---: | :---: | :---: | :---: | :---: |")
    for r in rows:
        b = r["baseline_grep_read"]
        c = r["chronos_explore_1call"]
        print(
            f"| {r['fixture']} | {b['tool_calls']} | {c['tool_calls']} | "
            f"{b['est_tokens']} ({b['chars']} chars) | {c['est_tokens']} ({c['chars']} chars) | "
            f"{r['token_delta_pct']:+}% | {c['median_ms']} ms |"
        )


if __name__ == "__main__":
    main()
