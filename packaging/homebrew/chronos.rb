class Chronos < Formula
  desc "Single-binary Rust MCP server & CLI for 5D Code Graph, AST editing, and blast-radius gating"
  homepage "https://github.com/ptai-eng/chronos-agent"
  version "1.8.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{version}/chronos-macos-arm64.tar.gz"
      sha256 "ec672853c5abe717e0cc57c0800d06723203999899ac59be5eb8d46f73e91343"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{version}/chronos-linux-x64.tar.gz"
      sha256 "17d088ce7714f3f3080be919e2a61a25c1fbd2c59a3e4c7c2928544dd1b74e31"
    end
  end

  def install
    bin.install "chronos"
    bin.install "chronos-mcp"
  end

  test do
    assert_match "CHRONOS", shell_output("#{bin}/chronos --help")
    assert_match "CHRONOS", shell_output("#{bin}/chronos-mcp --help")
  end
end
