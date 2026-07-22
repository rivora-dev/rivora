# Rivora v0.3 Implementation Plan

> **Release:** v0.3 — Engineering Assistance  
> **Status:** Implemented  
> **Primary question:** Can Rivora help?

## Goal

Extend Rivora so current and historical engineering understanding becomes
actively useful through explainable, evidence-backed assistance—without
turning Rivora into an autonomous agent or automatic remediation system.

Preserve v0.1 Runtime Foundation and v0.2 Investigation Intelligence.

```text
Observe → Remember → Understand → Assist
```

---

# Phase 1 — Composite Capabilities and Assisted Workflows

## Purpose

Coordinate existing Core Capabilities into durable, inspectable assisted
workflows.

## Primary RFCs

* RFC-011 — Capabilities
* RFC-014 — Runtime Execution Model
* RFC-018 — Composite Capabilities and Assisted Workflows

## Deliverables

* Composite Capability definitions
* Assisted Workflow domain + persistence
* Plan / execute / cancel / resume / explain / summarize
* CLI `assist` and Workspace Assistance session

---

# Phase 2 — Expanded Engineering Connectors

## Purpose

Richer read-only evidence across CI, infrastructure, and observability.

## Primary RFCs

* RFC-012 — Connectors

## Deliverables

* GitHub Actions connector
* Kubernetes connector
* Sentry connector
* Fixture mode, status, secret redaction
* CLI `connector` commands and Workspace connector panel

---

# Phase 3 — Explainable Engineering Assistance

## Purpose

Turn evidence into hypotheses, verification guidance, readiness, risk,
root-cause guidance, prioritized recommendations, and reports.

## Primary RFCs

* RFC-019 — Engineering Assistance

## Deliverables

* Hypothesis model and generation
* Next-best verification suggestions
* Deployment readiness assessment
* Risk forecast
* Root-cause guidance
* Recommendation prioritization factors
* Engineering reports

---

# Explicitly Out of Scope for v0.3

Do not implement:

* Automatic infrastructure mutation, deploy, merge, restart
* Auto-applied Recommendations
* Unrestricted agent loops or tool invention
* Collaboration / multi-user product features
* Marketplace / plugin SDK
* Hosted multi-tenant infrastructure
* Chat-first agent interface as primary UX

---

# Prior Releases

* v0.1 — Runtime Foundation (Implemented)
* v0.2 — Investigation Intelligence (Implemented)
