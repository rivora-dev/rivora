# RFC-019: Engineering Assistance

**Status:** Implemented  
**Target Version:** v0.3

# Purpose

v0.1 established engineering reasoning. v0.2 established cross-
Investigation memory. v0.3 must answer:

> Can Rivora help?

This RFC defines explainable assistance outputs derived from current and
historical evidence: hypotheses, next-best verification, deployment
readiness, risk forecasts, root-cause guidance, prioritized
Recommendations, and engineering reports.

Assistance is evidence-backed guidance. It is never automatic remediation.

# Philosophy

* Every assistance claim cites evidence.
* Supporting and contradicting evidence are both visible.
* Uncertainty is explicit; high confidence is not fact.
* Historical outcomes inform, never prove, current causality.
* Recommendations remain proposals and are never auto-applied.
* Deterministic MVP baselines are preferred; optional models must stay
  replaceable and explainable.
* Assistance is scoped to an Investigation unless explicitly summarized
  from labeled historical context.

# Assistance Objects

## Hypothesis

A ranked, uncertain statement about what may be happening.

Fields:

* id, investigation_id
* statement
* status: `Proposed`, `Supported`, `Contradicted`, `Verified`,
  `Rejected`, `Inconclusive`
* confidence
* supporting evidence object ids
* contradicting evidence object ids
* related Investigation ids
* derivation method
* verification status summary
* timestamps, provenance, metadata

A Hypothesis is never treated as fact without Verification.

## Verification Suggestion (Next-Best Verification)

A structured suggestion for what to verify next.

Fields:

* id, investigation_id
* claim or hypothesis id
* expected evidence description
* reason it matters
* available method (local inspection, connector collect, re-verify, …)
* estimated confidence impact
* prerequisites
* feasibility (`Feasible`, `Blocked`, `RequiresHuman`)
* confirmation required flag
* supporting evidence ids
* provenance

v0.3 never executes destructive external verification.

## Deployment Readiness Assessment

Explainable readiness for proceed / hold / inspect.

Fields:

* id, investigation_id
* status: `Ready`, `Hold`, `Inspect`, `Unknown`
* confidence
* dimensions (test/CI status, unresolved failures, change scope,
  prior incidents, verification coverage, high-risk hypotheses, …)
* blockers, warnings
* supporting and contradicting evidence ids
* required verification ids / suggestions
* recommendation summary
* provenance

## Risk Forecast

Evidence-backed risk items, not opaque scores.

Each risk item:

* category: regression, deployment, operational, verification,
  evidence_quality, recurrence
* severity, confidence
* supporting evidence ids
* historical comparison note
* mitigation or verification suggestion
* explanation

## Root-Cause Guidance

Ranked probabilistic guidance, not a declared root cause.

Fields:

* leading hypothesis ids
* supporting / contradicting evidence
* related prior Investigations
* prior mitigation outcomes (labeled historical)
* confidence
* recommended verification order
* known gaps
* provenance

## Prioritized Recommendation View

Recommendations remain `Recommendation` objects. Prioritization produces
an inspectable ranking view:

* recommendation id
* rank
* score
* factors (name, weight, contribution, explanation)
* overall explanation

Factors may include evidence strength, verification status, expected
risk reduction, reversibility, prior outcome success, contradiction
level, confidence, urgency.

## Engineering Report

A durable Artifact-style summary generated from Runtime data.

Fields:

* id, investigation_id
* title
* sections (investigation state, evidence, knowledge, evaluations,
  verifications, hypotheses, readiness, risks, recommendations,
  historical context, gaps)
* object references per section
* generated_at, provenance
* markdown or structured body for presentation

Reports are derived snapshots. Regenerating creates a new report;
historical reports remain inspectable.

# Runtime Ownership

All assistance derivation lives in the Runtime.

Capabilities expose:

* Generate Hypotheses
* Recommend Next Verification
* Assess Deployment Readiness
* Forecast Risk
* Generate Root-Cause Guidance
* Prioritize Recommendations
* Generate Engineering Report
* Summarize Investigation State

Composite Capabilities (RFC-018) may coordinate these with Core
Capabilities.

# Evidence Rules

Every assistance output must be traceable to some of:

* Observations / Memory
* Knowledge
* Evaluations
* Verification Receipts
* Recommendations
* Learning Outcomes
* Recalled Context (attached only for influence; suggested may be cited
  as candidate historical context when labeled)
* Related Investigations

Contradicting evidence must not be suppressed.

# Storage

Under the Investigation:

```text
investigations/{id}/
  hypotheses/{object_id}.json
  assistance/
    readiness/{object_id}.json
    risks/{object_id}.json
    verification_suggestions/{object_id}.json
    root_cause/{object_id}.json
    reports/{object_id}.json
```

Prioritization views may be ephemeral Runtime results or stored inside
report/recommendation metadata for MVP; durable storage is preferred
when the ranking must be inspected later.

# Human Control

* Recommendations stay `Proposed`.
* Readiness statuses are guidance, not automated deployment gates.
* Root-cause guidance remains probabilistic.
* Confirmation rules from RFC-018 still apply when assistance steps are
  composed into workflows that alter durable conclusions.

# Out of Scope

* Automatic infrastructure mutation
* Auto-merge, auto-deploy, auto-remediation
* Opaque model-only ranking without factors
* Chat-first agent interface requirement
* Enterprise multi-tenant assistance policies

# Acceptance Criteria

* Hypotheses are ranked with supporting and contradicting evidence
* Next-best verification is explainable
* Deployment readiness is inspectable with blockers and evidence
* Risk forecasts include categories, severity, and mitigations
* Root-cause guidance is probabilistic and evidence-backed
* Recommendations are prioritized with visible factors
* Engineering reports generate from Runtime data
* CLI and Workspace use the same Capabilities
* No external system is modified

# Summary

Engineering Assistance turns understanding into help. Every answer cites
evidence, surfaces uncertainty, and leaves action decisions with humans.
