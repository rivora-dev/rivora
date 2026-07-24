# Troubleshooting Guide (v0.9+)

## Installer / binary install

**Primary install:** `curl -fsSL https://rivora.dev/install | sh`

| Symptom | Fix |
|---------|-----|
| Unsupported OS/arch | macOS or Linux on aarch64/x86_64 only; no Windows installer yet |
| Checksum mismatch | Do not install; re-download; report if it persists |
| Not writable install dir | `RIVORA_INSTALL_DIR=$HOME/.local/bin` (never uses sudo) |
| Binary not found after install | Add install dir to `PATH` (installer prints the exact line) |
| Want a specific version | `RIVORA_VERSION=v0.10.0` |
| Prefer manual install | GitHub Releases archives + `SHA256SUMS` — see `docs/guides/INSTALL.md` |
| `rivora` prints help and exits (pre-v0.10.0) | Upgrade to ≥0.10.0; bare `rivora` opens the Workspace |
| `interactive Workspace requires a terminal` | Use a TTY for the Workspace, or a CLI subcommand in scripts/CI |

## Store lock conflict

**Symptom:** `store lock conflict` / exit code `12`

**Cause:** Another Rivora CLI or Workspace process holds the store.

**Fix:**

1. Close the other process.
2. If it crashed: `rivora doctor recover-lock`
3. Confirm: `rivora doctor health`

## Schema mismatch

**Symptom:** exit code `11` / `schema mismatch`

**Cause:** Store was written by a newer Rivora than this binary.

**Fix:** Upgrade Rivora; do not downgrade over a newer schema.

## Corrupt records

**Symptom:** health shows `corrupt_records`; some listings miss entries

**Fix:**

1. `rivora doctor health --json` — identify paths
2. Backup: `rivora doctor backup /safe/backup`
3. Move corrupt files aside; do not rewrite history in place
4. Rebuild indexes: `rivora doctor rebuild-indexes`

## Partial completion

**Symptom:** exit code `6` / `partial completion`

**Meaning:** Some stages finished; overall work is incomplete. **Not** full success.

Inspect lifecycle / execution traces and resume with the same idempotency key where the contract allows.

## Connector timeouts / rate limits

**Symptom:** exit codes `9` (timeout) or `7` (provider) / `rate limited`

**Fix:** Retry later with the same idempotency key for safe observe paths; do not bypass approval for mutations.

## Oversized payload

**Symptom:** exit code `2` / `payload too large`

**Fix:** Reduce Observation payload under 1 MiB; use fixtures or paginated connector collection.

## CLI always exits 1 for every error (pre-v0.9)

Upgrade to v0.9 for stable exit codes (`rivora doctor exit-codes`).
