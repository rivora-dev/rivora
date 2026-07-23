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

## Current boundary

v0.6 can create Execution Plans from accepted Proposals, evaluate centralized
execution policy, require exact-revision human approval, dry-run when supported,
invoke bounded external capabilities through Runtime-owned adapters, record
Receipts, independently verify immediate postconditions, and link to v0.5
Implementation Records / Measured Outcomes.

v0.6 cannot: run unrestricted shell commands; auto-execute accepted Proposals;
perform high-risk writes (merge, force-push, infrastructure delete); autonomously
remediate, schedule, or roll back; or treat external API success as Outcome success.

```text
Proposal Accepted
  ≠ Execution Approved
  ≠ Execution Started
  ≠ Execution Completed
  ≠ Execution Verified
  ≠ Outcome Successful
```

## Future backlog

Autonomous remediation, inferred implementation tracking, collaboration, SDKs,
marketplaces, hosted control planes, multi-tenancy, multi-user approval workflows,
and enterprise administration remain future work. They are not part of v0.6.
