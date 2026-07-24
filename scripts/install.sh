#!/usr/bin/env sh
# Rivora installer — canonical source for https://rivora.dev/install
#
# Usage:
#   curl -fsSL https://rivora.dev/install | sh
#   curl -fsSL https://rivora.dev/install | RIVORA_VERSION=v0.9.1 sh
#   curl -fsSL https://rivora.dev/install | RIVORA_INSTALL_DIR=$HOME/.local/bin sh
#
# This script must work when executed via stdin (do not rely on $0 being a file).
# shellcheck shell=sh disable=SC2039,SC3043

set -eu

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

GITHUB_REPO="${RIVORA_GITHUB_REPO:-rivora-dev/rivora}"
GITHUB_API="${RIVORA_GITHUB_API:-https://api.github.com}"
GITHUB_RELEASES="${RIVORA_GITHUB_RELEASES:-https://github.com/${GITHUB_REPO}/releases}"
DOWNLOAD_BASE="${RIVORA_DOWNLOAD_BASE:-https://github.com/${GITHUB_REPO}/releases/download}"

# Optional overrides (documented):
#   RIVORA_VERSION       — e.g. v0.9.1 or 0.9.1
#   RIVORA_INSTALL_DIR   — destination directory for binaries
#   RIVORA_GITHUB_REPO   — override repo (tests)
#   RIVORA_GITHUB_API    — override API base (tests / mocks)
#   RIVORA_DOWNLOAD_BASE — override download base (tests / mocks)

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

info()  { printf '%s\n' "$*"; }
warn()  { printf 'Warning: %s\n' "$*" >&2; }
die()   { printf 'Error: %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Platform detection (testable)
# ---------------------------------------------------------------------------

# Normalize uname -s → rust OS component
detect_os() {
    _os="${1:-}"
    if [ -z "$_os" ]; then
        _os="$(uname -s 2>/dev/null || true)"
    fi
    case "$_os" in
        Darwin|darwin) printf '%s\n' "apple-darwin" ;;
        Linux|linux)   printf '%s\n' "unknown-linux-gnu" ;;
        *) die "Unsupported operating system: ${_os:-unknown}. Rivora binaries support macOS and Linux only." ;;
    esac
}

# Normalize uname -m → rust arch component
detect_arch() {
    _arch="${1:-}"
    if [ -z "$_arch" ]; then
        _arch="$(uname -m 2>/dev/null || true)"
    fi
    case "$_arch" in
        arm64|aarch64) printf '%s\n' "aarch64" ;;
        x86_64|amd64)  printf '%s\n' "x86_64" ;;
        *) die "Unsupported CPU architecture: ${_arch:-unknown}. Supported: aarch64, x86_64." ;;
    esac
}

# Build target triple: <arch>-<os>
detect_target() {
    _os_in="${1:-}"
    _arch_in="${2:-}"
    _os_part="$(detect_os "$_os_in")"
    _arch_part="$(detect_arch "$_arch_in")"
    printf '%s\n' "${_arch_part}-${_os_part}"
}

# ---------------------------------------------------------------------------
# Version handling
# ---------------------------------------------------------------------------

# Accept 0.9.1 or v0.9.1 → normalize to v0.9.1. Reject arbitrary tags.
normalize_version() {
    _raw="${1:-}"
    if [ -z "$_raw" ]; then
        die "Empty version."
    fi
    # Strip leading v if present, then re-add after validation
    _v="$_raw"
    case "$_v" in
        v*) _v="${_v#v}" ;;
    esac
    # Strict: MAJOR.MINOR.PATCH only (digits)
    case "$_v" in
        ''|*[!0-9.]*) die "Invalid version '${_raw}'. Expected vMAJOR.MINOR.PATCH (e.g. v0.9.1)." ;;
    esac
    # Must match N.N.N with only digits in each component
    _maj="${_v%%.*}"
    _rest="${_v#*.}"
    _min="${_rest%%.*}"
    _pat="${_rest#*.}"
    if [ "$_maj" = "$_v" ] || [ "$_min" = "$_rest" ] || [ -z "$_pat" ]; then
        die "Invalid version '${_raw}'. Expected vMAJOR.MINOR.PATCH (e.g. v0.9.1)."
    fi
    # Ensure no extra dots / empty components
    case "$_v" in
        *.*.*.*) die "Invalid version '${_raw}'. Expected vMAJOR.MINOR.PATCH (e.g. v0.9.1)." ;;
    esac
    case "$_maj" in ''|*[!0-9]*) die "Invalid version '${_raw}'." ;; esac
    case "$_min" in ''|*[!0-9]*) die "Invalid version '${_raw}'." ;; esac
    case "$_pat" in ''|*[!0-9]*) die "Invalid version '${_raw}'." ;; esac
    printf 'v%s\n' "$_v"
}

# Strip leading v → bare version for --version comparison
bare_version() {
    _t="${1#v}"
    printf '%s\n' "$_t"
}

# ---------------------------------------------------------------------------
# Install directory selection
# ---------------------------------------------------------------------------

# Priority:
# 1. RIVORA_INSTALL_DIR
# 2. A user-writable directory already on PATH (prefer ~/.local/bin, then ~/bin)
# 3. $HOME/.local/bin
# Never silently use root-owned dirs. Never invoke sudo.
select_install_dir() {
    if [ -n "${RIVORA_INSTALL_DIR:-}" ]; then
        printf '%s\n' "$RIVORA_INSTALL_DIR"
        return 0
    fi

    _home="${HOME:-}"
    if [ -z "$_home" ]; then
        die "HOME is not set; set RIVORA_INSTALL_DIR explicitly."
    fi

    # Prefer user-local bins that are already on PATH and writable (or creatable)
    for _candidate in "${_home}/.local/bin" "${_home}/bin"; do
        case ":${PATH}:" in
            *":${_candidate}:"*)
                if [ -d "$_candidate" ] && [ -w "$_candidate" ]; then
                    printf '%s\n' "$_candidate"
                    return 0
                fi
                if [ ! -e "$_candidate" ]; then
                    # Parent writable? We'll create it later.
                    _parent="$(dirname "$_candidate")"
                    if [ -w "$_parent" ] || [ ! -e "$_parent" ]; then
                        printf '%s\n' "$_candidate"
                        return 0
                    fi
                fi
                ;;
        esac
    done

    # Default fallback: ~/.local/bin (even if not on PATH yet)
    printf '%s\n' "${_home}/.local/bin"
}

path_contains() {
    _dir="$1"
    case ":${PATH}:" in
        *":${_dir}:"*) return 0 ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------------------
# HTTP helpers
# ---------------------------------------------------------------------------

http_get() {
    # http_get URL DEST
    _url="$1"
    _dest="$2"
    _attempt=1
    _max=3
    while [ "$_attempt" -le "$_max" ]; do
        if command -v curl >/dev/null 2>&1; then
            if curl \
                --proto '=https' \
                --tlsv1.2 \
                --silent \
                --show-error \
                --fail \
                --location \
                --connect-timeout 15 \
                --max-time 300 \
                --retry 2 \
                --retry-delay 1 \
                --output "$_dest" \
                "$_url"
            then
                return 0
            fi
        elif command -v wget >/dev/null 2>&1; then
            if wget \
                --https-only \
                --quiet \
                --timeout=30 \
                --tries=3 \
                --output-document="$_dest" \
                "$_url"
            then
                return 0
            fi
        else
            die "curl or wget is required to download Rivora."
        fi
        _attempt=$((_attempt + 1))
        if [ "$_attempt" -le "$_max" ]; then
            warn "Download failed (attempt $((_attempt - 1))/$_max); retrying..."
            sleep 1
        fi
    done
    die "Failed to download: ${_url}"
}

http_get_stdout() {
    # http_get_stdout URL → stdout
    _url="$1"
    if command -v curl >/dev/null 2>&1; then
        curl \
            --proto '=https' \
            --tlsv1.2 \
            --silent \
            --show-error \
            --fail \
            --location \
            --connect-timeout 15 \
            --max-time 60 \
            --retry 2 \
            --retry-delay 1 \
            "$_url"
    elif command -v wget >/dev/null 2>&1; then
        wget \
            --https-only \
            --quiet \
            --timeout=30 \
            --tries=3 \
            --output-document=- \
            "$_url"
    else
        die "curl or wget is required to download Rivora."
    fi
}

# ---------------------------------------------------------------------------
# Release resolution
# ---------------------------------------------------------------------------

resolve_latest_version() {
    # Uses GitHub Releases API; ignores draft and prerelease.
    _api_url="${GITHUB_API}/repos/${GITHUB_REPO}/releases/latest"
    _json="$(http_get_stdout "$_api_url" 2>/dev/null)" || die "Failed to resolve latest release from GitHub (${_api_url})."
    # Extract tag_name without requiring jq
    _tag="$(printf '%s\n' "$_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
    if [ -z "$_tag" ]; then
        die "Could not parse latest release tag from GitHub API."
    fi
    normalize_version "$_tag"
}

# ---------------------------------------------------------------------------
# Checksums
# ---------------------------------------------------------------------------

sha256_file() {
    _file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$_file" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$_file" | awk '{print $1}'
    else
        die "Neither sha256sum nor shasum is available; cannot verify download integrity."
    fi
}

verify_checksum() {
    # verify_checksum ARCHIVE_PATH ARCHIVE_BASENAME SUMS_PATH
    _archive="$1"
    _basename="$2"
    _sums="$3"

    if [ ! -f "$_sums" ]; then
        die "Checksum file missing."
    fi
    if [ ! -f "$_archive" ]; then
        die "Archive missing for checksum verification."
    fi

    _expected=""
    # Match lines like: <hex>  <filename>  or  <hex> *filename
    while IFS= read -r _line || [ -n "$_line" ]; do
        # Skip empty / comments
        case "$_line" in
            ''|\#*) continue ;;
        esac
        _sum="$(printf '%s\n' "$_line" | awk '{print $1}')"
        _name="$(printf '%s\n' "$_line" | awk '{print $2}')"
        # Strip optional leading *
        case "$_name" in
            \**) _name="${_name#\*}" ;;
        esac
        # Also allow basename-only match when path-prefixed
        _name_base="$(basename "$_name")"
        if [ "$_name" = "$_basename" ] || [ "$_name_base" = "$_basename" ]; then
            _expected="$_sum"
            break
        fi
    done < "$_sums"

    if [ -z "$_expected" ]; then
        die "No checksum entry found for ${_basename} in SHA256SUMS."
    fi

    # Must be exactly 64 hexadecimal characters
    case "$_expected" in
        *[!0-9a-fA-F]*) die "Malformed checksum for ${_basename}." ;;
    esac
    _len="$(printf '%s' "$_expected" | wc -c | tr -d ' ')"
    if [ "$_len" != "64" ]; then
        die "Malformed checksum for ${_basename} (expected 64 hex characters)."
    fi

    _actual="$(sha256_file "$_archive")"
    if [ "$_actual" != "$_expected" ]; then
        die "SHA-256 mismatch for ${_basename}.
  expected: ${_expected}
  actual:   ${_actual}
Refusing to install an unverified binary."
    fi
}

# ---------------------------------------------------------------------------
# Extraction and installation
# ---------------------------------------------------------------------------

# Reject path traversal in tar members (no absolute paths, no ..).
# Avoid pipelines so die() exits this shell (not a subshell).
assert_safe_tar() {
    _archive="$1"
    if ! command -v tar >/dev/null 2>&1; then
        die "tar is required to extract Rivora archives."
    fi
    _list_file="${TMPDIR_INSTALL:-${TMPDIR:-/tmp}}/rivora-tar-list.$$"
    if ! tar -tzf "$_archive" > "$_list_file" 2>/dev/null; then
        rm -f "$_list_file"
        die "Could not list archive contents (corrupt or unreadable archive)."
    fi
    while IFS= read -r _member || [ -n "${_member:-}" ]; do
        [ -n "${_member:-}" ] || continue
        case "$_member" in
            /*)
                rm -f "$_list_file"
                die "Archive contains absolute path: ${_member}"
                ;;
            *../*|*/../*|../*|*/..)
                rm -f "$_list_file"
                die "Archive contains path traversal: ${_member}"
                ;;
        esac
    done < "$_list_file"
    rm -f "$_list_file"
}

install_from_archive() {
    # install_from_archive ARCHIVE DEST_DIR
    _archive="$1"
    _dest="$2"
    _extract="$3"

    assert_safe_tar "$_archive"

    mkdir -p "$_extract"
    tar -xzf "$_archive" -C "$_extract"

    # Expected layout: rivora binary at root or in a single top-level dir
    _bin=""
    if [ -f "${_extract}/rivora" ]; then
        _bin="${_extract}/rivora"
    else
        # Single top-level directory
        for _d in "${_extract}"/*; do
            if [ -f "${_d}/rivora" ]; then
                _bin="${_d}/rivora"
                break
            fi
        done
    fi

    if [ -z "$_bin" ] || [ ! -f "$_bin" ]; then
        die "Archive does not contain the expected 'rivora' binary."
    fi
    if [ ! -x "$_bin" ]; then
        chmod +x "$_bin" || die "Could not mark rivora executable."
    fi

    mkdir -p "$_dest"
    if [ ! -w "$_dest" ]; then
        die "Install directory is not writable: ${_dest}
Set RIVORA_INSTALL_DIR to a user-writable path (e.g. \$HOME/.local/bin).
This installer never uses sudo."
    fi

    # Optional workspace binary
    _ws=""
    if [ -f "$(dirname "$_bin")/rivora-workspace" ]; then
        _ws="$(dirname "$_bin")/rivora-workspace"
        if [ ! -x "$_ws" ]; then
            chmod +x "$_ws" || true
        fi
    fi

    # Atomic replace: install to temp name then mv
    _tmp_bin="${_dest}/.rivora.${$}.tmp"
    cp "$_bin" "$_tmp_bin"
    chmod 755 "$_tmp_bin"
    mv -f "$_tmp_bin" "${_dest}/rivora"

    if [ -n "$_ws" ] && [ -f "$_ws" ]; then
        _tmp_ws="${_dest}/.rivora-workspace.${$}.tmp"
        cp "$_ws" "$_tmp_ws"
        chmod 755 "$_tmp_ws"
        mv -f "$_tmp_ws" "${_dest}/rivora-workspace"
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    # Allow RIVORA_TEST_SKIP_MAIN for unit tests that source this file
    if [ "${RIVORA_TEST_SKIP_MAIN:-}" = "1" ]; then
        return 0
    fi

    TARGET="$(detect_target)"
    INSTALL_DIR="$(select_install_dir)"

    if [ -n "${RIVORA_VERSION:-}" ]; then
        VERSION="$(normalize_version "$RIVORA_VERSION")"
    else
        info "Resolving latest stable release..."
        VERSION="$(resolve_latest_version)"
    fi

    BARE="$(bare_version "$VERSION")"
    ARCHIVE_NAME="rivora-${VERSION}-${TARGET}.tar.gz"
    DOWNLOAD_URL="${DOWNLOAD_BASE}/${VERSION}/${ARCHIVE_NAME}"
    SUMS_URL="${DOWNLOAD_BASE}/${VERSION}/SHA256SUMS"

    info "Installing Rivora ${VERSION}"
    info "Platform: ${TARGET}"
    info "Install directory: ${INSTALL_DIR}"

    # Note existing binary if present
    if [ -x "${INSTALL_DIR}/rivora" ]; then
        _prev="$("${INSTALL_DIR}/rivora" --version 2>/dev/null || true)"
        if [ -n "$_prev" ]; then
            info "Replacing existing: ${_prev}"
        fi
    fi

    # Secure temp directory with cleanup
    TMPDIR_INSTALL="$(mktemp -d "${TMPDIR:-/tmp}/rivora-install.XXXXXX")"
    cleanup() {
        rm -rf "$TMPDIR_INSTALL"
    }
    trap cleanup EXIT INT HUP TERM

    ARCHIVE_PATH="${TMPDIR_INSTALL}/${ARCHIVE_NAME}"
    SUMS_PATH="${TMPDIR_INSTALL}/SHA256SUMS"
    EXTRACT_PATH="${TMPDIR_INSTALL}/extract"

    info "Downloading..."
    http_get "$DOWNLOAD_URL" "$ARCHIVE_PATH"
    http_get "$SUMS_URL" "$SUMS_PATH"

    info "Verifying SHA-256..."
    verify_checksum "$ARCHIVE_PATH" "$ARCHIVE_NAME" "$SUMS_PATH"

    info "Extracting and installing..."
    install_from_archive "$ARCHIVE_PATH" "$INSTALL_DIR" "$EXTRACT_PATH"

    # Verify installed version
    INSTALLED="$("${INSTALL_DIR}/rivora" --version 2>/dev/null || true)"
    case "$INSTALLED" in
        *"${BARE}"*)
            info "Installed rivora ${BARE}"
            ;;
        *)
            die "Installed binary version mismatch.
  expected: rivora ${BARE}
  got:      ${INSTALLED:-<no output>}
Binary was not left partially installed without verification — check ${INSTALL_DIR}/rivora"
            ;;
    esac

    if [ -x "${INSTALL_DIR}/rivora-workspace" ]; then
        _ws_ver="$("${INSTALL_DIR}/rivora-workspace" --version 2>/dev/null || true)"
        info "Installed rivora-workspace (${_ws_ver:-ok})"
    fi

    if ! path_contains "$INSTALL_DIR"; then
        info ""
        info "Note: ${INSTALL_DIR} is not on your PATH."
        info "Add it for the current shell:"
        info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        info "Or add that line to your shell profile (~/.bashrc, ~/.zshrc, etc.)."
        info "This installer does not modify shell profiles."
    fi

    info ""
    info "Success. Try:"
    info "  rivora --version"
    info "  rivora --help"
}

main "$@"
