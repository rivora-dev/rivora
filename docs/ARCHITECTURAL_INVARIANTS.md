# Architectural Invariants

> Purpose: Preserve Rivora's architecture as the implementation evolves.
>
> **This is the tracked, authoritative source of non-negotiable invariants for
> contributors.** Local notes under `docs/internal/` are working copies only
> and are not required for the public repository.

## Core Philosophy

- Build the foundation before the ecosystem.
- Build the Runtime before the interface.
- Build understanding before automation.
- Build depth before breadth.

## Runtime

- The Runtime is the single source of engineering reasoning.
- Interfaces never contain business logic.
- Business logic belongs only in the Runtime.
- The Runtime owns Engineering Loop reasoning (Memory → Evaluation →
  Verification → Improvement → Learning).

## Investigations

- Every Engineering Object belongs to exactly one Investigation.
- Investigations are the unit of engineering understanding.
- Investigation histories remain independent and durable.

## Memory

- Memory is append-only.
- Historical facts are immutable.
- Knowledge is derived from Memory.
- Corrections create additional records; history is never rewritten.

## Knowledge

- Knowledge is derived, never manually maintained as a competing source of truth.
- Memory remains the historical source of truth.
- Historical and current evidence remain distinguishable.

## Evaluation

- Every evaluation is explainable and evidence-backed.
- Evaluations preserve supporting references.

## Verification

- Verification validates conclusions rather than creating them.
- Verification produces durable receipts.
- Failed and inconclusive verification attempts remain visible.
- **External API success does not equal Verification success.**
- **External API success does not equal Measured Outcome success.**

## Recommendations and Improvement

- Recommendations and Improvement Proposals are proposals, not facts.
- **Improvement Proposals are never auto-applied.**
- Proposal acceptance never implies execution.

## Learning

- Learning improves future reasoning without rewriting history.
- **Learning requires measured evidence.**
- Learning must not invent success from API acceptance or incomplete verification.
- Rejected, failed, ignored, and inconclusive outcomes remain visible.

## Capabilities

- Capabilities express engineering intent and domain meaning.
- Capabilities orchestrate Runtime behavior; they never contain engineering reasoning.
- CLI and Workspace use the same Capability implementations.
- **Every Capability explicitly declares Engineering Loop participation**
  (`Supported` / `NotApplicable` / `Unsupported` / `Deferred`) for each stage.
- **Capabilities do not directly write Memory, Evaluation, Verification,
  Improvement, or Learning artifacts outside Runtime orchestration.**
- Capabilities provide typed lifecycle contributions; the Runtime applies them.
- **Unsupported, deferred, and not-applicable stages are explicit** — never
  silent absence (`None`).
- **Replay of lifecycle processing is idempotent** (no duplicate artifacts for
  the same idempotency key / lineage head).

## Connectors

- **Connectors provide normalized external facts.**
- Connectors only observe and normalize (Observation Connectors).
- Connectors never evaluate, verify, recommend, or learn.
- **Observation Connectors remain read-only.**
- Vendor-specific API types stay inside Connectors; Capabilities consume
  canonical Runtime types.
- External mutation occurs only through bounded Execution Capabilities (or
  adapters) invoked by the Runtime after policy and approval — never from
  Observation Connectors.

## Execution Safety (v0.6+, preserved in v0.7)

- **Execution requires an Execution Plan, centralized policy evaluation,
  exact-revision human approval, and explicit live confirmation.**
- Proposal Accepted ≠ Execution Approved ≠ Execution Started ≠ Execution
  Verified ≠ Outcome Successful.
- Dry-run must never perform live mutation.
- Target drift invalidates approval.
- Rollback remains separately planned and approved; it is never automatic.
- High-risk and prohibited actions remain denied under centralized policy.

## Engineering Loop (v0.7)

```text
Connectors provide normalized engineering data.
Capabilities express engineering intent.
The Runtime produces engineering knowledge through the Engineering Loop:

Memory → Evaluation → Verification → Improvement → Learning
```

- Lifecycle runs are durable and inspectable.
- Partial progress, failures, deferred stages, and unsupported stages are
  represented explicitly.
- Lineage from Observation / invocation through Runtime artifacts is preserved.

## Interfaces

- Workspace, CLI, APIs, SDKs, and future interfaces all invoke the same Capabilities.
- Interfaces present results; they do not reason.
- Interfaces must not invent lifecycle state independently of the Runtime.

## Testing Standards

- Architecture tests
- Red → Green → Refactor (TDD)
- Integration tests
- End-to-end Investigation lifecycle tests
- Release-specific vertical slices and regression suites for prior versions

## Guiding Principle

When in doubt, preserve:

1. An exceptional Runtime
2. A thoughtful Workspace
3. An extensible ecosystem

Architectural evolution happens through RFCs, not accidental implementation changes.
