# CLI Exit Code Contract (v0.9)

| Code | Name | Meaning |
|-----:|------|---------|
| 0 | success | Full success (including intentional full no-op replay) |
| 1 | internal | Unexpected internal / storage failure |
| 2 | validation | Validation / usage / payload limits |
| 3 | not_found | Investigation or object not found |
| 4 | unsupported | Unsupported operation |
| 5 | blocked | Precondition / conflict / not allowed |
| 6 | partial | Partial completion — **not** full success |
| 7 | provider_failure | External provider failure or rate limit |
| 8 | auth_failure | Provider authentication failure |
| 9 | timeout | Operation timed out |
| 10 | corrupt_store | Corrupt store / record |
| 11 | schema_mismatch | Incompatible store schema |
| 12 | lock_conflict | Store lock held by another process |
| 13 | policy_denial | Execution policy denied |
| 14 | verification_failure | Independent Verification failed |

```bash
rivora doctor exit-codes --json
```

With `--json`, failures also emit a structured error object on stderr:

```json
{
  "error": true,
  "code": "store_locked",
  "message": "...",
  "exit_code": 12,
  "failure_class": "blocked",
  "retryable": false
}
```
