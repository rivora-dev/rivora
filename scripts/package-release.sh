#!/usr/bin/env sh
# Package Rivora release archives for a single target triple.
#
# Usage:
#   scripts/package-release.sh <version> <target> <bin-dir> <out-dir>
#
# Example:
#   scripts/package-release.sh v0.9.1 aarch64-apple-darwin target/release dist
#
# Produces:
#   rivora-<version>-<target>.tar.gz containing rivora, rivora-workspace (if present),
#   LICENSE, and README.md.
#
# shellcheck shell=sh

set -eu

VERSION="${1:-}"
TARGET="${2:-}"
BIN_DIR="${3:-}"
OUT_DIR="${4:-}"

die() { printf 'Error: %s\n' "$*" >&2; exit 1; }

[ -n "$VERSION" ] || die "usage: package-release.sh <version> <target> <bin-dir> <out-dir>"
[ -n "$TARGET" ]  || die "usage: package-release.sh <version> <target> <bin-dir> <out-dir>"
[ -n "$BIN_DIR" ] || die "usage: package-release.sh <version> <target> <bin-dir> <out-dir>"
[ -n "$OUT_DIR" ] || die "usage: package-release.sh <version> <target> <bin-dir> <out-dir>"

# Normalize version to vX.Y.Z
case "$VERSION" in
    v*) ;;
    *) VERSION="v${VERSION}" ;;
esac

ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
RIVORA_BIN="${BIN_DIR}/rivora"
WS_BIN="${BIN_DIR}/rivora-workspace"

[ -f "$RIVORA_BIN" ] || die "Missing binary: ${RIVORA_BIN}"
[ -x "$RIVORA_BIN" ] || chmod +x "$RIVORA_BIN" || die "rivora is not executable"

# Smoke --version when the binary can run on this host
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)   HOST_TARGET="aarch64-apple-darwin" ;;
    Darwin-x86_64)  HOST_TARGET="x86_64-apple-darwin" ;;
    Linux-x86_64)   HOST_TARGET="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64|Linux-arm64) HOST_TARGET="aarch64-unknown-linux-gnu" ;;
    *) HOST_TARGET="" ;;
esac

if [ -n "$HOST_TARGET" ] && [ "$HOST_TARGET" = "$TARGET" ]; then
    VER_OUT="$("$RIVORA_BIN" --version 2>/dev/null || true)"
    BARE="${VERSION#v}"
    case "$VER_OUT" in
        *"$BARE"*) ;;
        *) die "rivora --version did not report ${BARE}: ${VER_OUT:-<empty>}" ;;
    esac
fi

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/rivora-pkg.XXXXXX")"
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT INT HUP TERM

cp "$RIVORA_BIN" "${STAGE}/rivora"
chmod 755 "${STAGE}/rivora"

INCLUDE_WS=0
if [ -f "$WS_BIN" ]; then
    cp "$WS_BIN" "${STAGE}/rivora-workspace"
    chmod 755 "${STAGE}/rivora-workspace"
    INCLUDE_WS=1
fi

cp "${ROOT}/LICENSE" "${STAGE}/LICENSE"
cp "${ROOT}/README.md" "${STAGE}/README.md"

mkdir -p "$OUT_DIR"
ARCHIVE_NAME="rivora-${VERSION}-${TARGET}.tar.gz"
ARCHIVE_PATH="${OUT_DIR}/${ARCHIVE_NAME}"

# Create archive with only intended members (no full paths)
if [ "$INCLUDE_WS" -eq 1 ]; then
    tar -czf "$ARCHIVE_PATH" -C "$STAGE" rivora rivora-workspace LICENSE README.md
else
    tar -czf "$ARCHIVE_PATH" -C "$STAGE" rivora LICENSE README.md
fi

# Verify archive contents
tar -tzf "$ARCHIVE_PATH" | while IFS= read -r member || [ -n "$member" ]; do
    case "$member" in
        /*|*../*|*/../*) die "Unsafe archive member: ${member}" ;;
    esac
done

# Confirm required members
tar -tzf "$ARCHIVE_PATH" | grep -qx 'rivora' || die "Archive missing rivora"
tar -tzf "$ARCHIVE_PATH" | grep -qx 'LICENSE' || die "Archive missing LICENSE"
tar -tzf "$ARCHIVE_PATH" | grep -qx 'README.md' || die "Archive missing README.md"

printf '%s\n' "$ARCHIVE_PATH"
