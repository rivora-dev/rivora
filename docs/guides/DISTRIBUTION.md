# Rivora Binary Distribution

**Release:** v0.9.1 — Binary Distribution and Installer

This guide defines the public binary distribution contract for Rivora.

## Installation (primary)

```bash
curl -fsSL https://rivora.dev/install | sh
```

GitHub Releases remain the canonical artifact store:

```text
https://github.com/rivora-dev/rivora/releases
```

`https://rivora.dev/install` is the stable user-facing installation contract.
It serves the tracked installer at `scripts/install.sh` via a narrow Cloudflare
Worker route and does not replace the marketing website origin.

## Asset contract

For release tag `vX.Y.Z`, GitHub Release assets are:

| Asset | Description |
|-------|-------------|
| `rivora-vX.Y.Z-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `rivora-vX.Y.Z-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `rivora-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 (glibc) |
| `rivora-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 (glibc) |
| `SHA256SUMS` | SHA-256 digests for every archive above |

### Archive contents

Each archive contains only intended distributables:

```text
rivora
rivora-workspace   # interactive Workspace binary
LICENSE
README.md
```

Archives must **not** contain source trees, `target/`, credentials, `.env`,
local stores, internal docs, benchmarks, or Git metadata.

### Naming rules

- Version in the filename matches the Git tag, including the leading `v`.
- Target triples are Rust target triples for the supported platforms.
- `SHA256SUMS` uses the standard `sha256sum` text format:
  `<64-hex>  <filename>`.

## Supported platforms (v0.9.1)

| Target | OS | Arch | Notes |
|--------|----|------|-------|
| `aarch64-apple-darwin` | macOS | ARM64 | Native GitHub `macos-14` runner |
| `x86_64-apple-darwin` | macOS | x86_64 | Cross-built on Apple Silicon runner |
| `x86_64-unknown-linux-gnu` | Linux | x86_64 | Native `ubuntu-latest` |
| `aarch64-unknown-linux-gnu` | Linux | ARM64 | Native `ubuntu-24.04-arm` |

Windows is **not** supported in this release. A PowerShell installer
(`install.ps1`) is out of scope for v0.9.1.

## Release pipeline

Tag push (`v*`) triggers `.github/workflows/release.yml`:

1. Confirm tag matches workspace `Cargo.toml` version
2. `cargo fmt --check`, Clippy (`-D warnings`), full test suite
3. Build release binaries per target matrix
4. Package archives via `scripts/package-release.sh`
5. Generate `SHA256SUMS`
6. Refuse silent overwrite when an existing asset has a different checksum
7. Upload assets to the matching GitHub Release

Permissions: `contents: write` only.

Manual dry-run (no upload):

```text
Actions → Release → Run workflow → dry_run = true
```

## Installer behavior

Source: `scripts/install.sh`

| Concern | Behavior |
|---------|----------|
| OS | Darwin → `apple-darwin`, Linux → `unknown-linux-gnu` |
| Arch | `arm64`/`aarch64` → `aarch64`, `x86_64`/`amd64` → `x86_64` |
| Version | Latest stable (non-draft, non-prerelease) by default |
| Override | `RIVORA_VERSION=v0.9.1` or `0.9.1` |
| Install dir | `RIVORA_INSTALL_DIR` → writable PATH dir → `$HOME/.local/bin` |
| Integrity | Download `SHA256SUMS`, verify selected archive only |
| Privilege | Never invokes `sudo`; never writes shell profiles |
| Transport | HTTPS only; TLS verification always enabled |

### Examples

```bash
# Latest stable
curl -fsSL https://rivora.dev/install | sh

# Explicit version
curl -fsSL https://rivora.dev/install | RIVORA_VERSION=v0.9.1 sh

# Custom directory
curl -fsSL https://rivora.dev/install | RIVORA_INSTALL_DIR=$HOME/bin sh

# Inspect before execute
curl -fsSL https://rivora.dev/install -o install-rivora.sh
less install-rivora.sh
sh install-rivora.sh
```

## Cloudflare endpoint

Worker: `rivora-install` (`distribution/install-worker/`)

| Method | Result |
|--------|--------|
| `GET /install` | Installer script body |
| `HEAD /install` | Same headers, empty body |
| other | `405 Method Not Allowed` |

Headers:

```http
Content-Type: text/x-shellscript; charset=utf-8
Cache-Control: public, max-age=300
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
```

The script is embedded at Worker build time from `scripts/install.sh`.

## Manual installation from GitHub

1. Download the archive for your platform and `SHA256SUMS` from the release page.
2. Verify: `sha256sum -c SHA256SUMS` (or `shasum -a 256` against the listed digest).
3. Extract: `tar -xzf rivora-vX.Y.Z-<target>.tar.gz`
4. Move `rivora` (and optionally `rivora-workspace`) onto your `PATH`.

## Source build fallback

```bash
git clone https://github.com/rivora-dev/rivora.git
cd rivora
git checkout v0.9.1
cargo build --workspace --release
./target/release/rivora --version
```

## Uninstall

```bash
rm -f "$(command -v rivora)" "$(command -v rivora-workspace)"
# or, if installed to a custom dir:
rm -f "$RIVORA_INSTALL_DIR/rivora" "$RIVORA_INSTALL_DIR/rivora-workspace"
```

## Security notes

- Install only after SHA-256 verification succeeds.
- The installer does not use `sudo` and does not modify shell profiles.
- Prefer inspecting the script (`curl … -o` + review) before piping to `sh`.
- Report security issues according to project security documentation.

## Related files

| Path | Role |
|------|------|
| `scripts/install.sh` | Canonical installer |
| `scripts/package-release.sh` | Per-target packaging |
| `scripts/tests/install.test.sh` | Installer unit tests |
| `.github/workflows/release.yml` | Tag-triggered build/publish |
| `distribution/install-worker/` | Cloudflare `/install` Worker |
| `docs/guides/INSTALL.md` | End-user install guide |
