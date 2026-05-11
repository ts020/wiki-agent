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

probe_url() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsIL -o /dev/null "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget --spider -q "$1"
  else
    err "curl or wget is required"
  fi
}

if [ "$VERSION" != "latest" ] && ! probe_url "$archive_url"; then
  case "$VERSION" in
    v*)
      alt_version="${VERSION#v}"
      alt_base="https://github.com/$REPO/releases/download/$alt_version"
      ;;
    *)
      alt_version="v$VERSION"
      alt_base="https://github.com/$REPO/releases/download/$alt_version"
      ;;
  esac
  if probe_url "$alt_base/$asset"; then
    printf 'Falling back to tag %s\n' "$alt_version" >&2
    archive_url="$alt_base/$asset"
    checksum_url="$alt_base/checksums.txt"
  fi
fi

need tar
tmp="$(mktemp -d 2>/dev/null || mktemp -d -t md-wiki)"
trap 'rm -rf "$tmp"' EXIT INT TERM

printf 'Downloading %s\n' "$archive_url"
download "$archive_url" "$tmp/$asset"

skip_checksum_reason=""
expected=""
actual=""
if download "$checksum_url" "$tmp/checksums.txt"; then
  expected=$(awk -v f="$asset" '$2==f && length($1)==64 && $1 ~ /^[0-9a-fA-F]+$/ {print $1; exit}' "$tmp/checksums.txt")
  if [ -z "$expected" ]; then
    skip_checksum_reason="checksums.txt has no valid sha256 entry for $asset"
  elif command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$asset" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')
  else
    skip_checksum_reason="sha256sum and shasum are both unavailable"
  fi
else
  skip_checksum_reason="checksums.txt could not be downloaded from $checksum_url"
fi

if [ -n "$expected" ] && [ -n "$actual" ]; then
  if [ "$expected" != "$actual" ]; then
    err "checksum mismatch for $asset
  expected: $expected
  actual:   $actual"
  fi
  printf 'Checksum verified: %s\n' "$asset"
fi

if [ -n "$skip_checksum_reason" ]; then
  if [ "${MD_WIKI_SKIP_CHECKSUM:-}" = "1" ]; then
    printf 'Skipping checksum verification: %s\n' "$skip_checksum_reason" >&2
  else
    err "checksum verification failed: $skip_checksum_reason
Re-run with MD_WIKI_SKIP_CHECKSUM=1 only if you understand the risk."
  fi
fi

extract_dir="$tmp/extract"
mkdir -p "$extract_dir"
tar -xzf "$tmp/$asset" -C "$extract_dir"
extracted_bin="$(find "$extract_dir" -type f -name "$bin" | head -n 1)"
[ -n "$extracted_bin" ] || err "archive did not contain $bin"
case "$extracted_bin" in
  "$extract_dir"/*) ;;
  *) err "extracted binary path is outside the extraction directory: $extracted_bin" ;;
esac

mkdir -p "$INSTALL_DIR"
cp "$extracted_bin" "$INSTALL_DIR/$bin"
chmod 0755 "$INSTALL_DIR/$bin"

printf 'Installed %s to %s\n' "$bin" "$INSTALL_DIR/$bin"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) printf 'Add %s to PATH to run %s directly.\n' "$INSTALL_DIR" "$BIN_NAME" >&2 ;;
esac
