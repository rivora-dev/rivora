# Store Recovery Guide (v0.9)

## Safe recovery principles

1. **Never silently rewrite** append-only Memory or historical receipts.
2. Prefer **backup first**.
3. Isolate corrupt files; keep healthy siblings.
4. Rebuild **derived** indexes only.

## Recovery playbook

### 1. Health snapshot

```bash
rivora doctor health --json
rivora doctor export --out /tmp/diag.json
```

### 2. Backup

```bash
rivora doctor backup /safe/rivora-backup-$(date +%Y%m%d)
```

### 3. Stale lock

```bash
rivora doctor recover-lock
```

### 4. Corrupt JSON

1. From health diagnostics, note paths under `investigations/{id}/...`
2. Move corrupt files to a quarantine directory outside the store
3. Confirm healthy records still list
4. `rivora doctor rebuild-indexes`

### 5. Prior-version stores (0.1–0.8)

Open with v0.9. Missing directories are empty (lazy). `store.json` is created on first open (schema v1). History is preserved.

### 6. Interrupted writes

Orphan `*.tmp` files are cleaned on open. Canonical paths use atomic rename / exclusive create.

## When to escalate

- Schema version greater than this build supports
- Suspected filesystem corruption beyond single JSON files
- Need forensic preservation — stop writing and archive the whole data dir
