# Rivora v0.4 Implementation Plan

> **Release:** v0.4 — Improvement Proposals
> **Status:** Implemented
> **Primary question:** Can Rivora propose how the engineering system should improve?

## Goal

Extend the v0.1-v0.3 Runtime with durable, explainable Improvement Proposals
without applying changes or mutating engineering systems.

```text
Observe → Remember → Understand → Assist → Propose Improvement
```

## Architectural Basis

* RFC-020 — Improvement Proposal Model and Lifecycle
* RFC-021 — Proposal Generation, Comparison, Planning, and Export
* RFC-018 — Composite Capabilities
* RFC-019 — Engineering Assistance

## Phase 1 — Proposal Model and Lifecycle

* distinct durable Improvement Proposal Engineering Object
* explicit human-controlled lifecycle and immutable transitions
* full preserved revisions and feedback provenance
* lazy per-Investigation storage with corrupted-record isolation
* shared Runtime Capabilities, CLI commands, and Workspace area
* no application or source-object mutation

Gate: focused model/storage/lifecycle/interface tests plus all existing tests.

## Phase 2 — Generation and Comparison

* deterministic read-only generation from existing durable evidence
* current/historical evidence labeling and dismissed-context exclusion
* bounded alternatives and inspectable comparison/priority factors
* implementation outlines and Verification Plans
* feedback-driven refinement
* bounded `propose_engineering_improvement` Composite Capability

Gate: focused generation/comparison/composite tests plus full validation.

## Phase 3 — Planning, Export, and Experience

* deterministic Markdown and structured artifacts
* bounded coding-agent handoff text
* Investigation Proposal portfolio and traceability
* Workspace status/revision/export experience and explicit boundary language
* isolated end-to-end workflow and restart verification

Gate: export/interface/e2e tests, structured review, full repository validation,
manual isolated verification, and documentation/version closeout.

## Compatibility

Existing v0.1-v0.3 JSON remains unchanged. v0.4 storage is additive and lazy;
missing Proposal directories return empty. No destructive migration is planned.

## Explicitly Out of Scope

Automatic application, repository editing, patches written to source, branches,
commits, pull requests, deploys, infrastructure/configuration/ticket mutation,
agent invocation, unrestricted loops, scheduled automation, collaboration,
SDKs, marketplaces, hosted control planes, multi-tenancy, and v0.5+ behavior.
