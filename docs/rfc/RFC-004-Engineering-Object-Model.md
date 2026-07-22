# RFC-004: Engineering Object Model

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines Rivora's core engineering domain model.

RFC-000 explains why Rivora exists.

RFC-001 defines the engineering principles.

RFC-002 defines the architecture.

RFC-003 defines how engineers interact with Rivora.

RFC-004 defines the language the Runtime uses to understand engineering work.

Every capability, interface, and subsystem should build upon these shared objects.

---

# Philosophy

Software engineering is not a collection of prompts.

It is a collection of engineering work.

Rivora models that work through durable engineering objects instead of transient conversations.

These objects create a common language across the Runtime.

---

# Core Engineering Objects

## Investigation

The primary unit of engineering work.

An Investigation groups everything related to understanding and improving a specific engineering problem or goal.

An Investigation may contain:

- Observations
- Memory
- Knowledge
- Evaluations
- Verification Receipts
- Recommendations
- Artifacts
- Timelines
- Graphs
- Notes
- Capability Executions

Investigations persist over time and preserve engineering context.

---

## Observation

An Observation is a recorded engineering event.

Examples:

- Pull request opened
- Deployment completed
- Test failed
- Incident created
- Engineer request
- Workspace interaction
- Capability execution

Observations are immutable facts.

---

## Memory

Memory is the durable record of observations.

Memory answers:

"What happened?"

Memory is append-only.

It never rewrites history.

---

## Knowledge

Knowledge is derived from Memory.

Knowledge answers:

"What does it mean?"

Knowledge represents relationships, patterns, correlations, summaries, and engineering understanding.

Knowledge can evolve as additional observations arrive.

---

## Evaluation

An Evaluation interprets engineering significance.

Examples include:

- Risk
- Health
- Confidence
- Severity
- Impact
- Readiness

Evaluations should always be evidence-backed.

---

## Verification Receipt

A Verification Receipt captures evidence supporting or disproving an engineering conclusion.

Receipts provide traceability and explainability.

Every important recommendation should be supported by one or more receipts.

---

## Recommendation

A Recommendation proposes an engineering action.

Recommendations never represent facts.

They are generated from Evaluations and supported by Verification Receipts.

Humans remain responsible for final decisions.

---

## Artifact

Artifacts are durable outputs created during engineering work.

Examples:

- Reports
- Graphs
- Timelines
- Summaries
- Architecture diagrams
- Root cause analyses

Artifacts are attached to Investigations.

---

## Capability

Capabilities are reusable Runtime behaviors.

Examples:

- Investigate
- Verify
- Learn
- Search Memory
- Analyze Risk
- Generate Timeline

Capabilities operate on engineering objects rather than directly on external systems.

---

# Relationships

```
Investigation
│
├── Observations
├── Memory
├── Knowledge
├── Evaluations
├── Verification Receipts
├── Recommendations
├── Artifacts
└── Capability Executions
```

The Investigation is the aggregate root of engineering work.

---

# Object Lifecycle

Every Observation enters the Runtime and contributes to the object model.

```
Observation
      ↓
Memory
      ↓
Knowledge
      ↓
Evaluation
      ↓
Verification Receipt
      ↓
Recommendation
      ↓
Artifact
```

These objects accumulate within an Investigation over time.

---

# Modeling Principles

- Facts are immutable.
- Memory records facts.
- Knowledge is derived.
- Recommendations are never facts.
- Every recommendation should be explainable.
- Engineering context belongs to Investigations.
- Objects should be composable and reusable across every interface.

---

# Architectural Guarantees

Rivora guarantees:

- Investigations are the primary unit of engineering work.
- Observations are immutable.
- Memory is append-only.
- Knowledge is derived from Memory.
- Evaluations are evidence-backed.
- Recommendations require supporting evidence.
- Artifacts preserve engineering outputs.
- Capabilities operate on engineering objects.
- Every interface shares the same object model.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

The Engineering Object Model establishes Rivora's ubiquitous language.

Rather than thinking in terms of prompts or commands, Rivora understands engineering work through durable domain objects centered around Investigations.

This shared model allows the Runtime, Workspace, CLI, and future integrations to reason about engineering work consistently while preserving long-term engineering understanding.
