"""Verify release version parity across Cargo.toml, npm package.json, server.json, and Dockerfile."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml, re.MULTILINE)
    if not m:
        print("ERROR: Could not find workspace version in Cargo.toml", file=sys.stderr)
        return 1
    cargo_ver = m.group(1)

    pkg = json.loads(
        (ROOT / "npm" / "chronos-mcp" / "package.json").read_text(encoding="utf-8")
    )
    srv = json.loads((ROOT / "server.json").read_text(encoding="utf-8"))
    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")

    errors: list[str] = []
    if pkg.get("version") != cargo_ver:
        errors.append(
            f"npm/chronos-mcp/package.json version={pkg.get('version')} != Cargo.toml {cargo_ver}"
        )
    if srv.get("version") != cargo_ver:
        errors.append(
            f"server.json version={srv.get('version')} != Cargo.toml {cargo_ver}"
        )

    oci_id = (srv.get("packages") or [{}])[0].get("identifier", "")
    expected_suffix = f":{cargo_ver}"
    if not oci_id.endswith(expected_suffix):
        errors.append(
            f"server.json packages[0].identifier='{oci_id}' must end with '{expected_suffix}'"
        )

    dm = re.search(r"^ARG\s+CHRONOS_VERSION=v([^\s]+)", dockerfile, re.MULTILINE)
    docker_ver = dm.group(1) if dm else None
    if docker_ver != cargo_ver:
        errors.append(f"Dockerfile CHRONOS_VERSION=v{docker_ver} != v{cargo_ver}")

    if errors:
        for e in errors:
            print(f"VERSION MISMATCH: {e}", file=sys.stderr)
        return 1

    print(
        f"version_sync_ok: Cargo.toml == package.json == server.json == Dockerfile ({cargo_ver})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
