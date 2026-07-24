# Install Rivora

## Quick install (recommended)

```bash
curl -fsSL https://rivora.dev/install | sh
```

This detects your OS and architecture, downloads the latest stable release
from [GitHub Releases](https://github.com/rivora-dev/rivora/releases), verifies
SHA-256 checksums, and installs into a user-writable directory (default:
`$HOME/.local/bin`).

It never runs `sudo` and never modifies shell profiles.

## Supported platforms

| Platform | Target triple |
|----------|---------------|
| macOS Apple Silicon | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Linux x86_64 (glibc) | `x86_64-unknown-linux-gnu` |
| Linux ARM64 (glibc) | `aarch64-unknown-linux-gnu` |

Windows is not supported by the shell installer in this release.

## Options

### Explicit version

```bash
curl -fsSL https://rivora.dev/install | RIVORA_VERSION=v0.9.1 sh
# also accepts:
curl -fsSL https://rivora.dev/install | RIVORA_VERSION=0.9.1 sh
```

### Custom install directory

```bash
curl -fsSL https://rivora.dev/install | RIVORA_INSTALL_DIR=$HOME/bin sh
```

Priority:

1. `RIVORA_INSTALL_DIR`
2. A user-writable directory already on `PATH` (`~/.local/bin`, then `~/bin`)
3. `$HOME/.local/bin`

### PATH

If the install directory is not on your `PATH`, the installer prints exact
commands. Example:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Add that line to your shell profile yourself if you want it permanent.

### Inspect before execute

```bash
curl -fsSL https://rivora.dev/install -o install-rivora.sh
less install-rivora.sh
sh install-rivora.sh
```

## What gets installed

| Binary | Role |
|--------|------|
| `rivora` | CLI (one-shot and scripting) |
| `rivora-workspace` | Interactive terminal Workspace |

## Verify

```bash
rivora --version
rivora-workspace --version
rivora --help
```

## Uninstall

```bash
rm -f "$(command -v rivora)" "$(command -v rivora-workspace)"
```

## Manual install from GitHub Releases

1. Open <https://github.com/rivora-dev/rivora/releases>
2. Download the archive for your platform and `SHA256SUMS`
3. Verify checksums:

   ```bash
   sha256sum -c SHA256SUMS
   # or on macOS:
   shasum -a 256 -c SHA256SUMS
   ```

4. Extract and install:

   ```bash
   tar -xzf rivora-v0.9.1-<target>.tar.gz
   mkdir -p "$HOME/.local/bin"
   mv rivora rivora-workspace "$HOME/.local/bin/"
   ```

## Build from source

Requirements: Rust 1.75+ (edition 2021).

```bash
git clone https://github.com/rivora-dev/rivora.git
cd rivora
git checkout v0.9.1   # or main
cargo build --workspace --release
./target/release/rivora --version
./target/release/rivora-workspace --version
```

Binaries:

- `target/release/rivora`
- `target/release/rivora-workspace`

## Troubleshooting

| Symptom | What to try |
|---------|-------------|
| `Unsupported operating system` | Use macOS or Linux; Windows is not supported yet |
| `Unsupported CPU architecture` | Need aarch64 or x86_64 |
| `SHA-256 mismatch` | Re-download; do not install; report if it persists |
| `Install directory is not writable` | Set `RIVORA_INSTALL_DIR` to a user-owned path |
| `not on your PATH` | Export `PATH` as printed by the installer |
| `curl: command not found` | Install `curl` or `wget` |
| `Neither sha256sum nor shasum` | Install coreutils / use a platform with shasum |
| Network / API failures | Check connectivity to `github.com` and `api.github.com` |

## Security

- Downloads use HTTPS with TLS verification (never disabled).
- The selected archive is verified against `SHA256SUMS` before extraction.
- No implicit privilege escalation.
- Prefer reviewing the installer script before piping to a shell.

See also: [DISTRIBUTION.md](./DISTRIBUTION.md), [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).
