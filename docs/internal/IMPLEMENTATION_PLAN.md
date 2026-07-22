# Rivora v0.1 Implementation Plan

> **Release:** v0.1 — Runtime Foundation
> **Status:** Implemented
> **Primary question:** Can Rivora reason?

## Goal

Implement the first usable Rivora Runtime capable of executing the complete engineering reasoning lifecycle:

```text
Observation
→ Memory
→ Knowledge
→ Evaluation
→ Verification
→ Recommendation
→ Learning
```

Rivora v0.1 establishes the architectural foundation defined by RFC-000 through RFC-014 without attempting to deliver later roadmap features such as investigation graphs, collaboration, automation, or a plugin ecosystem.

---

# Phase 1 — Core Runtime

## Purpose

Create the foundational types, boundaries, and lifecycle required for every later Runtime subsystem.

## Primary RFCs

* RFC-004 — Engineering Object Model
* RFC-005 — Runtime
* RFC-013 — Investigation Lifecycle
* RFC-014 — Runtime Execution Model

---

# Phase 2 — Engineering Reasoning

## Purpose

Implement the complete reasoning pipeline inside the Runtime.

## Primary RFCs

* RFC-006 — Event and Observation Model
* RFC-007 — Memory
* RFC-008 — Knowledge
* RFC-009 — Evaluation
* RFC-010 — Verification
* RFC-011 — Capabilities
* RFC-012 — Connectors
* RFC-013 — Investigation Lifecycle
* RFC-014 — Runtime Execution Model

Use the repository's canonical RFC names and numbering if they differ from this summary.

---

# Phase 3 — Capabilities, Connectors, and Interfaces

## Purpose

Expose the Runtime through a small set of consistent operations and prove that multiple interfaces can use the same underlying system.

## Primary RFCs

* RFC-003 — Interaction Model
* RFC-011 — Capabilities
* RFC-012 — Connectors
* RFC-014 — Runtime Execution Model

---

# Explicitly Out of Scope for v0.1

Do not implement:

* Related Investigation Graph.
* Context Graph.
* Artifact Graph beyond basic object relationships required by v0.1.
* Semantic cross-investigation search.
* Organizational knowledge.
* Multi-user collaboration.
* Comments and assignments.
* Scheduled automation.
* Background autonomous workflows.
* Automatic application of Recommendations.
* Connector SDK.
* Capability SDK.
* Plugin marketplace or extension registry.
* Enterprise SSO.
* RBAC.
* Multi-tenancy.
* Billing.
* Compliance features.
* Hosted cloud control plane.
* Public general-purpose API.
* Production MCP server.

Small internal interfaces may be designed so future APIs or MCP integrations remain possible, but those products are not v0.1 deliverables.

---

# Development Method

Every feature must follow:

## Red

* Write a failing test describing the expected behavior.
* Run the test.
* Confirm it fails for the intended reason.

## Green

* Implement the smallest correct change.
* Run the focused test.
* Run the relevant subsystem tests.

## Refactor

* Improve naming and structure.
* Remove duplication.
* Preserve architectural boundaries.
* Run the full test suite.

Do not write large amounts of implementation code before establishing the expected behavior through tests.

---

# Release Criteria

Rivora v0.1 is complete when:

* The canonical Investigation lifecycle is implemented.
* Observations can be normalized and ingested.
* Memory is durable and append-only.
* Knowledge is derived from Memory.
* Evaluations are explainable.
* Verification produces durable Verification Receipts.
* Recommendations are evidence-backed and remain proposals.
* Learning records outcomes without rewriting history.
* Capabilities execute the Runtime end to end.
* At least one connector is production-ready for MVP usage.
* The Workspace and CLI use the same Runtime and Capabilities.
* A complete Investigation can run from Observation through Learning.
* Architectural invariants are covered by tests.
* Formatting passes.
* Clippy passes with denied warnings.
* All unit, integration, and end-to-end tests pass.
* Public behavior is documented.
* No later-version features have leaked into the release.

---

# Success

Rivora v0.1 proves that the architectural foundation works.

It does not need to solve every engineering problem.

It must demonstrate that one coherent Runtime can observe engineering work, preserve durable Memory, derive Knowledge, evaluate and verify conclusions, produce explainable Recommendations, and learn from outcomes through a simple Workspace and CLI.