# v1.0 Contract Freeze Assessment (post-v0.9)

| Contract | Classification | Notes |
|----------|----------------|-------|
| Engineering Object Model | Ready to Freeze | Stable domain types |
| Runtime APIs | Ready to Freeze | Shared Runtime ownership |
| Memory | Ready to Freeze | Append-only invariant |
| Knowledge | Ready to Freeze | Derived-from-Memory |
| Evaluation | Ready to Freeze | Evidence-backed |
| Verification | Ready to Freeze | Independent receipts |
| Improvement | Ready to Freeze | Suggestions only |
| Learning | Ready to Freeze | Measured evidence |
| Capability | Ready to Freeze | Descriptor + loop participation |
| Connector | Ready to Freeze | Observe/normalize only |
| Engineering Loop | Ready to Freeze | Stage statuses explicit |
| Persistence | Ready to Freeze | store schema v1 |
| Migration | Ready to Freeze | Additive prior versions |
| Replay/idempotency | Ready to Freeze | Contract table |
| CLI | Requires Minor Revision | Exit codes new in 0.9 — freeze after release soak |
| Workspace interaction model | Ready to Freeze | Thin over Runtime |
| Execution authority | Ready to Freeze | Plan/policy/approval/confirm |
| Approval | Ready to Freeze | Exact revision |
| Receipt | Ready to Freeze | Durable |
| Rollback | Ready to Freeze | Explicit only |
| Diagnostics | Requires Minor Revision | doctor surface new in 0.9 |
| Operating envelope | Ready to Freeze | Documented profiles |

**v1.0 may begin only after** a separate release-preparation step tags v0.9.0 and soaks CLI/diagnostics contracts.

No **Requires Breaking Revision** classifications remain from the v0.9 audit.
