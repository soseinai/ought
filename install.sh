#!/bin/sh
# install.sh — installer for ought
# https://github.com/soseinai/ought
#
# Usage:
#   curl -sS https://sosein.ai/install.sh | sh
#
# Environment variables:
#   OUGHT_VERSION     install a specific version (e.g. v0.1.0). Default: latest
#   OUGHT_INSTALL_DIR install location. Default: $HOME/.local/bin

set -eu

# --- Configuration ----------------------------------------------------
REPO="soseinai/ought"
VERSION="${OUGHT_VERSION:-latest}"
INSTALL_DIR="${OUGHT_INSTALL_DIR:-$HOME/.local/bin}"

# --- Helpers ----------------------------------------------------------
red()   { printf '\033[31m%s\033[0m' "$1"; }
green() { printf '\033[32m%s\033[0m' "$1"; }
bold()  { printf '\033[1m%s\033[0m'  "$1"; }

err() {
  printf '%s %s\n' "$(red 'error:')" "$1" >&2
  exit 1
}

info() {
  printf '%s %s\n' "$(green '==>')" "$1"
}

# --- Detect OS and architecture ---------------------------------------
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Linux)  os_target="unknown-linux-gnu" ;;
  Darwin) os_target="apple-darwin" ;;
  *) err "unsupported operating system: $OS (ought supports Linux and macOS)" ;;
esac

case "$ARCH" in
  x86_64|amd64)  arch_target="x86_64" ;;
  arm64|aarch64) arch_target="aarch64" ;;
  *) err "unsupported architecture: $ARCH" ;;
esac

target="${arch_target}-${os_target}"

# --- Resolve download URL ---------------------------------------------
if [ "$VERSION" = "latest" ]; then
  url="https://github.com/${REPO}/releases/latest/download/ought-${target}.tar.gz"
else
  v="${VERSION#v}"
  url="https://github.com/${REPO}/releases/download/v${v}/ought-${target}.tar.gz"
fi

info "Detected platform: ${target}"
info "Downloading ${url}"

# --- Check prerequisites ----------------------------------------------
if ! command -v curl >/dev/null 2>&1; then
  err "curl is required but not installed"
fi

if ! command -v tar >/dev/null 2>&1; then
  err "tar is required but not installed"
fi

# --- Download and extract ---------------------------------------------
TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t ought)
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

if ! curl -fsSL --output "$TMP_DIR/ought.tar.gz" "$url"; then
  err "failed to download $url

The release for ${target} may not exist yet. Check available releases at:
  https://github.com/${REPO}/releases"
fi

if ! tar -xzf "$TMP_DIR/ought.tar.gz" -C "$TMP_DIR"; then
  err "failed to extract the downloaded archive"
fi

if [ ! -f "$TMP_DIR/ought" ]; then
  err "the downloaded archive did not contain an 'ought' binary"
fi

# --- Install ----------------------------------------------------------
mkdir -p "$INSTALL_DIR"
mv "$TMP_DIR/ought" "$INSTALL_DIR/ought"
chmod +x "$INSTALL_DIR/ought"

info "Installed ought to $(bold "$INSTALL_DIR/ought")"

# --- Verify -----------------------------------------------------------
if "$INSTALL_DIR/ought" --version >/dev/null 2>&1; then
  installed_version=$("$INSTALL_DIR/ought" --version 2>/dev/null || echo "unknown")
  info "Verified: ${installed_version}"
else
  printf '%s the binary was installed but failed to run\n' "$(red 'warning:')" >&2
fi

# --- PATH advice ------------------------------------------------------
case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    printf '\n'
    info "$(bold "$INSTALL_DIR") is already in your PATH"
    info "Run $(bold 'ought --help') to get started"
    ;;
  *)
    printf '\n'
    printf '%s %s is not in your PATH.\n' "$(bold 'NOTE:')" "$INSTALL_DIR"
    printf '\n'
    printf 'Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):\n'
    printf '\n'
    printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
    printf '\n'
    ;;
esac
