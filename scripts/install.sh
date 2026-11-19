#!/usr/bin/env sh
# One-line installer for CHRONOS-AGENT (chronos + chronos-mcp) on Linux x64 and macOS arm64.
# Usage: curl -fsSL https://raw.githubusercontent.com/ptai-eng/chronos-agent/main/scripts/install.sh | sh
set -eu

REPO="ptai-eng/chronos-agent"
INSTALL_DIR="${CHRONOS_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${CHRONOS_VERSION:-}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS:$ARCH" in
  Linux:x86_64|Linux:amd64)
    ARCHIVE="chronos-linux-x64.tar.gz"
    ;;
  Darwin:arm64|Darwin:aarch64)
    ARCHIVE="chronos-macos-arm64.tar.gz"
    ;;
  *)
    echo "Unsupported prebuilt platform '$OS/$ARCH'." >&2
    echo "Prebuilt archives target Linux x86_64 and macOS arm64. Build from source with: cargo build --release -j 4" >&2
    exit 1
    ;;
esac

if ! command -v curl >/dev/null 2>&1; then
  echo "Error: curl is required to download release artifacts." >&2
  exit 1
fi
if ! command -v tar >/dev/null 2>&1; then
  echo "Error: tar is required to extract release artifacts." >&2
  exit 1
fi

if [ -z "$VERSION" ]; then
  echo "Resolving latest CHRONOS-AGENT release tag..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$VERSION" ]; then
    echo "Error: failed to resolve latest release tag from GitHub API." >&2
    exit 1
  fi
fi

case "$VERSION" in
  v*) ;;
  *) VERSION="v$VERSION" ;;
esac

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

ARCHIVE_URL="https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE"
SUMS_URL="https://github.com/$REPO/releases/download/$VERSION/SHA256SUMS.txt"

echo "Downloading $ARCHIVE ($VERSION)..."
curl -fsSL "$ARCHIVE_URL" -o "$TMP_DIR/$ARCHIVE"

# Verify SHA-256 checksum automatically when SHA256SUMS.txt exists on the release
if curl -fsSL "$SUMS_URL" -o "$TMP_DIR/SHA256SUMS.txt" 2>/dev/null; then
  EXPECTED="$(grep " $ARCHIVE\$" "$TMP_DIR/SHA256SUMS.txt" | awk '{print $1}' | head -n 1 || true)"
  if [ -n "$EXPECTED" ]; then
    if command -v sha256sum >/dev/null 2>&1; then
      ACTUAL="$(sha256sum "$TMP_DIR/$ARCHIVE" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
      ACTUAL="$(shasum -a 256 "$TMP_DIR/$ARCHIVE" | awk '{print $1}')"
    else
      ACTUAL=""
    fi
    if [ -n "$ACTUAL" ] && [ "$ACTUAL" != "$EXPECTED" ]; then
      echo "SHA-256 verification failed for $ARCHIVE (expected $EXPECTED, got $ACTUAL)" >&2
      exit 1
    fi
    [ -n "$ACTUAL" ] && echo "Verified SHA-256 checksum ($ACTUAL)."
  fi
fi

mkdir -p "$TMP_DIR/extracted" "$INSTALL_DIR"
tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR/extracted"

for bin in chronos chronos-mcp; do
  if [ ! -f "$TMP_DIR/extracted/$bin" ]; then
    echo "Error: expected binary '$bin' missing from $ARCHIVE" >&2
    exit 1
  fi
  cp "$TMP_DIR/extracted/$bin" "$INSTALL_DIR/$bin"
  chmod +x "$INSTALL_DIR/$bin"
done

"$INSTALL_DIR/chronos" --version
echo "Installed chronos and chronos-mcp into $INSTALL_DIR"

case ":${PATH:-}:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Add $INSTALL_DIR to your PATH if not already present:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo "Next step inside any repo: chronos init --client all --with-hooks"
