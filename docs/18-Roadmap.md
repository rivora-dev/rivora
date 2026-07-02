# Roadmap

> Where Rivora is headed. Order may change based on design partner feedback.

---

## Completed

### Phase 1–9 — Core memory engine and CLI

Local `.rivora/` store, adaptive memory model, human feedback lifecycle,
deterministic recall, reliability receipts, and the CLI memory interface.

### Phase 10–11 — Git and GitHub evidence connectors

Read-only local Git history ingestion and read-only GitHub REST API ingestion
(pull requests, issues, workflow runs, releases, deployments).

### Phase 13 — Fixture evidence and demo scenarios

Deterministic local fixture ingestion for demos and tests. Four packaged
scenarios: `basic`, `checkout-incident`, `release-regression`,
`workflow-failure`.

### Phase 14–15 — Self-hosted Slack

Slack adapter, dev mode, Socket Mode transport, `rivora slack doctor`
validation, and token redaction in diagnostics.

### Phase 16 — Public launch polish

Public repository, README, changelog, security docs, contributor guide, and
design partner onboarding.

### Phase 17 — Design Partner Feedback Loop

Collect structured feedback from early design partners and the open-source
community before adding provider evidence connectors. Adds GitHub issue
templates, a pull request template, community feedback and feedback analysis
docs, a design partner report template, and ADR 0017.

See [ADR 0017](adr/0017-design-partner-feedback-before-provider-connectors.md)
and [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md).

### Phase 18A — Vercel Evidence Connector

Read-only Vercel deployment evidence ingestion. First provider evidence
connector, proving the connector model before adding Cloudflare, Render,
and AWS.

### Phase 18B — Cloudflare Evidence Connector

Read-only Cloudflare Pages and Workers deployment evidence ingestion. Second
provider evidence connector. Supports Pages deployment evidence and Workers
deployment evidence. Does not ingest D1, KV, R2, Queues, logs, analytics,
WAF, or security events.

### Phase 18.5 — Evidence-to-Memory Product Validation

Validates and hardens the complete Rivora product loop across all evidence
sources implemented so far: Git, GitHub, Vercel, Cloudflare Pages, and
Cloudflare Workers. Adds a multi-source demo scenario, cross-source evidence
grouping, improved ask/evidence/recall behavior, and comprehensive end-to-end
validation tests. This phase proves the product works end-to-end before
adding more provider connectors.

See [PRODUCT_VALIDATION.md](PRODUCT_VALIDATION.md).

### Phase 19 — Slack + CLI Usability Hardening

`rivora doctor` command for local diagnostics (store, `.gitignore`, provider
tokens). Subcommand help (`rivora <command> --help`) for all commands. Guided
next steps after `init`, `ingest`, `remember`, and `feedback`. Improved
empty-state messages with a "What happened? Why? What next?" structure.
Malformed `--since` guidance for provider ingests. Output formatting
improvements. Slack dev/socket parity improvements.

### Phase 20A — Sentry Observability Evidence Connector

Read-only, metadata-first Sentry issue/error evidence ingestion. Supports
explicit organization and project selection, environment/query/time filters,
stable local evidence IDs, deterministic recall, and cross-source release
summaries. It never resolves or assigns issues, mutates Sentry, or ingests raw
event payloads and stack traces by default.

### Phase 20A.1 — Sentry Connector Audit + Hardening

Audits and hardens GET-only enforcement, the exact metadata allowlist,
malicious nested-field exclusion, token/debug redaction, deterministic limits
and filtering, evidence and memory rendering, CLI/Slack parity, doctor output,
fixtures, and public documentation. Live Sentry remains optional and was not
required for the audit.

### Phase 20B — PlanetScale Data-Layer Evidence Connector

Read-only, metadata-first PlanetScale branch and deploy-request evidence.
Supports explicit organization/database selection, optional branch/time
filters, stable local IDs, deterministic recall, and cross-source release
summaries. It never connects to a database, runs SQL, reads customer rows or
branch passwords, ingests connection strings/raw query results/schema dumps/
schema diffs/raw DDL, or mutates PlanetScale.

### Phase 20B.1 — PlanetScale Connector Audit + Hardening

Audits and hardens PlanetScale service-token/OAuth authentication, GET-only
transport, credential redaction, poisoned allowlisted fields, safe permalinks,
deterministic deduplication and limits, approval-gated recall, CLI/Slack/doctor
parity, fixtures, and public documentation. Deploy operations remain deferred
because their payloads can contain raw DDL and table-level details.

---

## Planned

### Phase 21 — Evidence Correlation / Release Window Review

Careful cross-source review using same-window and nearby-evidence language,
without claiming root cause.

### Phase 22 — Reliability Wiki Memory

Human-approved operational memory organized for team review and recall.

### Phase 23 — Ability Proposal Runtime

Explainable, receipt-backed action proposals for human approval. Rivora still
does not execute infrastructure actions.

### Phase 24 — Next Provider Connector Based on Feedback

Selected from real user and design-partner demand. Render remains deferred
until that demand exists. OpenObserve is a future observability evidence
candidate. AWS, GCP, Azure, Kubernetes, and other providers remain deferred.

---

## Prioritization principles

* **Provider order may change based on design partner feedback.** The
  connector prioritization table in
  [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md) drives the decision.
* **All provider integrations start as read-only evidence connectors.** They
  ingest evidence and never mutate infrastructure, trigger deployments, roll
  back, or run remediation.
* **The Ability Runtime remains deferred** until evidence quality and recall
  usefulness are validated by real usage.
* **Evidence correlation precedes wiki memory and proposals.** Rivora first
  learns to present nearby evidence carefully, then organizes approved memory,
  then considers an Ability Proposal Runtime.
* **Safety is not traded for speed.** Every memory operation produces a
  receipt. Human approval is required before evidence becomes memory.

---

## Related

- [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md)
- [COMMUNITY_FEEDBACK.md](COMMUNITY_FEEDBACK.md)
- [DESIGN_PARTNER_ONBOARDING.md](DESIGN_PARTNER_ONBOARDING.md)
- [EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md)
- [adr/0017-design-partner-feedback-before-provider-connectors.md](adr/0017-design-partner-feedback-before-provider-connectors.md)
