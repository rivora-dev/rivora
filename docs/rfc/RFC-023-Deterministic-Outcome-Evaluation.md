# RFC-023: Deterministic Outcome Evaluation

**Status:** Accepted  
**Target Version:** v0.5  
**Depends on:** RFC-008, RFC-009, RFC-020, RFC-021, RFC-022

---

# Purpose

Define deterministic, explainable evaluation of Measured Learning Outcomes from
Proposal expected results, Verification Plans, Implementation Records, and evidence.

No hosted-model dependency, training, probabilistic black-box scoring, or opaque AI ranking.

---

# Inputs

- Exact Proposal revision (success criteria, verification plan, expected impact)
- Implementation Record
- Baseline, post-change, verification, contradiction, and regression evidence links
- Policy thresholds and prior Outcome revisions

---

# Outputs

- Per-expected-result assessments
- Observed-result summary
- Overall classification
- Decomposed confidence with penalties
- Regressions and contradictions
- Unresolved questions
- Verification readiness
- Lesson candidates
- Provenance trace of evaluation steps

---

# Expected Results

Each expected result supports:

- description
- metric or observable
- baseline presence quality
- target or expected direction
- allowed tolerance
- evaluation window status
- evidence requirements
- verification method
- importance / weight
- attribution notes

Result kinds include boolean, numeric threshold, directional improvement, categorical
state, event occurrence, latency/duration, count/frequency, reliability rate, test
result, human assessment, and composite criteria.

Missing baseline reduces confidence and may yield `Inconclusive`; it must not fabricate certainty.

---

# Result Assessment

For each expected result:

```text
Satisfied | PartiallySatisfied | NotSatisfied | Regressed | Inconclusive | NotMeasured | Invalid
```

Each assessment includes reason, evidence references, confidence, baseline comparison,
window status, contradictions, and missing evidence.

---

# Overall Classification Rules

Deterministic policy (inspectable in Runtime):

| Condition | Classification |
| --- | --- |
| Implementation not proven | `NotImplemented` |
| Measurement assumptions invalid | `Invalidated` |
| Material net degradation | `Regressed` |
| Insufficient / contradictory evidence | `Inconclusive` |
| Required expectations not satisfied | `Unsuccessful` |
| Meaningful benefits and harms coexist | `Mixed` |
| Most required expectations satisfied with bounded gaps | `PartiallySuccessful` |
| All required expectations satisfied and no material regression | `Successful` |
| Default before evaluation | `Pending` |

`Mixed`, `Inconclusive`, and `Unsuccessful` are never collapsed.

---

# Materiality

Regressions and contradictions carry severity, scope, confidence, affected expected
result, reversibility, and whether a Proposal guardrail was violated.

Only material regressions force `Regressed` or `Mixed` according to the rules above.

---

# Confidence

Confidence is strength of evidence, not product optimism.

Components (0.0–1.0 qualitative contribution, explained):

- implementation evidence quality
- baseline evidence quality
- post-change evidence quality
- verification completeness
- evidence consistency
- evidence independence
- temporal fit
- attribution clarity
- sample adequacy
- regression coverage

Explicit penalties for missing baseline, missing implementation proof, stale evidence,
narrow sample, conflicting sources, unresolved contradictions, open evaluation window,
changed environment, concurrent unrelated changes, and human-only assertion without support.

A high-confidence unsuccessful Outcome is valid and valuable.

Users can inspect final confidence, components, penalties, included/excluded evidence,
and what would increase confidence.

---

# Causality Language

Default conservative language:

- `ObservedAfterImplementation`
- `CorrelatedWithImplementation`
- `ConsistentWithExpectedMechanism`
- `DirectlyVerified`
- `CausallyProven` (only when evidence truly warrants)

Never overclaim causation from temporal ordering alone.

---

# Regression Analysis

Typed regressions: correctness, reliability, performance, security, cost,
maintainability, developer experience, observability, compatibility, operational
burden, user experience, process throughput, other.

Each regression record includes type, severity, confidence, baseline, observed state,
evidence, affected component, relationship to Proposal, materiality, follow-up, status.

---

# Verification

`evaluate_measured_learning_outcome` moves the Outcome to `Evaluated` (or keeps
`UnderEvaluation` when blocked) with a full evaluation report stored on the revision.

`verify_measured_learning_outcome` requires:

- status `Evaluated`
- non-empty actor and reason
- verification readiness true (or explicit override reason recorded)

Verified revisions are immutable. Corrections create a new revision or superseding Outcome.

---

# Capabilities

- `evaluate_measured_learning_outcome`
- `verify_measured_learning_outcome`
- `compare_measured_learning_outcomes`

---

# Failure Handling

Typed errors for insufficient evidence, open evaluation window, missing implementation,
invalid transition, and authorization. Partial evaluation still records unresolved questions.
