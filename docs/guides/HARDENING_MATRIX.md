# Production Hardening Matrix (v0.9)

| ID | Subsystem | Description | Severity | Likelihood | v1.0 blocking | Disposition |
|----|-----------|-------------|----------|------------|---------------|-------------|
| H-001 | Persistence | No store lock / multi-process corruption | High | Medium | Yes | **Resolved** — exclusive lock + stale recovery |
| H-002 | Persistence | write_json without fsync / shared temp | High | Medium | Yes | **Resolved** — unique temp + sync |
| H-003 | Persistence | Corrupt memory/obs fail whole list | High | Medium | Yes | **Resolved** — isolation |
| H-004 | Idempotency | Observation key TOCTOU | High | Medium | Yes | **Resolved** — key index claim |
| H-005 | Errors | Binary exit codes only | Medium | High | Yes | **Resolved** — stable codes |
| H-006 | Connectors | Observation HTTP no timeout | High | Medium | Yes | **Resolved** — shared client timeouts |
| H-007 | Connectors | Uneven redaction | High | Medium | Yes | **Resolved** — redact_json + sanitize |
| H-008 | Scale | Unbounded search/list | Medium | High | Yes | **Resolved** — default/max limits |
| H-009 | Ops | No store health/doctor | Medium | High | Yes | **Resolved** — doctor commands |
| H-010 | Payload | No max Observation size | Medium | Medium | Yes | **Resolved** — 1 MiB limit |
| H-011 | Schema | No store manifest | Medium | Medium | Yes | **Resolved** — store.json v1 |
| H-012 | Backup | No backup/restore path | Medium | Medium | No | **Resolved** — doctor backup |
| H-013 | Search | Full scan ranking | Medium | High | No | **Deferred** — envelope only (AD-001) |
| H-014 | Connectors | Single-page collection | Low | High | No | **Deferred** — documented |

All v1.0-blocking items are resolved or deferred with non-blocking rationale.
