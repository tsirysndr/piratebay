#!/usr/bin/env sh
# piratebay installer
#
#   curl -fsSL https://raw.githubusercontent.com/tsirysndr/piratebay/main/install.sh | bash
#
# Environment overrides:
#   PIRATEBAY_VERSION   Release tag to install (default: latest)
#   PIRATEBAY_INSTALL   Install directory     (default: /usr/local/bin, fallback ~/.local/bin)

set -eu

REPO="tsirysndr/piratebay"
BIN="piratebay"
VERSION="${PIRATEBAY_VERSION:-latest}"
INSTALL_DIR="${PIRATEBAY_INSTALL:-}"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m!\033[0m %s\n' "$*" >&2; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || error "missing required command: $1"
}

need uname
need tar
need mktemp

if command -v curl >/dev/null 2>&1; then
  FETCH="curl -fsSL"
elif command -v wget >/dev/null 2>&1; then
  FETCH="wget -qO-"
else
  error "need curl or wget to download release assets"
fi

# Normalize the CPU architecture.
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)   ARCH64="x86_64" ;;
  arm64|aarch64)  ARCH64="aarch64" ;;
  *)              error "unsupported architecture: $ARCH" ;;
esac

# Map (OS, arch) onto the Rust target triple used in the release asset names.
OS="$(uname -s)"
case "$OS" in
  Linux)
    case "$ARCH64" in
      x86_64)  TARGET="x86_64-unknown-linux-musl" ;;
      aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    esac ;;
  Darwin)
    case "$ARCH64" in
      x86_64)  TARGET="x86_64-apple-darwin" ;;
      aarch64) TARGET="aarch64-apple-darwin" ;;
    esac ;;
  FreeBSD)
    [ "$ARCH64" = "x86_64" ] || error "unsupported architecture on FreeBSD: $ARCH"
    TARGET="x86_64-unknown-freebsd" ;;
  NetBSD)
    [ "$ARCH64" = "x86_64" ] || error "unsupported architecture on NetBSD: $ARCH"
    TARGET="x86_64-unknown-netbsd" ;;
  OpenBSD)
    [ "$ARCH64" = "x86_64" ] || error "unsupported architecture on OpenBSD: $ARCH"
    TARGET="x86_64-unknown-openbsd" ;;
  *)
    error "unsupported OS: $OS" ;;
esac

# Resolve the release tag.
if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release for $REPO"
  VERSION="$($FETCH "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' \
    | head -n1 \
    | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
  [ -n "$VERSION" ] || error "could not resolve latest release tag"
fi

case "$VERSION" in
  v*) ;;
  *)  VERSION="v$VERSION" ;;
esac

ASSET="${BIN}_${VERSION}_${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

# Pick an install directory.
if [ -z "$INSTALL_DIR" ]; then
  if [ "$(id -u)" = "0" ] || [ -w "/usr/local/bin" ] 2>/dev/null; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="$HOME/.local/bin"
  fi
fi
mkdir -p "$INSTALL_DIR"

TMP="$(mktemp -d 2>/dev/null || mktemp -d -t piratebay)"
trap 'rm -rf "$TMP"' EXIT INT TERM

info "Downloading $ASSET"
case "$FETCH" in
  curl*) curl -fsSL "$URL" -o "$TMP/$ASSET"  || error "download failed: $URL" ;;
  wget*) wget -qO "$TMP/$ASSET" "$URL"       || error "download failed: $URL" ;;
esac

# Best-effort checksum verification. Release assets use different digest
# extensions depending on the platform (.sha512 / .sha256 / .sha256sum).
verify_checksum() {
  _sum="$TMP/checksum"
  for _ext in sha512 sha256 sha256sum; do
    if eval "$FETCH \"$URL.$_ext\"" > "$_sum" 2>/dev/null && [ -s "$_sum" ]; then
      _expected="$(awk '{print $1}' "$_sum")"
      case "$_ext" in
        sha512)
          if command -v sha512sum >/dev/null 2>&1; then
            _actual="$(sha512sum "$TMP/$ASSET" | awk '{print $1}')"
          elif command -v shasum >/dev/null 2>&1; then
            _actual="$(shasum -a 512 "$TMP/$ASSET" | awk '{print $1}')"
          else
            warn "no sha512 tool available, skipping verification"; return 0
          fi ;;
        *)
          if command -v sha256sum >/dev/null 2>&1; then
            _actual="$(sha256sum "$TMP/$ASSET" | awk '{print $1}')"
          elif command -v shasum >/dev/null 2>&1; then
            _actual="$(shasum -a 256 "$TMP/$ASSET" | awk '{print $1}')"
          else
            warn "no sha256 tool available, skipping verification"; return 0
          fi ;;
      esac
      [ "$_expected" = "$_actual" ] || error "checksum mismatch (expected $_expected, got $_actual)"
      info "Checksum verified ($_ext)"
      return 0
    fi
  done
  warn "no published checksum, skipping verification"
}
verify_checksum

info "Extracting"
tar -xzf "$TMP/$ASSET" -C "$TMP"
[ -f "$TMP/$BIN" ] || error "binary '$BIN' not found in archive"

INSTALLED="$INSTALL_DIR/$BIN"
if [ -w "$INSTALL_DIR" ]; then
  install -m 0755 "$TMP/$BIN" "$INSTALLED"
else
  info "Elevating with sudo to install into $INSTALL_DIR"
  sudo install -m 0755 "$TMP/$BIN" "$INSTALLED"
fi

info "Installed $BIN $VERSION to $INSTALLED"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    warn "$INSTALL_DIR is not on your PATH"
    warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

"$INSTALLED" --version 2>/dev/null || true
