# RFC-008: Evaluation

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines the Evaluation subsystem of the Rivora Runtime.

If Memory answers:

> **What happened?**

And Knowledge answers:

> **What does it mean?**

Evaluation answers:

> **How significant is it?**

Evaluation transforms engineering understanding into evidence-backed engineering judgment.

---

# Philosophy

Evaluation does not create facts.

Evaluation does not guess.

Evaluation interprets engineering understanding using observable evidence.

Its purpose is to help engineers prioritize attention, not replace their judgment.

---

# Responsibilities

The Evaluation subsystem is responsible for:

- Assessing engineering significance
- Measuring risk and confidence
- Evaluating engineering health
- Determining readiness
- Prioritizing engineering work
- Supplying Verification and Recommendations with explainable assessments

Evaluation always operates on Knowledge, never directly on raw observations.

---

# Inputs

Evaluation may use:

- Knowledge
- Engineering Memory
- Investigation context
- Historical outcomes
- Runtime capabilities
- User-defined policies
- Organizational rules

Knowledge remains the primary input.

---

# Outputs

Evaluation may produce:

- Risk assessments
- Confidence scores
- Health assessments
- Readiness assessments
- Severity classifications
- Prioritization guidance

Evaluations are judgments—not immutable facts.

---

# Characteristics

## Evidence-backed

Every evaluation should be explainable through supporting Knowledge and Memory.

## Context-aware

Evaluations consider the Investigation and surrounding engineering context.

## Consistent

Equivalent engineering situations should produce equivalent evaluations.

## Evolvable

Evaluations may improve as Knowledge and Learning evolve.

---

# Runtime Relationship

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

Evaluation is where Rivora begins reasoning about engineering work.

---

# Relationship to Verification

Evaluation proposes an engineering judgment.

Verification determines whether that judgment is supported by sufficient evidence.

An evaluation may be revised if verification uncovers stronger or conflicting evidence.

---

# What Evaluation Does Not Do

Evaluation does not:

- modify Memory
- derive Knowledge
- rewrite history
- execute capabilities
- make final engineering decisions

Those responsibilities belong elsewhere in the Runtime.

---

# Architectural Guarantees

Evaluation guarantees:

- Evaluations are derived from Knowledge.
- Every evaluation is explainable.
- Memory remains the source of truth.
- Human engineers retain final decision-making authority.
- Evaluation produces assessments, not facts.
- Verification validates important conclusions before recommendations.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

Evaluation is Rivora's engineering reasoning layer.

Memory preserves history.

Knowledge builds understanding.

Evaluation assesses significance.

Together they allow Rivora to move from recording engineering activity to helping engineers understand what deserves attention and why, while remaining grounded in evidence and preserving human control.
