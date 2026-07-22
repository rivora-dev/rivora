# RFC-005: Runtime

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines the Rivora Runtime.

The Runtime is the core execution engine responsible for transforming engineering observations into shared engineering understanding.

Every interface—including the Workspace, CLI, APIs, MCP servers, SDKs, and future integrations—communicates with the same Runtime.

---

# Runtime Philosophy

The Runtime is the product.

Interfaces are experiences built on top of it.

The Runtime owns engineering understanding.

It does not own GitHub, CI, cloud providers, observability systems, or coding agents. Those remain external systems of record.

---

# Responsibilities

The Runtime is responsible for:

- Receiving engineering observations
- Managing Investigations
- Building and maintaining Memory
- Deriving Knowledge
- Performing Evaluations
- Producing Verification Receipts
- Generating Recommendations
- Measuring outcomes through Learning
- Executing Capabilities
- Serving every interface consistently

No interface duplicates this logic.

---

# Runtime Lifecycle

Every observation entering Rivora follows the same lifecycle.

```text
Observation
      ↓
Memory
      ↓
Knowledge
      ↓
Evaluation
      ↓
Verification
      ↓
Recommendation
      ↓
Learning
```

This lifecycle is continuous.

Learning improves future interpretation while historical Memory remains immutable.

---

# Runtime Components

## Observation Engine

Accepts events from external systems and user interactions.

---

## Investigation Manager

Creates, updates, and maintains Investigations.

Every observation belongs to an Investigation.

---

## Memory Engine

Persists immutable engineering facts.

Provides durable engineering memory.

---

## Knowledge Engine

Derives relationships, summaries, patterns, and engineering understanding from Memory.

---

## Evaluation Engine

Determines engineering significance using evidence.

Produces risk, health, confidence, readiness, and other engineering assessments.

---

## Verification Engine

Validates conclusions with observable evidence.

Produces Verification Receipts.

---

## Recommendation Engine

Generates explainable engineering recommendations.

Recommendations are proposals—not facts.

---

## Learning Engine

Measures outcomes after recommendations are applied.

Improves future Runtime behavior without rewriting history.

---

## Capability Engine

Executes reusable engineering capabilities such as:

- Investigate
- Verify
- Learn
- Search Memory
- Analyze Risk
- Generate Timeline

Capabilities operate on the Engineering Object Model defined in RFC-004.

---

# Execution Model

Every interface invokes Runtime capabilities.

```text
Workspace ─┐
CLI ───────┼──► Runtime
API ───────┤
MCP ───────┤
SDK ───────┘
```

The Runtime returns engineering objects—not interface-specific responses.

Each interface decides how to present those objects.

---

# Runtime Principles

- Stateless interfaces, stateful Runtime.
- Business logic exists only in the Runtime.
- Memory is append-only.
- Knowledge is derived.
- Recommendations require evidence.
- Learning improves future decisions.
- Every interface receives consistent behavior.

---

# Architectural Guarantees

The Runtime guarantees:

- One shared implementation for every interface.
- Consistent engineering understanding regardless of entry point.
- Immutable historical memory.
- Explainable recommendations.
- Evidence-backed evaluations.
- Reusable capabilities.
- Independence from any specific coding agent or UI.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

The Rivora Runtime is the central execution engine that powers every interface and capability.

By concentrating engineering logic inside the Runtime, Rivora delivers consistent, explainable, and continuously improving engineering understanding regardless of whether engineers interact through the Workspace, CLI, or future integrations.
