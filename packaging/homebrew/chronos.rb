class Chronos < Formula
  desc "Single-binary Rust MCP server & CLI for 5D Code Graph, AST editing, and blast-radius gating"
  homepage "https://github.com/ptai-eng/chronos-agent"
  version "1.8.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{version}/chronos-macos-arm64.tar.gz"
      sha256 "85a69214353b348d5a3bf64a3545bc6e09b2d2c31196f4d086a5b3987ad95031"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/ptai-eng/chronos-agent/releases/download/v#{version}/chronos-linux-x64.tar.gz"
      sha256 "0cc389a9d27f4d71efe97399e7df310c86b9965f5cea5d4317e770e7933ba60c"
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
