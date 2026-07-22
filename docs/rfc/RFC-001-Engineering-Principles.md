# RFC-001: Engineering Principles

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

## Purpose

This RFC defines the engineering principles that guide Rivora's architecture and implementation.

RFC-000 explains **why Rivora exists**.

RFC-001 explains **how Rivora should be built**.

---

## Engineering Principle 1

### Rivora doesn't replace engineering tools. It helps them work together as one engineering system.

GitHub remains GitHub.

CI remains CI.

Cloud providers remain cloud providers.

Observability remains observability.

Rivora creates shared understanding across them.

---

## Engineering Principle 2

### Build understanding before automation.

Automation without understanding creates brittle systems.

Rivora first observes, remembers, understands, and evaluates before recommending action.

---

## Engineering Principle 3

### Evidence before intuition.

Every recommendation should be explainable and backed by observable engineering evidence.

---

## Engineering Principle 4

### Memory precedes knowledge.

Memory records what happened.

Knowledge explains what it means.

Knowledge is derived from memory.

---

## Engineering Principle 5

### Humans remain responsible.

AI proposes.

Engineers decide.

---

## Engineering Principle 6

### Engineering memory belongs to the user.

External systems remain systems of record.

Rivora owns relationships, context, and understanding.

---

## Engineering Principle 7

### Design composable systems.

Every subsystem should stand on its own:

- Memory
- Knowledge
- Evaluation
- Verification
- Improvement
- Learning
- Capabilities

---

## Engineering Principle 8

### Learn from outcomes.

Recommendations become valuable only when their outcomes are measured.

Learning closes the feedback loop.

---

## Engineering Principle 9

### Explain before acting.

Every recommendation should answer:

- What happened?
- Why?
- What evidence supports it?
- What should happen next?

---

## Engineering Principle 10

### Simplicity compounds.

Prefer small, composable systems over clever abstractions.

---

## Summary

These principles are architectural constraints.

When evaluating any RFC or pull request, ask:

- Does this strengthen shared engineering understanding?
- Does it integrate instead of replace?
- Is it evidence-backed?
- Is it composable?
- Is it simple?
- Does it preserve user ownership?
