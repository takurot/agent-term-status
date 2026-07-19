#!/usr/bin/env bash
set -euo pipefail

# install.sh — install agent-term-status (ats) from a GitHub Release
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/takurot/agent-term-status/main/scripts/install.sh | bash
#
# Environment variables:
#   ATS_VERSION  — release tag (default: latest)
#   ATS_INSTALL_DIR — target directory (default: ~/.local/bin)

REPO="takurot/agent-term-status"
BINARY="ats"
DAEMON_BINARY="ats-daemon"

INSTALL_DIR="${ATS_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${ATS_VERSION:-latest}"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
Darwin)  OS="darwin" ;;
Linux)   OS="linux" ;;
*)
    echo "unsupported OS: $OS"
    exit 1
    ;;
esac

case "$ARCH" in
arm64|aarch64) ARCH="aarch64" ;;
x86_64|amd64)  ARCH="x86_64" ;;
*)
    echo "unsupported architecture: $ARCH"
    exit 1
    ;;
esac

ARCHIVE="agent-term-status-${ARCH}-${OS}.tar.gz"

if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"
else
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "→ downloading $DOWNLOAD_URL"
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$DOWNLOAD_URL" -O "$TMP_DIR/$ARCHIVE"
else
    echo "neither curl nor wget found; cannot download"
    exit 1
fi

echo "→ extracting"
tar xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

mkdir -p "$INSTALL_DIR"

for bin in "$BINARY" "$DAEMON_BINARY"; do
    SRC="$TMP_DIR/$bin"
    if [ ! -f "$SRC" ]; then
        echo "WARNING: $bin not found in archive"
        continue
    fi

    cp "$SRC" "$INSTALL_DIR/$bin"
    chmod +x "$INSTALL_DIR/$bin"
    echo "✓ installed $INSTALL_DIR/$bin"
done

# symlink agent-term-status → ats for cargo-install compatibility
if [ ! -e "$INSTALL_DIR/agent-term-status" ]; then
    ln -s "$INSTALL_DIR/$BINARY" "$INSTALL_DIR/agent-term-status"
fi

# PATH hint
if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$INSTALL_DIR"; then
    echo "→ add $INSTALL_DIR to your PATH:"
    case "${SHELL:-}" in
    *zsh)  echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc" ;;
    *bash) echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc" ;;
    *fish) echo "  fish_add_path $INSTALL_DIR" ;;
    *)     echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
    esac
fi

echo
echo "Installation complete! Try: ats --version"
