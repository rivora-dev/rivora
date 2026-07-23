# RFC-027: Execution Receipts, Verification, and Failure Handling

**Status:** Accepted  
**Target Version:** v0.6  
**Depends on:** RFC-009, RFC-022, RFC-025, RFC-026

---

# Purpose

This RFC defines Attempts, Receipts, partial failure handling, post-execution
verification, retry safety, rollback metadata, traceability, and integration with
v0.5 Implementation Records and Measured Learning Outcomes.

---

# Problem

An external API success response is not proof that Rivora's expected effect occurred.
Partial failures, unsafe retries, and missing verification would otherwise collapse into
false success.

---

# Non-goals

- Autonomous rollback
- Hidden retries
- Treating verification as long-term Outcome success
- Automatic Outcome creation without explicit linkage steps where required

---

# Architectural Boundary

```text
Execution Attempt
  ≠ Execution Receipt
  ≠ Execution Verification passed
  ≠ Implementation Record
  ≠ Measured Learning Outcome successful
```

A Receipt is evidence of what the external system reported.
Verification independently checks observed state.
v0.5 Learning determines whether the change improved engineering outcomes over time.

---

# Object Model

## Execution Attempt

One attempt to invoke an approved Plan.

The Runtime persists `Started` and the idempotency reservation before invoking
an external mutation:

```text
Started → Persist → Execute → Receipt → Verify
```

On restart, a persisted Started attempt is recovered without silently issuing
the mutation again. A matching retry produces a durable
`DuplicateSuppressed` attempt or resumes only when the capability contract
proves doing so is safe.

| Field | Meaning |
| --- | --- |
| `id` | Attempt identifier |
| `plan_id` / `plan_lineage_id` / `plan_revision_number` | Exact Plan |
| `approval_id` | Approval used |
| `investigation_id` | Owning Investigation |
| `actor` | Who started the attempt |
| `capability_id` / `target_system` / `environment` | Target |
| `status` | Attempt status |
| `requested_actions` | Actions requested |
| `completed_actions` | Successfully completed actions |
| `failed_actions` | Failed actions |
| `skipped_actions` | Skipped after failure or policy |
| `uncertain_actions` | Unknown completion state |
| `idempotency_key` | Deduplication key |
| `retry_safety` | Classified retry safety |
| `errors` | Structured errors |
| `external_references` | External ids/urls |
| `receipt_ids` | Linked receipts |
| `started_at` / `finished_at` | Timestamps |
| `provenance` | Provenance |

### Attempt status

```text
Started
Completed
PartiallyCompleted
Failed
Blocked
DuplicateSuppressed
```

## Execution Receipt

What the external system reports happened.

| Field | Meaning |
| --- | --- |
| `id` | Receipt identifier |
| `attempt_id` | Parent attempt |
| `investigation_id` | Owning Investigation |
| `capability_id` / `target_system` | Target |
| `action_name` | Action performed |
| `request_summary` | Sanitized request summary |
| `response_summary` | Sanitized response summary |
| `changed_resources` | Reported changed resources |
| `unchanged_resources` | Reported unchanged resources |
| `external_identifiers` | External ids |
| `result_status` | Success / Failed / Partial / Uncertain |
| `warnings` | Warnings |
| `rollback_metadata` | How to roll back if available |
| `verification_requirements` | Checks still required |
| `raw_evidence_refs` | Sanitized evidence refs (no secrets) |
| `sanitization` | What was redacted |
| `provenance` | Provenance |
| `created_at` | Timestamp |

A Receipt is never proof of success.

## Execution Verification

Immediate postcondition checks independent of the API success response.

| Field | Meaning |
| --- | --- |
| `id` | Verification identifier |
| `attempt_id` / `receipt_ids` | Linkage |
| `investigation_id` | Owning Investigation |
| `checks` | Named checks |
| `results` | Per-check results |
| `status` | Passed / Failed / Inconclusive |
| `confidence` | Confidence |
| `contradictions` | Observed contradictions |
| `unresolved_risks` | Remaining risks |
| `actor` | Verifier (runtime or external) |
| `evidence` | Supporting evidence refs |
| `provenance` | Provenance |
| `created_at` | Timestamp |
| `revision` | Verification revision |

---

# Partial Failure

Partial completion is first-class:

- completed / failed / skipped / uncertain actions are recorded separately;
- no collapse into success without detail;
- no automatic continuation after material failure unless the Plan defines safe continuation;
- rollback availability and recommended next action are recorded.
- ambiguous transport outcomes are recorded in `uncertain_actions`; a timeout
  is uncertainty, not definite failure.

---

# Rollback

Plans and Receipts store rollback metadata:

- available or not;
- capability and inputs;
- risks;
- verification;
- irreversible effects.

Every reversible capability explicitly defines its inverse action and inputs
per completed action. Runtime never guesses an inverse by choosing an arbitrary
supported action. Rollback metadata produces a separate draft Execution Plan
that must pass normal validation and receive its own approval.

Automatic rollback is out of scope. A user may approve a separate rollback Plan if a
supported capability exists.

---

# Retries

| Classification | Behavior |
| --- | --- |
| Safe | May retry with same idempotency key after explicit user request |
| ConditionallySafe | Retry only when preconditions re-validate |
| Unsafe | Requires new Plan revision and approval |
| Unknown | Treated as Unsafe |

Runtime never auto-retries.

Dry-run and live execution use distinct idempotency namespaces so a preview
cannot suppress a later approved mutation.

# Capability-Specific Verification

Verification is an independent observation, never reuse of mutation-response
fields. v0.6 checks:

- issue comments by exact comment identifier and content;
- labels by final presence or absence;
- created issues by exact identifier and expected fields;
- pull requests by exact identifier and `draft=true`;
- workflow dispatch by workflow, approved ref, dispatch time, and correlated run.

Unrelated observations and non-success Receipts cannot produce a passing
Verification.

---

# Integration with v0.5

After verification, Rivora may create or link:

```text
Execution Plan
  → Execution Attempt
  → Execution Receipt
  → Implementation Record
  → Measured Learning Outcome
```

- Receipts provide structured implementation evidence.
- Implementation Records do not claim Outcome success.
- Measured Outcomes use existing v0.5 evaluation (RFC-023) without duplication.
- Linkage capability: `link_execution_to_implementation`.
- Linkage is idempotent: one execution linkage cannot create duplicate
  Implementation Records.
- `trace_execution` populates both `implementation_record_id` and
  `measured_outcome_id` when those durable objects exist.

---

# Capabilities

- `execute_plan`
- `list_execution_attempts`
- `get_execution_attempt`
- `verify_execution_attempt`
- `record_execution_receipt` (internal/runtime; may be exposed for diagnostics)
- `create_rollback_plan`
- `export_execution_receipt`
- `link_execution_to_implementation`
- `trace_execution`

---

# Storage

```text
investigations/{id}/execution_attempts/{attempt_id}.json
investigations/{id}/execution_receipts/{receipt_id}.json
investigations/{id}/execution_verifications/{verification_id}.json
```

Append-only snapshots, atomic write, corruption isolation, additive paths.

---

# Traceability

A user must be able to trace:

```text
Measured Outcome
  → Implementation Record
  → Execution Receipt
  → Execution Attempt
  → Approval
  → Execution Plan revision
  → Proposal revision
  → Recommendation / evidence
```

Trace includes IDs, revisions, actors, timestamps, policy decisions, approval scope,
capability, external references, requested vs observed actions, verification checks,
errors, retry decisions, and rollback information.

---

# Security

- Sanitize external responses before persistence.
- Never store secrets, tokens, or authorization headers.
- Failed and partial attempts remain visible.
- Verification failure blocks Closed-as-success semantics.

---

# Success criteria

1. Attempts, Receipts, and Verifications are durable and distinct.
2. Partial failure is represented without false success.
3. Verification queries state independently of API success.
4. Idempotency and retry safety are enforced.
5. Rollback is metadata-only unless a separate approved Plan runs.
6. v0.5 linkage works without rewriting historical Outcomes.
7. Started attempts and idempotency reservations survive restart.
8. Ambiguous outcomes remain explicitly uncertain.
9. Rollback plans use only explicit inverse metadata and require separate approval.
