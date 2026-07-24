#!/usr/bin/env sh
# Deterministic installer unit tests (no live GitHub dependency).
# shellcheck shell=sh disable=SC2039,SC1091

set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
INSTALL_SH="${ROOT}/scripts/install.sh"
FAILS=0
PASSES=0

pass() { PASSES=$((PASSES + 1)); printf '  PASS  %s\n' "$1"; }
fail() { FAILS=$((FAILS + 1)); printf '  FAIL  %s\n' "$1"; }

assert_eq() {
    name="$1"
    expected="$2"
    actual="$3"
    if [ "$expected" = "$actual" ]; then
        pass "$name"
    else
        fail "$name"
        printf '    expected: %s\n    actual:   %s\n' "$expected" "$actual"
    fi
}

assert_contains() {
    name="$1"
    needle="$2"
    haystack="$3"
    case "$haystack" in
        *"$needle"*) pass "$name" ;;
        *) fail "$name (missing $needle)" ;;
    esac
}

assert_fails() {
    name="$1"
    shift
    out=""
    rc=0
    out="$("$@" 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ]; then
        pass "$name"
    else
        fail "$name (expected non-zero exit)"
        printf '    output: %s\n' "$out"
    fi
}

assert_ok() {
    name="$1"
    shift
    out=""
    rc=0
    out="$("$@" 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ]; then
        pass "$name"
    else
        fail "$name (exit $rc)"
        printf '    output: %s\n' "$out"
    fi
}

# Source installer functions without running main
RIVORA_TEST_SKIP_MAIN=1
# shellcheck source=../install.sh
. "$INSTALL_SH"

printf '== Detection ==\n'
assert_eq "macOS ARM64 target" "aarch64-apple-darwin" "$(detect_target Darwin arm64)"
assert_eq "macOS x86_64 target" "x86_64-apple-darwin" "$(detect_target Darwin x86_64)"
assert_eq "Linux x86_64 target" "x86_64-unknown-linux-gnu" "$(detect_target Linux x86_64)"
assert_eq "Linux ARM64 target" "aarch64-unknown-linux-gnu" "$(detect_target Linux aarch64)"
assert_eq "arch aarch64 alias" "aarch64" "$(detect_arch aarch64)"
assert_eq "arch amd64 alias" "x86_64" "$(detect_arch amd64)"
assert_fails "unsupported OS" detect_os Windows
assert_fails "unsupported arch" detect_arch riscv64

printf '== Version handling ==\n'
assert_eq "normalize v0.9.1" "v0.9.1" "$(normalize_version v0.9.1)"
assert_eq "normalize 0.9.1" "v0.9.1" "$(normalize_version 0.9.1)"
assert_eq "bare version" "0.9.1" "$(bare_version v0.9.1)"
assert_fails "reject malformed tag" normalize_version latest
assert_fails "reject empty version" normalize_version ""
assert_fails "reject path-like tag" normalize_version "../evil"
assert_fails "reject prerelease-looking" normalize_version v0.9.1-rc1
assert_fails "reject four components" normalize_version v1.2.3.4

printf '== Install directory ==\n'
TEST_HOME="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-home.XXXXXX")"
TEST_CUSTOM="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-custom.XXXXXX")"
export HOME="$TEST_HOME"
unset RIVORA_INSTALL_DIR || true
assert_eq "default install dir" "${TEST_HOME}/.local/bin" "$(select_install_dir)"
export RIVORA_INSTALL_DIR="$TEST_CUSTOM"
assert_eq "custom install dir" "$TEST_CUSTOM" "$(select_install_dir)"
unset RIVORA_INSTALL_DIR || true

printf '== Checksum verification ==\n'
TEST_CK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-cksum.XXXXXX")"
printf 'hello rivora\n' > "${TEST_CK_DIR}/payload.bin"
TEST_GOOD_SUM="$(sha256_file "${TEST_CK_DIR}/payload.bin")"
printf '%s  rivora-v0.9.1-test.tar.gz\n' "$TEST_GOOD_SUM" > "${TEST_CK_DIR}/SHA256SUMS"
cp "${TEST_CK_DIR}/payload.bin" "${TEST_CK_DIR}/rivora-v0.9.1-test.tar.gz"

if verify_checksum "${TEST_CK_DIR}/rivora-v0.9.1-test.tar.gz" "rivora-v0.9.1-test.tar.gz" "${TEST_CK_DIR}/SHA256SUMS"; then
    pass "checksum success"
else
    fail "checksum success"
fi

printf 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef  rivora-v0.9.1-test.tar.gz\n' > "${TEST_CK_DIR}/bad.sums"
assert_fails "checksum mismatch" verify_checksum "${TEST_CK_DIR}/rivora-v0.9.1-test.tar.gz" "rivora-v0.9.1-test.tar.gz" "${TEST_CK_DIR}/bad.sums"

printf '%s  other-file.tar.gz\n' "$TEST_GOOD_SUM" > "${TEST_CK_DIR}/missing.sums"
assert_fails "missing checksum entry" verify_checksum "${TEST_CK_DIR}/rivora-v0.9.1-test.tar.gz" "rivora-v0.9.1-test.tar.gz" "${TEST_CK_DIR}/missing.sums"

printf 'not-a-hash  rivora-v0.9.1-test.tar.gz\n' > "${TEST_CK_DIR}/malformed.sums"
assert_fails "malformed checksum" verify_checksum "${TEST_CK_DIR}/rivora-v0.9.1-test.tar.gz" "rivora-v0.9.1-test.tar.gz" "${TEST_CK_DIR}/malformed.sums"

printf '== Security / static checks ==\n'
if grep -E 'insecure|--no-check-certificate|CURL_INSECURE' "$INSTALL_SH" >/dev/null 2>&1; then
    fail "found TLS-disable flags"
else
    pass "no TLS disable"
fi
# Fail only if sudo is invoked as a command (allow documentation mentions)
if grep -E '(^|[[:space:]])sudo[[:space:]]+[^[:space:]]' "$INSTALL_SH" >/dev/null 2>&1; then
    fail "found sudo usage"
else
    pass "no sudo"
fi
if grep -E '(>>|>|tee).*\.(bashrc|zshrc|profile)' "$INSTALL_SH" >/dev/null 2>&1; then
    fail "mutates shell profile"
else
    pass "no profile mutation"
fi
# verify_checksum definition should appear before install_from_archive definition
v_line="$(grep -n '^verify_checksum()' "$INSTALL_SH" | head -1 | cut -d: -f1)"
i_line="$(grep -n '^install_from_archive()' "$INSTALL_SH" | head -1 | cut -d: -f1)"
if [ -n "$v_line" ] && [ -n "$i_line" ] && [ "$v_line" -lt "$i_line" ]; then
    pass "verify defined before install"
else
    fail "verify not before install"
fi
# main flow order
main_v="$(grep -n 'verify_checksum "\$ARCHIVE_PATH"' "$INSTALL_SH" | head -1 | cut -d: -f1)"
main_i="$(grep -n 'install_from_archive "\$ARCHIVE_PATH"' "$INSTALL_SH" | head -1 | cut -d: -f1)"
if [ -n "$main_v" ] && [ -n "$main_i" ] && [ "$main_v" -lt "$main_i" ]; then
    pass "verify before install in main"
else
    fail "verify not before install in main"
fi

printf '== Path traversal rejection ==\n'
TEST_TRAV="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-trav.XXXXXX")"
if tar --help 2>&1 | grep -q absolute-names; then
    abs_archive="${TEST_TRAV}/abs.tar.gz"
    tar --absolute-names -czf "$abs_archive" /etc/hosts 2>/dev/null || true
    if [ -f "$abs_archive" ] && tar -tzf "$abs_archive" 2>/dev/null | head -1 | grep -q '^/'; then
        assert_fails "absolute path rejected" assert_safe_tar "$abs_archive"
    else
        pass "absolute path (skipped)"
    fi
else
    pass "absolute path (bsd tar skip)"
fi

printf '== Atomic install + version verify (local fixture) ==\n'
TEST_INSTALL="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-install.XXXXXX")"
mkdir -p "${TEST_INSTALL}/pkg"
cat > "${TEST_INSTALL}/pkg/rivora" <<'EOF'
#!/bin/sh
echo "rivora 0.9.2"
EOF
chmod +x "${TEST_INSTALL}/pkg/rivora"
cp "${ROOT}/LICENSE" "${TEST_INSTALL}/pkg/LICENSE"
cp "${ROOT}/README.md" "${TEST_INSTALL}/pkg/README.md"
tar -czf "${TEST_INSTALL}/rivora-v0.9.2-test.tar.gz" -C "${TEST_INSTALL}/pkg" rivora LICENSE README.md
mkdir -p "${TEST_INSTALL}/dest"
install_from_archive "${TEST_INSTALL}/rivora-v0.9.2-test.tar.gz" "${TEST_INSTALL}/dest" "${TEST_INSTALL}/extract"
assert_eq "installed binary exists" "1" "$([ -x "${TEST_INSTALL}/dest/rivora" ] && echo 1 || echo 0)"
assert_contains "installed version" "0.9.2" "$("${TEST_INSTALL}/dest/rivora" --version)"

mkdir -p "${TEST_INSTALL}/nowrite"
chmod 555 "${TEST_INSTALL}/nowrite"
assert_fails "non-writable install dir" install_from_archive "${TEST_INSTALL}/rivora-v0.9.2-test.tar.gz" "${TEST_INSTALL}/nowrite" "${TEST_INSTALL}/extract2"
chmod 755 "${TEST_INSTALL}/nowrite" 2>/dev/null || true

printf '== Packaging script ==\n'
if [ -f "${ROOT}/target/release/rivora" ]; then
    TEST_OUTPKG="$(mktemp -d "${TMPDIR:-/tmp}/rivora-test-pkg.XXXXXX")"
    binver="$("${ROOT}/target/release/rivora" --version 2>/dev/null || true)"
    case "$binver" in
        *0.9.2*)
            chmod +x "${ROOT}/scripts/package-release.sh"
            assert_ok "package-release" "${ROOT}/scripts/package-release.sh" v0.9.2 aarch64-apple-darwin "${ROOT}/target/release" "$TEST_OUTPKG"
            assert_eq "archive present" "1" "$([ -f "${TEST_OUTPKG}/rivora-v0.9.2-aarch64-apple-darwin.tar.gz" ] && echo 1 || echo 0)"
            ;;
        *)
            pass "package-release skip (binary not 0.9.2 yet)"
            ;;
    esac
else
    pass "package-release skip (no release binary)"
fi

rm -rf "$TEST_HOME" "$TEST_CUSTOM" "$TEST_CK_DIR" "$TEST_TRAV" "$TEST_INSTALL" "${TEST_OUTPKG:-}" 2>/dev/null || true

printf '\nResults: %s passed, %s failed\n' "$PASSES" "$FAILS"
if [ "$FAILS" -ne 0 ]; then
    exit 1
fi
exit 0
