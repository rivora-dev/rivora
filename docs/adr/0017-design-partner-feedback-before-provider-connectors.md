# ADR 0017: Design partner feedback before provider connectors

Date: 2026-06-29

## Status

Accepted

## Context

Rivora launched a public v0.1 preview with local memory, Git and GitHub
evidence ingestion, a CLI, packaged demo scenarios, and a self-hosted Slack
interface. The natural next step is to add cloud provider evidence connectors
(Vercel, Cloudflare, Render, AWS, GCP, Azure, Kubernetes) and eventually an
Ability Proposal Runtime.

Building those connectors without signal from real teams risks building
integrations nobody uses. Rivora's product thesis is adaptive reliability
memory for engineering teams, and the core loop (Ask, Explain, Remember,
Recall) needs to feel useful before more evidence sources widen the top of the
funnel.

The v0.1 preview is the first time people outside the core team can try Rivora
end to end. That makes it the right moment to listen before building more.

## Decision

Rivora will collect structured design partner feedback before prioritizing
provider evidence connectors.

Concretely:

* Add structured GitHub issue templates for bug reports, feedback, evidence
  connector requests, Slack setup help, and design partner reports.
* Add a pull request template with a safety boundary checklist.
* Add a community feedback doc, a feedback analysis framework, and an updated
  design partner onboarding guide.
* Defer all cloud provider connectors, the Ability Runtime, Rivora Cloud,
  hosted OAuth, the official Slack Marketplace app, billing, and dashboards.
* Use the feedback analysis framework and connector prioritization table to
  decide Phase 18 connector order.

Provider integrations will be **read-only evidence connectors first**. They
will ingest evidence and never mutate infrastructure, trigger deployments,
roll back, or run remediation. The Ability Runtime remains deferred until
evidence quality and recall usefulness are validated by real usage.

## Consequences

* Slower feature expansion in the short term.
* Better connector prioritization based on real demand.
* Less risk of building unused integrations.
* Stronger trust posture: Rivora listens before acting.
* Clearer open-source community signal: feedback has a structured home.
* Phase 18 connector order may change based on what design partners report.

## Related

- [18-Roadmap.md](../18-Roadmap.md)
- [FEEDBACK_ANALYSIS.md](../FEEDBACK_ANALYSIS.md)
- [COMMUNITY_FEEDBACK.md](../COMMUNITY_FEEDBACK.md)
- [DESIGN_PARTNER_ONBOARDING.md](../DESIGN_PARTNER_ONBOARDING.md)
- [EVIDENCE_CONNECTORS.md](../EVIDENCE_CONNECTORS.md)
