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

---

## Planned

### Phase 20 — Ability Proposal Runtime

Deferred until Phase 19 usability improvements are validated by design partner
feedback. A runtime that proposes actions for human approval. Rivora still does
not execute infrastructure actions; proposals are explainable and
receipt-backed.

### Phase 21 — Next Provider Connector

Deferred until Phase 19 usability improvements are validated by design partner
feedback. The next provider connector will be chosen based on design partner
feedback and the connector prioritization table in
[FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md). Render, AWS, GCP, Azure,
Kubernetes, Sentry, Datadog, and PagerDuty are deferred until the current
product loop is validated by real usage.

---

## Prioritization principles

* **Phase 18 order may change based on design partner feedback.** The
  connector prioritization table in
  [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md) drives the decision.
* **All provider integrations start as read-only evidence connectors.** They
  ingest evidence and never mutate infrastructure, trigger deployments, roll
  back, or run remediation.
* **The Ability Runtime remains deferred** until evidence quality and recall
  usefulness are validated by real usage.
* **Safety is not traded for speed.** Every memory operation produces a
  receipt. Human approval is required before evidence becomes memory.

---

## Related

- [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md)
- [COMMUNITY_FEEDBACK.md](COMMUNITY_FEEDBACK.md)
- [DESIGN_PARTNER_ONBOARDING.md](DESIGN_PARTNER_ONBOARDING.md)
- [EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md)
- [adr/0017-design-partner-feedback-before-provider-connectors.md](adr/0017-design-partner-feedback-before-provider-connectors.md)
