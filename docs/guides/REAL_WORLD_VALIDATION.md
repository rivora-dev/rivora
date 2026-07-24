# Real-World Validation Report (v0.9)

Validation uses synthetic profiles that mirror small/medium/large-supported envelopes.
No secrets or private repositories are committed.

| Profile | Operations | Result | Threshold |
|---------|------------|--------|-----------|
| Small store | open, ingest, recall, health | Pass | store_open / ingestion budgets |
| Medium synthetic (50 obs) | ingest, recall, load | Pass | large investigation load < 3s |
| Duplicate ingestion | same idempotency key | Pass | single Memory record |
| Corrupt sibling JSON | list + health | Pass | healthy reads continue |
| Prior layout (no store.json) | open migrates manifest | Pass | compatible |
| Stale lock | recover + open | Pass | lock recovered |
| Backup outside root | backup + open | Pass | records preserved |
| Oversized payload | ingest reject | Pass | PayloadTooLarge |
| Connector redaction | sanitize/redact | Pass | no secret leak |
| Search default bound | 15 investigations | Pass | ≤ DEFAULT_LIST_LIMIT |
| Micro-benchmarks | open/read/write/ingest/dup | Pass | see PERFORMANCE_BUDGETS.md |

Automated suite: `cargo test -p rivora --test v0_9_production_hardening`.

Live provider calls (GitHub/Sentry) remain optional and fixture-first for CI.
