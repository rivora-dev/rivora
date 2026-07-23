# RFC-028 --- Connectors and Capabilities

**Status:** Implemented (v0.7)

**Author:** Sergio Rovira

**Target Version:** v0.7 (Engineering Loop Integration); foundational for later SDK work

------------------------------------------------------------------------

# Summary

This RFC formalizes the architectural boundary between **Connectors**
and **Capabilities**.

Connectors are responsible for communicating with external systems.

Capabilities are responsible for transforming external events into
engineering knowledge.

This separation ensures that Rivora remains an engineering system---not
an integration platform.

------------------------------------------------------------------------

# Motivation

As Rivora grows, it will integrate with dozens of engineering systems:

-   GitHub
-   Kubernetes
-   AWS
-   Vercel
-   Cloudflare
-   Docker
-   Sentry
-   Linear
-   Jira
-   Datadog
-   PostgreSQL

Without a clear architectural boundary, business logic can slowly leak
into integrations.

Instead, Rivora adopts a strict separation of responsibilities.

------------------------------------------------------------------------

# Design Principles

## Connectors provide data.

Connectors are responsible for:

-   Authentication
-   API communication
-   Event collection
-   Webhooks
-   Polling
-   Session management
-   Rate limiting
-   Normalization

A connector should never make engineering decisions. Its responsibility
ends after producing normalized engineering events.

## Capabilities produce engineering knowledge.

Capabilities consume normalized engineering events and transform them
into engineering knowledge.

Capabilities understand:

-   Engineering workflows
-   Investigations
-   Deployments
-   Incidents
-   Pull requests
-   Memory
-   Evaluation
-   Verification
-   Improvement
-   Learning

Capabilities never communicate directly with external APIs.

------------------------------------------------------------------------

# Architectural Overview

``` text
External Systems
        │
        ▼
+--------------------+
|    Connectors      |
+--------------------+
        │
        ▼
Normalized Engineering Events
        │
        ▼
+--------------------+
|    Capabilities    |
+--------------------+
        │
        ▼
Engineering Loop
        │
 ┌──────────────┐
 │   Memory     │
 ├──────────────┤
 │ Evaluation   │
 ├──────────────┤
 │ Verification │
 ├──────────────┤
 │ Improvement  │
 ├──────────────┤
 │ Learning     │
 └──────────────┘
```

------------------------------------------------------------------------

# Connector Responsibilities

A connector exists solely to communicate with external systems.

Example: **GitHub Connector**

Provides:

-   Pull Requests
-   Commits
-   Reviews
-   Workflow Runs
-   Releases
-   Issues

The connector never determines engineering quality or generates
recommendations. It simply produces normalized engineering events.

------------------------------------------------------------------------

# Capability Responsibilities

Capabilities transform engineering events into engineering knowledge.

Example: **Deployment Capability**

Consumes:

-   Deployment events
-   Workflow events
-   Runtime events

Produces:

-   Memory
-   Evaluation
-   Verification
-   Improvement
-   Learning

Capabilities understand engineering.

Connectors understand APIs.

------------------------------------------------------------------------

# Engineering Loop

Every capability participates in the Engineering Loop.

``` text
Capability
    ↓
Memory
    ↓
Evaluation
    ↓
Verification
    ↓
Improvement
    ↓
Learning
```

This is a core architectural invariant.

------------------------------------------------------------------------

# Connector Independence

Capabilities are intentionally connector-agnostic.

``` text
GitHub Connector
        │
        ▼
Deployment Capability
        ▲
        │
GitLab Connector
```

Both connectors feed the same capability.

------------------------------------------------------------------------

# v0.7 Implementation Notes

As of v0.7, Rivora implements the Engineering Loop contract without a
Capability marketplace or SDK:

- `ExecutionCapabilityDescriptor.engineering_loop` declares participation
  per stage (`Supported`, `NotApplicable`, `Unsupported`, `Deferred`).
- `ExecutionCapability::lifecycle_contributions` returns typed
  `CapabilityLifecycleContributions` (Memory / Evaluation / Verification /
  Improvement / Learning wrappers).
- Runtime `run_capability_lifecycle_for_attempt` validates contributions,
  applies existing subsystem APIs, and persists `CapabilityLifecycleRun`
  snapshots with explicit stage status and idempotent replay.
- Observation → Capability routing uses `accepted_input_types` (stable type
  identifiers), not human-readable names alone.
- CLI: `rivora capability list|show|route|lifecycle|trace`.
- Workspace: Capability Engineering Loop inspection surface.

# Capability SDK

Future versions of Rivora may include a Capability SDK.

``` bash
rivora capability new deployment
```

Generates:

``` text
capability.toml
memory.rs
evaluation.rs
verification.rs
improvement.rs
learning.rs
```

No connector code is generated. Capabilities operate on normalized
engineering events rather than external APIs. SDK scaffolding is **not**
part of v0.7.

------------------------------------------------------------------------

# Architectural Invariants

1.  Connectors never contain engineering logic.
2.  Capabilities never communicate directly with external APIs.
3.  Every capability participates in the Engineering Loop.
4.  All external systems are accessed through connectors.
5.  Engineering knowledge is produced only by capabilities.

------------------------------------------------------------------------

# Philosophy

Rivora is not an integration platform.

Rivora is an engineering system.

**Observation Connectors provide normalized external facts. Execution Capabilities perform explicitly authorized actions.**

**All Capabilities contribute typed context to the Engineering Loop, while the Runtime produces engineering knowledge.**

**The Engineering Loop continuously transforms that knowledge into a
better engineering system.**

This separation is one of the core architectural foundations of Rivora.
