# Production Readiness Scorecard (v0.9)

| Subsystem | Classification | Evidence | v1.0 blocker? |
|-----------|----------------|----------|---------------|
| Engineering Object Model | Production Ready | Domain types + tests through v0.8 | No |
| Runtime APIs | Production Ready | Shared Runtime + CapabilityService | No |
| Memory | Production Ready | Append-only + isolation + tests | No |
| Knowledge | Ready with Minor Limitations | Derived replace path (AD-002) | No |
| Evaluation | Production Ready | Evidence-backed evaluations | No |
| Verification | Production Ready | Independent receipts; API success ≠ success | No |
| Improvement | Production Ready | Proposals remain suggestions | No |
| Learning | Production Ready | Measured evidence required | No |
| Capability contract | Production Ready | v0.8 coverage + descriptors | No |
| Connector contract | Ready with Minor Limitations | Timeouts/redaction/limits; single-page providers | No |
| Capability Engineering Loop | Production Ready | v0.7/v0.8 loop + replay | No |
| Persistence | Production Ready | Durable writes, manifest, health | No |
| Migration | Production Ready | Additive 0.1–0.8 open + store.json | No |
| Replay/idempotency | Production Ready | Contracts + index + tests | No |
| CLI | Production Ready | Exit codes, doctor, bounds | No |
| Workspace | Ready with Minor Limitations | Bounded lists; terminal UI | No |
| Execution authority | Production Ready | v0.6 gates retained | No |
| Receipts | Production Ready | Durable + redaction | No |
| Rollback | Production Ready | Explicit only | No |
| Diagnostics | Production Ready | doctor health/export | No |
| Security | Ready with Minor Limitations | Redaction + residual local FS risk | No |
| Performance | Ready with Minor Limitations | Budgets + micro-benchmarks | No |
| Concurrency | Ready with Minor Limitations | Exclusive cross-process lock | No |

**Overall:** Production Ready for the documented local/on-prem operating envelope.
