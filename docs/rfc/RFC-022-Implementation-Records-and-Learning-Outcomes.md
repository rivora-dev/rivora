# RFC-022: Implementation Records and Learning Outcomes

**Status:** Accepted  
**Target Version:** v0.5  
**Depends on:** RFC-004, RFC-010, RFC-011, RFC-020, RFC-021

---

# Purpose

This RFC defines first-class Engineering Objects that close the feedback loop after an
Improvement Proposal is implemented *outside* Rivora:

1. **Implementation Record** — durable evidence that external work was performed  
2. **Measured Learning Outcome** — durable, auditable conclusion about measured effect  

Together with RFC-023 (evaluation) and RFC-024 (historical patterns), these objects let
Rivora answer whether a Proposal worked without claiming implementation or applying changes.

---

# Problem

v0.4 can accept a Proposal. Acceptance does not mean:

- the change was implemented;
- the expected result occurred;
- the result was verified.

v0.1 Learning Outcomes record Recommendation dispositions only. They are not sufficient
for Proposal-linked measurement, baselines, regressions, or revision-preserving history.

---

# Non-goals

- Autonomous implementation, Git mutation, PR creation, deployment, or external writes
- Training models or online learning
- Claiming acceptance implies implementation or success
- Automatically verifying Outcomes when confidence is high
- Replacing or rewriting historical Recommendation Learning Outcomes

---

# Architectural Boundary

```text
Accepted Proposal
  ≠ Implementation Record
  ≠ Evaluated Measured Learning Outcome
  ≠ Verified Measured Learning Outcome
  ≠ Learning Pattern (RFC-024)
```

Each transition requires explicit evidence and authority.

External implementation remains the responsibility of a human or external coding system.

---

# Object Model

## Implementation Record

An Implementation Record establishes that work associated with a Proposal was performed
outside Rivora. It does **not** prove the work succeeded.

### Required fields

| Field | Meaning |
| --- | --- |
| `id` | Snapshot identifier |
| `lineage_id` | Stable lineage across revisions |
| `revision_number` | One-based revision |
| `parent_record_id` | Prior immutable snapshot |
| `investigation_id` | Owning Investigation |
| `proposal_id` | Exact Proposal snapshot referenced at creation |
| `proposal_lineage_id` | Proposal lineage |
| `proposal_revision_number` | Proposal revision at link time |
| `actor` | Who reported the implementation |
| `source` | Typed source (`HumanDeclared`, `GitCommit`, …) |
| `status` | Lifecycle status |
| `summary` | Human-readable summary |
| `references` | Typed implementation references |
| `implemented_at` | Optional implementation timestamp |
| `observed_files` / `observed_components` | Declared scope |
| `evidence_ids` | Linked evidence object identifiers |
| `transitions` | Preserved lifecycle transitions |
| `provenance` | Actor, source, capability, timestamps |
| `created_at` / `updated_at` | Timestamps |

### Sources

`HumanDeclared`, `GitCommit`, `PullRequest`, `Patch`, `Deployment`,
`ConfigurationChange`, `RunbookExecution`, `ExternalAgent`, `Other`.

### References

Typed variants: commit SHA, PR URL/number, branch, deployment id, build id, incident id,
workflow run, artifact path, external URI, human note.

Network access is never required to create a record.

### Status lifecycle

```text
Reported → EvidenceLinked → ReadyForEvaluation
Reported | EvidenceLinked | ReadyForEvaluation → Withdrawn (reason required)
Any non-terminal → Superseded (successor required)
```

Implementation status never encodes success of the change.

### Revisions

Edits create immutable successor snapshots (same pattern as Improvement Proposals).
Prior revisions are never mutated in place.

## Measured Learning Outcome

A Measured Learning Outcome is the durable conclusion about the measured effect of an
implemented Proposal. It is distinct from the v0.1 Recommendation disposition
`LearningOutcome` stored under `learning/`.

Storage path: `learning_outcomes/`.

### Required fields

| Field | Meaning |
| --- | --- |
| `id` / `lineage_id` / `revision_number` / `parent_outcome_id` | Revision identity |
| `investigation_id` | Owning Investigation |
| `proposal_id` / `proposal_lineage_id` / `proposal_revision_number` | Exact Proposal |
| `implementation_record_id` / `implementation_lineage_id` | Linked implementation |
| `status` | Lifecycle |
| `classification` | Outcome classification |
| `confidence` | Evidence strength |
| `expected_results` | Measurable criteria from Proposal |
| `observed_results` | Observed result summaries |
| `assessments` | Per-expected-result assessments (RFC-023) |
| `regressions` / `contradictions` | Typed findings |
| `unresolved_questions` | Remaining uncertainty |
| `evidence_links` | Typed evidence relationships |
| `verification` | Explicit verification receipt when verified |
| `causal_language` | Conservative causality wording |
| `lessons` | Structured lessons |
| `recommended_follow_up` | Next actions |
| `historical_learning_eligible` | Whether patterns may consume this Outcome |
| `provenance` / timestamps | Audit trail |

### Classification

`Pending`, `Successful`, `PartiallySuccessful`, `Mixed`, `Unsuccessful`, `Regressed`,
`Inconclusive`, `NotImplemented`, `Invalidated`.

`Mixed`, `Inconclusive`, and `Unsuccessful` remain distinct.

### Lifecycle

```text
Draft → EvidenceCollection → UnderEvaluation → Evaluated → Verified → Archived
Draft | EvidenceCollection → Withdrawn (reason)
Any non-terminal → Superseded (successor)
```

### Verification authority

Verification requires an explicit actor and non-empty reason. Automation may recommend
readiness but must not auto-verify solely because confidence is high.

Once verified, that revision is immutable. Corrections create a new revision or
superseding Outcome.

## Outcome Evidence Relationships

Evidence may reuse existing Engineering Objects. Relationships are typed on the Outcome:

`SupportsExpectedResult`, `ContradictsExpectedResult`, `IndicatesRegression`,
`ConfirmsImplementation`, `DisputesImplementation`, `IsBaseline`, `IsPostChange`,
`IsInconclusive`, `IsSuperseded`, `IsDismissed`.

Dismissal records a reason and never deletes evidence.

---

# Storage

Per-Investigation, additive, lazy directories:

```text
investigations/<id>/
  implementations/{snapshot_id}.json
  learning_outcomes/{snapshot_id}.json
```

Properties:

- append-only snapshots for revisions
- missing directories treat as empty
- corruption isolation on list (valid records + diagnostics)
- no migration of v0.1–v0.4 data required

---

# Capabilities

Shared Capabilities (CLI and Workspace must not reimplement reasoning):

- `record_external_implementation`
- `revise_implementation_record`
- `link_implementation_evidence`
- `mark_implementation_ready`
- `create_measured_learning_outcome`
- `collect_outcome_evidence`
- `revise_measured_learning_outcome`
- `list_measured_learning_outcomes` / `show` / `trace`
- lifecycle transitions including withdraw / supersede / verify (verify in RFC-023)

v0.4 `record_external_implementation_reference` remains as inert Proposal metadata and
must not be treated as an Implementation Record.

---

# Compatibility

- Existing `learning/` Recommendation Learning Outcomes continue to load unchanged
- Proposals with optional `external_implementation_reference` remain valid
- New directories may be absent

---

# Security

- No repository, Git, CI, cloud, or tracker mutation
- Untrusted text sanitized for display/export
- Explicit authority for consequential lifecycle transitions

---

# Success Criteria

An engineer can link a Proposal revision to an external implementation, create a Measured
Learning Outcome, attach baseline and post-change evidence, revise both objects, reload
them, inspect provenance, and make only valid lifecycle transitions—without mutating the
engineered system under study.
