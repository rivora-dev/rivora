# Feedback Analysis

> A simple framework for evaluating design partner feedback and deciding what
> to build next.

---

## Purpose

Rivora is in a public v0.1 preview. Before adding cloud provider evidence
connectors, we collect structured feedback from early design partners and the
open-source community. This document defines the categories we evaluate and a
scoring table for connector prioritization.

See [ADR 0017](adr/0017-design-partner-feedback-before-provider-connectors.md)
for the decision to pause major feature expansion and listen first.

---

## Feedback categories

Every piece of feedback is sorted into one or more of these categories. The
goal is not a numeric score; it is a shared vocabulary for what to fix or build
next.

| Category | Question we are trying to answer |
|---|---|
| Activation friction | How hard was it to install and run the first command? |
| Demo clarity | Did the demo make the core loop obvious? |
| Evidence quality | Was the ingested evidence useful and accurate? |
| Memory quality | Were the proposed memory candidates worth approving? |
| Recall usefulness | Did `rivora ask` surface relevant past situations? |
| Slack setup friction | How painful was `rivora slack doctor` and Socket Mode? |
| Trust / safety concerns | Did anything make a user hesitate to trust Rivora? |
| Connector demand | Which evidence source do people ask for most? |
| Weekly usage potential | What single change would make Rivora a weekly habit? |

---

## Connector prioritization table

Provider integrations should be **read-only evidence connectors first**. They
should not mutate infrastructure, trigger deployments, roll back, or run
remediation. The Vercel connector (Phase 18A) is the first provider connector;
it ingests deployment evidence that can be turned into memory candidates
through the feedback loop.

Score each candidate on a simple 1 (low) to 3 (high) scale. Higher priority
goes to connectors with high demand, low setup complexity, good read-only API
quality, high evidence usefulness, low safety risk, and high demo value.

| Provider / Source | User demand | Setup complexity | Read-only API quality | Evidence usefulness | Safety risk | Demo value | Priority |
|---|---|---|---|---|---|---|---|
| Vercel (Phase 18A) | | | | | | | Done |
| Cloudflare (Phase 18B) | | | | | | | Done |
| Product validation (Phase 18.5) | | | | | | | Done |
| Render | | | | | | | |
| AWS | | | | | | | |
| GCP | | | | | | | |
| Azure | | | | | | | |
| Kubernetes | | | | | | | |
| Sentry (Phase 20A) | Validating | Low | Good (`GET`, `event:read`) | High | Low | High | Done |
| Datadog | | | | | | | |
| PagerDuty | | | | | | | |
| Linear | | | | | | | |
| Jira | | | | | | | |

Sentry is metadata-first in Phase 20A. Render remains deferred until direct
user or design-partner demand. The next planned data-layer connector is
PlanetScale and must remain metadata-first without reading customer rows.
Phase 20A.1 audited GET-only enforcement, field allowlisting, token and PII
redaction, offline fixtures, CLI/Slack parity, and cross-source behavior before
the roadmap proceeds.

### Scoring guide

* **User demand** — how many design partners and community members ask for it.
* **Setup complexity** — how hard it is for a user to grant read-only access.
* **Read-only API quality** — does the provider offer a clean `GET`-only path?
* **Evidence usefulness** — will the evidence feed memory candidates that
  matter during incidents and deploys?
* **Safety risk** — could the connector accidentally expose secrets or imply a
  write path? Lower is better.
* **Demo value** — can we build a deterministic fixture demo without
  credentials?
* **Priority** — a combined ranking used to order Phase 18 work.

---

## How feedback changes the roadmap

1. Collect feedback via the issue templates and community discussions (see
   [COMMUNITY_FEEDBACK.md](COMMUNITY_FEEDBACK.md)).
2. Sort each item into the categories above.
3. Update the connector table as demand signals arrive.
4. Decide Phase 18 connector order based on the table, not on assumption.
5. Record the decision in an ADR before starting implementation.

Phase 18 order may change based on design partner feedback. All provider
integrations start as read-only evidence connectors. See
[18-Roadmap.md](18-Roadmap.md).

---

## Related

- [COMMUNITY_FEEDBACK.md](COMMUNITY_FEEDBACK.md)
- [DESIGN_PARTNER_ONBOARDING.md](DESIGN_PARTNER_ONBOARDING.md)
- [adr/0017-design-partner-feedback-before-provider-connectors.md](adr/0017-design-partner-feedback-before-provider-connectors.md)
- [18-Roadmap.md](18-Roadmap.md)
- [EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md)
