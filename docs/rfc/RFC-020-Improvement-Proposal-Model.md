# RFC-020: Improvement Proposal Model and Lifecycle

**Status:** Proposed
**Target Version:** v0.4

# Purpose

Rivora v0.3 can recommend a direction. v0.4 must describe a concrete,
evidence-backed candidate change while preserving explicit human control.

This RFC defines the durable **Improvement Proposal** Engineering Object,
its lifecycle, revision history, provenance, and storage boundary.

# Recommendation versus Improvement Proposal

A Recommendation expresses a direction that should be considered. An
Improvement Proposal describes a bounded candidate change, affected scope,
risks, implementation outline, test strategy, Verification Plan, and success
criteria.

Recommendations may be sources for Improvement Proposals, but conversion is
never automatic. An accepted Proposal is not implemented. Implementation is
not a verified outcome. Proposal state never creates a Learning Outcome.

# Improvement Proposal Object

The MVP uses the existing `ObjectId` convention. Each durable snapshot records:

* id and owning Investigation id
* title, summary, rationale, category, status, priority, and confidence
* expected impact and affected components/resources
* supporting and contradicting evidence references, each labeled current or
  historical
* related Hypotheses, Evaluations, Verification Receipts, Recommendations,
  historical Investigations, and Learning Outcomes
* assumptions, constraints, risks, and alternatives
* implementation outline, test strategy, Verification Plan, success criteria,
  and reversibility considerations
* effort category, timestamps, provenance, and generation method
* lineage id, revision number, parent Proposal id, and superseding Proposal id
* immutable status-transition and feedback histories

Explicit bounded enums define category, status, priority, effort, and evidence
scope. Free strings describe domain-specific content but do not replace these
types.

# Lifecycle

Generated Proposals begin `Draft`. Explicitly created candidates without a
validated evidence or source-Recommendation reference also begin `Draft`.
Explicitly created evidence-backed candidates may begin `Proposed`.

The MVP statuses are:

```text
Draft → Proposed → UnderReview → Accepted
                           ├──→ Rejected
                           ├──→ Deferred
                           ├──→ Superseded
                           └──→ Withdrawn
```

Draft and Proposed may also transition directly to Rejected, Deferred,
Superseded, or Withdrawn. Deferred may return to UnderReview. UnderReview may
return to Proposed. Terminal Accepted, Rejected, Superseded, and Withdrawn
states do not transition further. Accepted never means implemented.

Every transition records from, to, actor, non-empty reason, and timestamp.
Only an explicit external caller may request acceptance. No generation,
comparison, ranking, refinement, or Composite Capability may accept a Proposal.

# Revision Model

Proposal content is immutable after creation. Refinement creates a new complete
snapshot with a new ObjectId, the same lineage id, incremented revision number,
and a parent Proposal link. The parent remains readable. Feedback and the
refinement reason are copied into the successor's provenance/history.

Status transitions create a new complete revision rather than overwriting the
previous snapshot. A superseding Proposal is written before a superseding link
revision is created, so the child-to-parent relationship remains recoverable
after partial failure.

Content and lifecycle operations require the unique latest lineage snapshot,
preventing ambiguous branches and duplicate revision numbers. Accepted,
Rejected, Superseded, and Withdrawn content cannot be refined in place. A new
candidate change requires a new Proposal; acceptance never carries silently to
changed content. A manually supplied external implementation reference is the
only inert metadata revision allowed after acceptance.

# Storage

Proposal snapshots live under their owning Investigation:

```text
investigations/{investigation_id}/proposals/{proposal_id}.json
```

The directory is created lazily on first write. Missing storage returns an
empty list. Writes use the existing atomic temp-file/rename convention.
Listings isolate corrupted records, return valid siblings deterministically,
and expose corruption diagnostics rather than silently treating corruption as
valid data.

No Proposal operation modifies Observations, Memory, Knowledge, Evaluations,
Verification Receipts, Recommendations, Learning Outcomes, Recalled Context,
related Investigations, connector sources, or repositories.

# Capabilities

The shared Capability surface includes:

* Create, Get, List, and Explain Improvement Proposal
* Update Proposal Status
* Refine Proposal
* Add Proposal Feedback
* Reject, Defer, Supersede, and Withdraw Proposal
* List Proposal Revisions

Explicit creation can cite validated current evidence—including v0.3 Recalled
Context, Composite workflows, verification suggestions, readiness, risks,
root-cause guidance, and reports—source Recommendations, affected components,
and likely resources. Referenced objects must belong to the owning
Investigation. Deterministic generation remains the primary evidence-backed
path.

Capabilities coordinate Runtime operations. CLI and Workspace never implement
lifecycle or revision rules.

# Interface Requirements

Every interface must display:

> Proposal only — not applied, not implemented, not verified.

There is no Apply action. Acceptance requires explicit confirmation. Rejection,
deferral, supersession, withdrawal, and refinement require a reason.

# Security and Compatibility

Proposal text is sanitized before durable creation and export. Credentials,
tokens, raw environment values, and secret-like fields are redacted. Evidence
is referenced by identifier rather than embedding raw connector payloads.

Existing v0.1-v0.3 types and files remain unchanged. No migration is required;
old stores simply have no Proposal directory.

# Acceptance Criteria

* Proposal and Recommendation remain distinct.
* Proposal and implementation remain distinct.
* Accepted and verified outcome remain distinct.
* Lifecycle actor/reason/time and every historical revision remain durable.
* Missing and partially corrupted Proposal storage loads safely.
* Investigation ownership and evidence provenance are preserved.
* CLI and Workspace use the same Capabilities.
* No Proposal operation applies a change or mutates source Engineering Objects.

# Summary

An Improvement Proposal is a durable, concrete, evidence-backed suggestion.
It preserves history and human decisions while remaining strictly separate
from implementation and outcomes.
