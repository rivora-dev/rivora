# Backup and Restore Guide (v0.9)

## Backup

```bash
rivora --data-dir .rivora/data doctor backup /safe/rivora-backup
```

Rules:

- Destination must not already exist
- Destination must not be inside the store root
- Live `.rivora.lock` is excluded

## Restore

1. Stop all Rivora processes using the store
2. Replace or point `--data-dir` at the backup:

```bash
rivora --data-dir /safe/rivora-backup doctor health
rivora --data-dir /safe/rivora-backup investigation list
```

Or copy backup contents over the original data directory after a final backup of the broken state.

## Verification after restore

```bash
rivora doctor health --json
rivora doctor rebuild-indexes
```

Confirm investigation counts, sample recall, and capability coverage still look correct.
