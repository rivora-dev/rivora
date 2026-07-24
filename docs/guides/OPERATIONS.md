# Operations Guide (v0.9)

Local-first operations for Rivora stores, locks, backups, and diagnostics.

## Data directory

Default: `.rivora/data`

```bash
rivora --data-dir /path/to/store doctor health
```

Layout (additive across versions):

```text
store.json                 # schema manifest (v0.9+)
.rivora.lock               # exclusive process lock
investigations/{id}/...
graph/relationships/
learning/patterns/
```

## Health and diagnostics

```bash
rivora doctor health
rivora doctor health --json
rivora doctor export --out /tmp/rivora-diag.json
rivora doctor exit-codes
rivora doctor replay-contracts
```

Health reports corrupt-record counts without failing healthy sibling reads.
Exports are local-only and sanitized (no remote telemetry).

## Locks

- Cross-process opens are exclusive.
- Same-process re-open is refcounted (CLI internals / tests).
- Stale locks (dead process or older than 300s) can be recovered:

```bash
rivora doctor recover-lock
```

If a live process holds the lock, recovery is refused.

## Backup and restore

```bash
rivora doctor backup /safe/path/rivora-backup
```

- Destination must **not** be inside the store root.
- Live lock file is excluded.
- Restore by pointing `--data-dir` at the backup (or copying it back).

## Index rebuild

Observation idempotency indexes are derived and rebuildable:

```bash
rivora doctor rebuild-indexes
```

## Interruption and recovery

- Append-only Memory and exclusive creates reduce partial-write risk.
- Durable writes use unique temps + `fsync` before rename.
- Corrupt JSON siblings are isolated for core history listings.
- Do **not** manually edit JSON history to “fix” outcomes; append corrections instead.

## Connector operations

- Observation connectors use 5s connect / 30s request timeouts.
- Rate limits and auth failures surface as explicit errors.
- Secrets are redacted from payloads and error strings.

## Security notes

- Never commit tokens or `.rivora/data` with secrets.
- Diagnostic export must be reviewed before sharing.
- Execution still requires plan → policy → exact-revision approval → confirmation.
