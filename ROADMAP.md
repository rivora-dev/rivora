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

## Current boundary

v0.8 expands the Capability Engineering Loop from a validated vertical slice into
the standard model for all first-party Capabilities and Connectors.

Every registered first-party Capability:

* exposes a complete descriptor (identity, provider, operation, risk, permissions,
  inputs/outputs, limitations, lifecycle participation)
* uses shared Runtime orchestration for Memory → Evaluation → Verification →
  Improvement → Learning
* never writes lifecycle artifacts directly
* routes via canonical, provider-independent input type identifiers

Every first-party Connector:

* authenticates, collects, normalizes, sanitizes, preserves provenance, and delivers
* emits canonical Observation kinds as Runtime inputs
* never evaluates, verifies, recommends, improves, or learns

CLI and Workspace always expose the same registered first-party set and a shared
coverage/health report. Live mutation still requires plan, policy, exact-revision
approval, confirmation, and independent verification.

```text
Connectors provide normalized facts
Capabilities express intent and typed contributions
Runtime produces engineering knowledge through the Engineering Loop
```

```text
Proposal Accepted
  ≠ Execution Approved
  ≠ Execution Started
  ≠ Execution Completed
  ≠ Execution Verified
  ≠ Outcome Successful
  ≠ Learning complete
```

v0.8 cannot: Capability marketplace/SDK, automatic Proposal acceptance, automatic
execution or rollback, connector reasoning, dynamic plugins, cloud control plane,
or production-hardening work reserved for v0.9.

## Future backlog

Autonomous remediation, inferred implementation tracking, collaboration, SDKs,
marketplaces, hosted control planes, multi-tenancy, multi-user approval workflows,
production hardening (v0.9), and enterprise administration remain future work.
They are not part of v0.8.
