"""Render a SHA-256-verified Homebrew formula from release SHA256SUMS.txt."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def parse_sums(path: Path) -> dict[str, str]:
    sums: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.strip().split()
        if len(parts) >= 2:
            digest = parts[0]
            filename = Path(parts[-1]).name
            sums[filename] = digest
    return sums


def render_formula(version: str, macos_sha: str, linux_sha: str) -> str:
    clean_ver = version.lstrip("v")
    return f'''class Chronos < Formula
  desc "Single-binary Rust MCP server & CLI for 5D Code Graph, AST editing, and blast-radius gating"
  homepage "https://github.com/ptai-eng/chronos-agent"
  version "{clean_ver}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{{version}}/chronos-macos-arm64.tar.gz"
      sha256 "{macos_sha}"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{{version}}/chronos-linux-x64.tar.gz"
      sha256 "{linux_sha}"
    end
  end

  def install
    bin.install "chronos"
    bin.install "chronos-mcp"
  end

  test do
    assert_match "CHRONOS", shell_output("#{{bin}}/chronos --help")
    assert_match "CHRONOS", shell_output("#{{bin}}/chronos-mcp --help")
  end
end
'''


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--sums", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    sums = parse_sums(Path(args.sums))
    linux_sha = sums.get("chronos-linux-x64.tar.gz")
    macos_sha = sums.get("chronos-macos-arm64.tar.gz")
    if not linux_sha:
        print("Missing chronos-linux-x64.tar.gz in SHA256SUMS.txt", file=sys.stderr)
        return 1
    macos_sha = macos_sha or "PENDING_MACOS_BUILD"

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        render_formula(args.version, macos_sha, linux_sha), encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
