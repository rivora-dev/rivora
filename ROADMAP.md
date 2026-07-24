# Rivora Roadmap

Rivora builds depth release by release while preserving one Runtime and strict
engineering-system mutation boundaries.

## Release progression

* v0.1 — Runtime Foundation: observe, remember, reason, verify, recommend, learn
* v0.2 — Investigation Intelligence: relate, search, recall, reuse context
* v0.3 — Engineering Assistance: composite workflows, hypotheses, risk, readiness, reports
* v0.4 — Improvement Proposals: durable candidate changes, comparison, refinement, proposed plans, export
* v0.5 — Measure and Learn: Implementation Records, Measured Learning Outcomes, patterns, and influence
* v0.6 — Execute through external systems: approved bounded capabilities, plans, receipts, verification
* v0.7 — Engineering Loop Integration: formal Capability participation in Memory → Evaluation → Verification → Improvement → Learning
* v0.8 — Capability Coverage: every first-party Capability and Connector participates consistently in the Engineering Loop
* v0.9 — Production Hardening: prove the architecture remains reliable at realistic local/on-prem scale
* v0.9.1 — Binary Distribution and Installer: first-party `rivora.dev/install`, platform archives, checksums

## Current boundary

v0.9.1 ships binary distribution and installation on top of v0.9 production hardening.

v0.9 is the architectural release gate between completeness and public stability.

It strengthens existing systems under imperfect production conditions:

* operating envelope (small / medium / large_supported)
* performance budgets and micro-benchmarks
* store lock, durable writes, corruption isolation, backup/restore
* replay contracts and observation idempotency indexes
* Connector timeouts, payload limits, rate-limit errors, redaction
* CLI exit codes, JSON errors, `doctor` diagnostics
* Workspace bounded lists
* production readiness scorecard and v1.0 freeze assessment

```text
v0.7 — Connect the architecture
v0.8 — Apply the architecture across first-party Capabilities
v0.9 — Prove the architecture remains reliable at realistic scale
v0.9.1 — Make Rivora installable from rivora.dev with verified binaries
v1.0 — Freeze the validated contracts
```

v0.9 / v0.9.1 do **not** introduce a new foundational Runtime subsystem, redesign the
Engineering Loop, expand broad provider coverage, or begin v1.0 work.

Hardening may not weaken determinism, provenance, append-only Memory,
Connector/Capability boundaries, Verification independence, or execution authority.

## Future backlog

v1.0 Stable Engineering Platform (contract freeze after v0.9 release soak),
autonomous remediation, inferred implementation tracking, collaboration, SDKs,
marketplaces, hosted control planes, multi-tenancy, multi-user approval workflows,
and enterprise administration remain future work.
