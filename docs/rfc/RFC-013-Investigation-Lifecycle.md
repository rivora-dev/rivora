# RFC-013: Investigation Lifecycle

**Status:** Draft (Foundational)
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines the lifecycle of an Investigation within the Rivora Runtime.

Investigations are the primary unit of engineering work.

Every Observation, Memory record, Knowledge object, Evaluation, Verification Receipt, Recommendation, and Learning outcome belongs to an Investigation.

This RFC defines how an Investigation evolves from creation to completion.

---

# Philosophy

An Investigation is not a ticket.

It is not an incident.

It is not a chat session.

An Investigation is a living engineering context that accumulates understanding over time.

The Runtime continuously enriches an Investigation as new observations arrive and new reasoning is performed.

---

# Lifecycle

Every Investigation progresses through the following stages:

Created
↓
Collecting
↓
Understanding
↓
Evaluating
↓
Verifying
↓
Recommending
↓
Learning
↓
Completed
↓
Archived

The lifecycle is iterative rather than strictly linear.

New observations may return an Investigation to earlier stages while preserving its history.

---

# Lifecycle Stages

## Created

A new engineering question, event, or user action creates an Investigation.

## Collecting

The Runtime gathers Observations from Connectors, users, and Capabilities.

Memory grows.

## Understanding

Knowledge is derived from accumulated Memory.

Patterns and engineering context emerge.

## Evaluating

The Runtime assesses engineering significance, risk, health, confidence, and readiness.

## Verifying

Engineering conclusions are validated using observable evidence.

Verification Receipts are generated.

## Recommending

Explainable engineering recommendations are produced.

Recommendations remain proposals.

## Learning

Observed outcomes improve future Runtime reasoning.

Historical Memory remains immutable.

## Completed

The engineering objective has been satisfied.

The Investigation remains searchable.

## Archived

Completed Investigations may be archived while remaining available for Memory and Knowledge.

---

# Investigation Ownership

Every Engineering Object belongs to exactly one primary Investigation.

Knowledge may create relationships between Investigations without merging them.

---

# Relationships

Investigations may:

- reference related Investigations
- share Knowledge
- reuse Verification Receipts
- contribute to organizational Learning

Historical Memory is never merged or rewritten.

---

# Reopening

A Completed Investigation may return to Collecting when significant new observations arrive.

History is preserved.

The lifecycle simply continues.

---

# Runtime Relationship

Every Runtime subsystem contributes to an Investigation:

Connectors → Observations

Observations → Memory

Memory → Knowledge

Knowledge → Evaluation

Evaluation → Verification

Verification → Recommendations

Recommendations → Learning

Learning improves future Investigations.

---

# Architectural Guarantees

Investigations guarantee:

- Every Engineering Object belongs to an Investigation.
- Investigation history remains durable.
- Memory is never lost.
- Lifecycle progression is explainable.
- Learning improves future Investigations without altering historical ones.
- Completed Investigations remain searchable.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

Investigations are Rivora's organizing structure.

Rather than representing a ticket, incident, or conversation, an Investigation is a continuously evolving body of engineering understanding.

By defining a consistent lifecycle, Rivora ensures every engineering problem follows the same observable, explainable, and continuously improving process—from first observation through long-term organizational learning.
