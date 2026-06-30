# Adaptive Memory Engine

The Adaptive Memory Engine is the Phase 6 memory builder for Open Rivora.
It lives in `crates/rivora-adaptive` and contains pure, deterministic logic
for proposing candidate memories, recalling similar situations, applying human
feedback, and producing evidence-backed receipts.

It implements the core MVP loop:

```text
Ask -> Explain -> Remember -> Recall
```

## What It Does

- Converts reliability context into `MemoryRecord` values with
  `MemoryStatus::Candidate`.
- Emits `MemoryCandidateCreated` receipts when candidates are proposed.
- Scores memory recall with a transparent deterministic model instead of
  opaque embeddings.
- Returns ranked `RecallResult` values with match scores, confidence,
  evidence references, and human-readable reasons.
- Applies typed `HumanFeedback` to memory records through the existing
  lifecycle helpers: `approve`, `reject`, `correct`, and `add_feedback`.
- Emits memory-operation receipts:
  `MemoryApproved`, `MemoryRejected`, `MemoryCorrected`,
  `RecallResult`, and `HumanFeedbackRecorded`.
- Produces memory-only recommendations such as "remember this",
  "review similar memory", "correct memory", "supersede stale memory",
  "reject low-confidence candidate", and "request more evidence".

## What It Does Not Do

The engine does not execute remediation, rollback, deployments,
infrastructure changes, long-running agent loops, or autonomous production
actions. It has no network, disk, connector, storage, or inference I/O. All
inputs are supplied by callers, and all outputs are memory records, recall
results, recommendations, and receipts.

The engine may recommend memory actions only. It cannot recommend direct
infrastructure mutation in Phase 6.

## Candidate Generation

`MemoryCandidateRequest` accepts service context, symptoms, event summary,
evidence ids, source details, scope, kind, confidence, and timestamps. The
engine validates that the candidate has evidence and creates a
`MemoryRecord` with:

- status `Candidate`,
- source/provenance fields populated from the request,
- graph/evidence references attached to the memory,
- service and symptom tags for later recall,
- a `MemoryCandidateCreated` receipt.

Candidates are never approved automatically. A human must apply feedback
before a candidate becomes active memory.

## Recall Matching

`RecallQuery` is matched against a supplied slice of `MemoryRecord` values or
snapshot records. By default, only `Active` memories are recalled. Candidate
memories can be surfaced explicitly for review with `include_candidates`.

The v0.1 scoring method is `deterministic-memory-recall-v1`. It scores:

- same service,
- same memory kind,
- same memory scope,
- overlapping symptoms and tags,
- overlapping evidence references,
- same source,
- overlapping title/body text,
- recallable status.

Every `RecallMatch` includes the memory id, numeric score, confidence,
matched reasons, evidence references, and the matched record. Low or empty
matches are safe: the engine returns an empty match list plus a valid
`RecallResult` receipt explaining that no memory exceeded the threshold.

## Human Feedback

Feedback is the engine's learning signal. The engine accepts existing
`HumanFeedback` values and applies them to a memory record:

| Feedback | Memory effect | Receipts |
|---|---|---|
| `Approved` | `Candidate` -> `Active` | `HumanFeedbackRecorded`, `MemoryApproved` |
| `Rejected` | `Candidate` -> `Rejected` | `HumanFeedbackRecorded`, `MemoryRejected` |
| `Corrected` / wrong-* | memory -> `Corrected` | `HumanFeedbackRecorded`, `MemoryCorrected` |
| `Useful` | raises confidence | `HumanFeedbackRecorded` |
| `NotUseful` | lowers confidence | `HumanFeedbackRecorded` |
| `NeedsMoreEvidence` | lowers confidence and labels review need | `HumanFeedbackRecorded` |

Rejected and corrected memories remain auditable. They are not deleted.

## Ask -> Explain -> Remember -> Recall

- **Ask:** CLI and Slack will pass a question or event context into future
  explain/remember flows.
- **Explain:** The engine provides transparent recall reasons and receipts;
  future surfaces can render those receipts to humans.
- **Remember:** New learning is captured as a candidate memory with evidence.
- **Recall:** Future questions or incidents can retrieve similar active
  memories with deterministic scoring and evidence references.

## Safety Boundary

`rivora-adaptive` is intentionally a no-I/O crate. It cannot mutate
infrastructure because it has no connector or execution capability. It also
does not persist memory directly; storage remains behind later local-first
storage surfaces.

Receipts produced by the engine contain no mutating suggested actions. Memory
recommendations convert only to read-only receipt actions.

## Future CLI and Slack Use

The CLI memory interface will call the engine for local `remember`, `recall`,
and candidate review commands. Slack will call the same engine for team
review, approval, rejection, correction, and recall flows.

Both surfaces should treat engine output as proposals for engineer review.
They should store receipts and feedback through the existing memory and
storage layers, but they should not add an infrastructure execution path to
the adaptive memory engine.

Related: [04-Architecture.md](04-Architecture.md) ·
[05-Adaptive-Engine.md](05-Adaptive-Engine.md) ·
[08-Context-Memory.md](08-Context-Memory.md) ·
[12-Reliability-Receipts.md](12-Reliability-Receipts.md) ·
[MEMORY_MODEL.md](MEMORY_MODEL.md) ·
[18-Roadmap.md](18-Roadmap.md)
