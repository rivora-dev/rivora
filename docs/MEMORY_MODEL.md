# Memory Model

> Rivora is adaptive reliability memory. This document defines how memory is
> structured, how it lives, how humans shape it, and how every operation is
> receipt-backed.

This is the canonical memory model for Open Rivora, extending the typed model
from [ADR-0015](adr/0015-context-memory.md) and the alignment in
[ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md). It does not
duplicate the Rust type definitions; it describes the semantics.

---

## The Memory Loop

Rivora's core MVP loop is **Ask → Explain → Remember → Recall**.

```
   Ask ─────▶ Explain ─────▶ Remember ─────▶ Recall
     ▲                                          │
     └──────────────────────────────────────────┘
```

- **Ask** — an engineer asks a question: "what changed?", "why did this
  incident happen?", "why did we decide to deploy this way?" Asked from Slack
  or the CLI.
- **Explain** — Rivora produces an explanation grounded in recalled memory and
  evidence from the [Context Graph](07-Context-Graph.md), and emits a
  [receipt](12-Reliability-Receipts.md). No explanation without evidence.
- **Remember** — the interaction (or an observation) is captured as a
  *candidate* memory awaiting human feedback. Memory is never created silently
  as truth; it starts as a candidate.
- **Recall** — when a new situation looks similar, relevant past memories are
  surfaced so the team benefits from what was learned. Recall is
  receipt-backed and reproducible.

The loop is continuous. A recall can prompt a new ask. An explanation can
become a memory. A memory can be corrected — and the correction is itself
remembered.

---

## Memory Kinds

Every memory has a `MemoryKind`. The fourteen existing kinds plus
`Correction` (added in Phase 6.5):

| Kind | Records |
|---|---|
| `Pattern` | A recurring behavior ("billing CPU spikes after invoice generation"). |
| `Evidence` | A piece of supporting evidence linked to graph/receipt. |
| `Decision` | A team decision and its rationale. |
| `Approval` | A human approval of a candidate memory or action. |
| `Rejection` | A human rejection of a candidate. |
| `Amendment` | A modification to an existing memory. |
| `Feedback` | Free-form engineer feedback. |
| `AbilityState` | The state of an Ability (see [06-Ability-SDK.md](06-Ability-SDK.md)). |
| `ReceiptRef` | A receipt referenced as a memory. |
| `Snapshot` | A memory snapshot reference. |
| `Observation` | An observed fact carried into memory. |
| `Annotation` | A human annotation on a memory or graph node. |
| `Resolution` | How an incident or issue was resolved. |
| `Correction` | A correction to a prior memory; references the corrected record and the reason. |
| `Custom(String)` | Organization-specific extension. |

`Correction` is the addition from Phase 6.5. It is the first-class record
produced when a memory is corrected; it carries a reference to the record it
corrects and the reason.

---

## Memory Lifecycle

```
                ┌── Approved ──▶ Active
   Candidate ───┤
                └── Rejected ──▶ Rejected

   Draft ────── Approved ──▶ Active

   Active ──── Superseded ──▶ Superseded
   Active ──── Expired ─────▶ Expired
   Active ──── Archived ─────▶ Archived
   Active ──── Invalidated ──▶ Invalidated
   Active ──── Corrected ────▶ Corrected  (+ new Correction record)
```

| Status | Meaning |
|---|---|
| `Draft` | Manually authored, not yet activated. |
| `Candidate` | Proposed by the engine (or imported) and awaiting human feedback. |
| `Active` | Approved and in use. The only status that influences recall by default. |
| `Rejected` | A candidate a human explicitly rejected. Preserved for learning; never deleted. |
| `Corrected` | A memory found wrong and corrected. The `Correction` record supersedes it. |
| `Superseded` | Replaced by a newer memory. |
| `Expired` | Past its retention deadline. |
| `Archived` | Retired but preserved for audit. |
| `Invalidated` | Found incorrect (stronger, legacy form; `Corrected` is preferred going forward). |

### Rules

- Only `Active` memories influence recall by default. `Candidate` memories are
  surfaced for review, not for use.
- `Rejected` and `Corrected` are **terminal-with-audit**: preserved, never
  deleted, always traceable to the feedback that produced them.
- No transition is silent. Every transition is driven by `HumanFeedback` and
  produces a receipt (see [Receipt Integration](#receipt-integration)).

---

## Human Feedback

Human feedback is the primary learning signal. It is a first-class typed value,
not a free-text field.

### FeedbackKind

| Kind | Effect |
|---|---|
| `Approved` | Approves a `Candidate` → transitions it to `Active`. Raises confidence. |
| `Rejected` | Rejects a `Candidate` → transitions it to `Rejected`. Preserved with the rejection reason. |
| `Corrected` | Corrects a memory → transitions it to `Corrected`, creates a `Correction` record, and supersedes the original. |
| `Useful` | Marks an `Active` memory as useful → raises confidence. |
| `NotUseful` | Marks an `Active` memory as not useful → lowers confidence; may trigger review/expiry. |
| `NeedsMoreEvidence` | Flags insufficient evidence → lowers confidence and requests more evidence before activation/use. |
| `WrongCause` | Corrects the *cause* of a memory → produces a `Correction` targeting the causal claim. |
| `WrongService` | Corrects the *service* a memory is scoped to → produces a `Correction` and re-scopes. |
| `WrongTimeWindow` | Corrects the *time window* a memory applies to → produces a `Correction` and re-bounds the record. |

### How feedback drives transitions

```
Candidate + Approved      → Active            (receipt: MemoryApproved)
Candidate + Rejected      → Rejected          (receipt: MemoryRejected)
Active   + Corrected      → Corrected         (receipt: MemoryCorrected, + Correction record)
Active   + Useful         → confidence ↑      (receipt: HumanFeedbackRecorded)
Active   + NotUseful      → confidence ↓      (receipt: HumanFeedbackRecorded)
Active   + NeedsMoreEvidence → confidence ↓, review (receipt: HumanFeedbackRecorded)
Any      + WrongCause / WrongService / WrongTimeWindow → Corrected (+ Correction)
```

Every `HumanFeedback` carries: the target memory id, the author (engineer
identity), a timestamp, a note, and optional corrected content. Every recorded
feedback emits a `HumanFeedbackRecorded` receipt.

---

## Confidence

Confidence is never a bare score. A `MemoryConfidence` is a value in `[0.0,
1.0]` alongside a `method`, an `evidence_count`, an `evidence_span`, and an
`uncertainty` statement (see [05-Adaptive-Engine.md](05-Adaptive-Engine.md) and
[ADR-0015](adr/0015-context-memory.md)).

Feedback adjusts confidence:

- `Approved`, `Useful` → confidence increases.
- `Rejected`, `NotUseful`, `NeedsMoreEvidence` → confidence decreases.
- `Corrected` → the original's confidence is effectively retired; the
  `Correction` record starts with confidence derived from the correcting
  evidence.

Confidence decays over time according to the record's `MemoryDecay` policy
(`None`, `Linear`, `Exponential`, `Step`, `Custom`). The effective confidence at
a given time is computed by `confidence_at`, which applies decay. Stale
memories naturally surface with reduced confidence unless refreshed by new
feedback or evidence.

---

## Retention and Decay

Every memory carries a `MemoryRetention` policy:

| Policy | Behavior |
|---|---|
| `Indefinite` | Kept until explicitly archived. |
| `TimeBound` | Expires at a fixed deadline. |
| `ReviewRequired` | Must be reviewed by a deadline or it is flagged. |
| `DecayBased` | Confidence decays; low-confidence records are candidates for expiry. |
| `EventBound` | Expires when a named event occurs. |
| `Custom(String)` | Organization-specific. |

Retention is evaluated on read (`is_expired_at`, `confidence_at`) rather than by
a background process, preserving auditability. Records are never silently
deleted; expiry and archiving are explicit, audited transitions. `Rejected` and
`Corrected` records are retained indefinitely as audit evidence regardless of
policy.

---

## Provenance

**No memory without provenance.** Every `MemoryRecord` carries a
`MemoryProvenance`: source, optional `receipt_id`, optional `graph_node_id`,
`observed_at`, and a note. This links every learned fact back to the
observation or decision that produced it.

- Memories derived from observations reference a graph node.
- Memories derived from receipts reference the receipt id.
- Memories captured from human input reference the engineer and the feedback
  receipt.

A memory with no provenance is invalid by construction (ADR-0015 validation
rule). This is the "evidence over vibes" principle enforced at the type level.

---

## Receipt Integration

Every memory operation produces a [receipt](12-Reliability-Receipts.md). The
memory-operation receipt kinds (added in Phase 6.5):

| Receipt kind | Produced when |
|---|---|
| `MemoryCandidateCreated` | The engine proposes a new candidate. |
| `MemoryApproved` | A human approves a candidate → `Active`. |
| `MemoryRejected` | A human rejects a candidate → `Rejected`. |
| `MemoryCorrected` | A human corrects a memory → `Corrected` + `Correction` record. |
| `MemorySuperseded` | An `Active` memory is superseded. |
| `RecallResult` | A recall query returns matched memories (reproducible). |
| `HumanFeedbackRecorded` | Any feedback is recorded with provenance. |

Consequences:

- Memory mutations are auditable end-to-end: who proposed, who approved, who
  corrected, and why.
- Recall results are reproducible from a `MemorySnapshot` + query, so "what did
  we know then?" is answerable.
- There is no path to mutate memory without a receipt.

---

## Safety Boundary

Memory is never a control plane. Updating memory does not perform any
infrastructure action. The only way anything becomes an action is via the
explicit human-approval path with audit trail and rollback — and that path is
**deferred** beyond the memory-first MVP. In the memory-first thesis, the
primary output is memory, not actions.

See [PRINCIPLES.md](PRINCIPLES.md) and
[ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md).

---

Related: [ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md) ·
[ADR-0015](adr/0015-context-memory.md) ·
[PRINCIPLES.md](PRINCIPLES.md) ·
[SLACK_APP.md](SLACK_APP.md) ·
[08-Context-Memory.md](08-Context-Memory.md) ·
[05-Adaptive-Engine.md](05-Adaptive-Engine.md) ·
[12-Reliability-Receipts.md](12-Reliability-Receipts.md) ·
[07-Context-Graph.md](07-Context-Graph.md)
