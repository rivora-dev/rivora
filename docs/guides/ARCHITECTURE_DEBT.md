# Architecture Debt Register (v0.9)

Audit basis: full v0.8 codebase review plus v0.9 hardening implementation and tests.

| ID | Title | Subsystem | Severity | Breaking? | Decision |
|----|-------|-----------|----------|-----------|----------|
| AD-001 | Search is full-scan ranking, not a durable inverted index | Search | Medium | No | Accept for MVP envelope; rebuildable indexes only where added for idempotency keys |
| AD-002 | Knowledge replace is delete-then-write (not single-file snapshot) | Persistence | Low | No | Accept; knowledge is derived and refreshable |
| AD-003 | Observation connectors still single-page for some providers | Connectors | Low | No | Documented limitation; not expanded in v0.9 |
| AD-004 | Process lock is advisory (PID file), not kernel flock | Concurrency | Low | No | Sufficient for local single-user MVP; documented |

No architecture debt requires a breaking redesign before v1.0 freeze preparation.
Items above are intentional envelope tradeoffs with documented workarounds.
