#!/bin/sh
set -eu

REPO="${MD_WIKI_REPO:-ts020/wiki-agent}"
VERSION="${MD_WIKI_VERSION:-latest}"
BIN_NAME="md-wiki"

err() {
  printf 'md-wiki install: %s\n' "$*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || err "required command not found: $1"
}

if [ -z "${MD_WIKI_INSTALL_DIR:-}" ] && [ -z "${HOME:-}" ]; then
  err "HOME is unset; set MD_WIKI_INSTALL_DIR explicitly"
fi
INSTALL_DIR="${MD_WIKI_INSTALL_DIR:-$HOME/.local/bin}"

download() {
  url="$1"
  out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$out" "$url"
  else
    err "curl or wget is required"
  fi
}

detect_target() {
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m | tr '[:upper:]' '[:lower:]')"

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *) err "unsupported architecture: $arch" ;;
  esac

  case "$os" in
    linux)
      [ "$arch" = "x86_64" ] || err "unsupported Linux architecture: $arch"
      printf '%s\n' "x86_64-unknown-linux-gnu"
      ;;
    darwin)
      case "$arch" in
        x86_64) printf '%s\n' "x86_64-apple-darwin" ;;
        aarch64) printf '%s\n' "aarch64-apple-darwin" ;;
      esac
      ;;
    mingw*|msys*|cygwin*)
      [ "$arch" = "x86_64" ] || err "unsupported Windows architecture: $arch"
      printf '%s\n' "x86_64-pc-windows-msvc"
      ;;
    *)
      err "unsupported OS: $os"
      ;;
  esac
}

target="$(detect_target)"
bin="$BIN_NAME"
case "$target" in
  *windows-msvc) bin="$BIN_NAME.exe" ;;
esac

asset="$BIN_NAME-$target.tar.gz"
if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/$REPO/releases/latest/download"
else
  base_url="https://github.com/$REPO/releases/download/$VERSION"
fi
archive_url="$base_url/$asset"
checksum_url="$base_url/checksums.txt"

need tar
tmp="$(mktemp -d 2>/dev/null || mktemp -d -t md-wiki)"
trap 'rm -rf "$tmp"' EXIT INT TERM

printf 'Downloading %s\n' "$archive_url"
download "$archive_url" "$tmp/$asset"

if download "$checksum_url" "$tmp/checksums.txt"; then
  if grep "  $asset\$" "$tmp/checksums.txt" >"$tmp/checksum.selected"; then
    if command -v sha256sum >/dev/null 2>&1; then
      (cd "$tmp" && sha256sum -c checksum.selected)
    elif command -v shasum >/dev/null 2>&1; then
      (cd "$tmp" && shasum -a 256 -c checksum.selected)
    else
      printf 'Checksum file found, but sha256sum/shasum is unavailable; skipping verification.\n' >&2
    fi
  else
    printf 'Checksum file does not list %s; skipping verification.\n' "$asset" >&2
  fi
else
  printf 'checksums.txt is unavailable; skipping verification.\n' >&2
fi

tar -xzf "$tmp/$asset" -C "$tmp"
extracted_bin="$(find "$tmp" -type f -name "$bin" | head -n 1)"
[ -n "$extracted_bin" ] || err "archive did not contain $bin"

mkdir -p "$INSTALL_DIR"
cp "$extracted_bin" "$INSTALL_DIR/$bin"
chmod 0755 "$INSTALL_DIR/$bin"

printf 'Installed %s to %s\n' "$bin" "$INSTALL_DIR/$bin"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) printf 'Add %s to PATH to run %s directly.\n' "$INSTALL_DIR" "$BIN_NAME" >&2 ;;
esac
