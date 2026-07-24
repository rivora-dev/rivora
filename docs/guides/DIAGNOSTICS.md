# Diagnostics Guide (v0.9)

All diagnostics are **local**. No remote telemetry is sent.

## Surfaces

| Command | Purpose |
|---------|---------|
| `rivora doctor health` | Store integrity, counts, lock, schema |
| `rivora doctor export` | Sanitized JSON bundle |
| `rivora doctor budgets` | Performance budgets |
| `rivora doctor envelope` | Operating envelope |
| `rivora doctor replay-contracts` | Idempotency contracts |
| `rivora capability coverage` | First-party Capability/Connector coverage |
| `rivora connector status` | Connector configuration (no secrets) |

## Health fields

- `schema_version`, `lock_held`, object counts
- `corrupt_records[]` with path + sanitized error
- `disk_bytes`, `migration_status`
- `supported_prior_versions`

## Export contents

- Health report
- Medium operating envelope
- Replay contracts
- Performance budgets
- Rivora version

Review before sharing; paths may still identify local projects.
