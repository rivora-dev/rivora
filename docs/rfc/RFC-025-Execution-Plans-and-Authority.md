# RFC-025: Execution Plans and Authority

**Status:** Accepted  
**Target Version:** v0.6  
**Depends on:** RFC-004, RFC-011, RFC-014, RFC-020, RFC-022

---

# Purpose

This RFC defines first-class Engineering Objects and authority rules that allow Rivora to
prepare, authorize, and govern controlled external execution without becoming an
unconstrained autonomous agent.

v0.6 answers:

> May Rivora execute an explicitly approved engineering action through a bounded external
> capability while preserving authority, provenance, verification, reversibility, and auditability?

---

# Problem

v0.4 accepts Proposals. v0.5 records external implementations and measures outcomes.
Neither release may mutate an external system.

There is no durable object that:

- converts an accepted Proposal into explicit external actions;
- binds human approval to an exact plan revision;
- evaluates centralized execution policy;
- separates plan creation from approval from execution start.

---

# Non-goals

- Autonomous execution of accepted Proposals
- Multi-user approval workflows
- Scheduled or daemon-based execution
- Autonomous rollback or remediation loops
- Unrestricted shell, Git, cloud, or Kubernetes mutation
- Hosted control planes or credential vaults

---

# Architectural Boundary

```text
Proposal Accepted
  ≠ Execution Plan exists
  ≠ Execution Approved
  ≠ Execution Started
  ≠ Execution Completed
  ≠ Execution Verified
  ≠ Outcome Successful
```

Each transition requires explicit authority. Acceptance never creates or approves an
Execution Plan. An Execution Plan never executes merely because it exists.

---

# Object Model

## Execution Plan

An Execution Plan converts an accepted Proposal into ordered external actions.

### Target snapshot

Execution authority is bound to an immutable `TargetSnapshot`, not to mutable
adapter configuration. The snapshot contains provider, owner, repository,
environment, capability, exact Plan revision, and branch/ref when applicable.
It is persisted with the Plan and copied into the Approval. Runtime compares
the approved snapshot with the adapter's current target immediately before any
mutation. A change to any bound field invalidates approval.

### Required fields

| Field | Meaning |
| --- | --- |
| `id` | Snapshot identifier |
| `lineage_id` | Stable lineage across revisions |
| `revision_number` | One-based revision |
| `parent_plan_id` | Prior immutable snapshot |
| `investigation_id` | Owning Investigation |
| `proposal_id` | Exact Proposal snapshot |
| `proposal_lineage_id` | Proposal lineage |
| `proposal_revision_number` | Proposal revision at plan creation |
| `status` | Lifecycle status |
| `capability_id` | Target capability identifier |
| `target_system` | External system family (e.g. `github`) |
| `target_environment` | Environment label (e.g. `production`, `sandbox`) |
| `target_snapshot` | Immutable normalized execution target and exact Plan revision |
| `actions` | Ordered `ExecutionAction` list |
| `inputs` | Structured action inputs |
| `expected_effects` | Expected external effects |
| `preconditions` | Preconditions that must hold before run |
| `risks` | Declared risks |
| `rollback` | Rollback metadata |
| `verification_plan` | Immediate verification checks |
| `required_authority` | Authority requirements |
| `supports_dry_run` | Whether dry-run is meaningful |
| `idempotency_strategy` | How retries are deduplicated |
| `scope_restrictions` | Explicit scope bounds |
| `transitions` | Preserved lifecycle transitions |
| `provenance` | Actor, source, capability, evidence |
| `created_at` / `updated_at` | Timestamps |

### Lifecycle

```text
Draft
  → ReadyForReview
  → Approved
  → Executing
  → Executed | PartiallyExecuted | Failed
  → Verified
  → Closed

Draft | ReadyForReview → Rejected | Cancelled | Superseded
Approved → Cancelled | Expired | Superseded
Any non-terminal → Superseded (successor required)
```

Invalid transitions are rejected. Status never encodes long-term Outcome success.

### Revisions

Edits create immutable successor snapshots. Prior approvals bind only the exact
`revision_number` they authorized. Changing a Plan invalidates prior approval.

## Execution Approval

| Field | Meaning |
| --- | --- |
| `id` | Approval identifier |
| `plan_id` | Exact Plan snapshot approved |
| `plan_lineage_id` | Plan lineage |
| `plan_revision_number` | Exact Plan revision |
| `actor` | Named approver |
| `reason` | Non-empty reason |
| `scope` | Approved action identifiers |
| `denied_actions` | Explicitly denied actions |
| `environment` | Approved environment |
| `capability_id` | Approved capability |
| `target_snapshot` | Exact immutable target authorized by the approver |
| `policy_decision` | Policy decision at approval time |
| `expires_at` | Optional expiration |
| `one_time` | Whether one-time use |
| `consumed` | Whether already used for execution |
| `invalidated` | Whether invalidated by revision/scope change |
| `provenance` | Approval provenance |
| `created_at` | Timestamp |

### Approval rules

Approval must never be inferred from:

- Proposal acceptance;
- CLI or Workspace invocation alone;
- prior approval of another revision;
- historical success;
- connector availability;
- stored credentials.

Approval becomes invalid if:

- Plan revision changes;
- action inputs change;
- target, capability, environment, or scope expands;
- provider, owner, repository, branch/ref, or any other target-snapshot field changes;
- approval expires;
- one-time approval was already consumed;
- preconditions are no longer satisfied.

CLI or Workspace confirmation cannot substitute for this Runtime-owned target
comparison.

## Execution Policy

Centralized policy evaluation returns:

```text
Allowed
AllowedWithApproval
AllowedDryRunOnly
Denied
```

Policy considers risk level, environment, scope, reversibility, dry-run availability,
preconditions, secrets availability, idempotency, blast radius, verification coverage,
rollback readiness, and prohibited actions.

Policy decisions are not scattered across CLI, Workspace, or connectors.

---

# Capabilities

Runtime-owned capabilities (CLI/Workspace must not reimplement):

- `create_execution_plan`
- `revise_execution_plan`
- `validate_execution_plan`
- `preview_execution_plan` (dry-run / plan validation)
- `approve_execution_plan`
- `reject_execution_plan`
- `cancel_execution_plan`
- `list_execution_plans`
- `get_execution_plan`
- `list_execution_plan_revisions`
- `explain_execution_policy`
- `trace_execution`
- `export_execution_plan`

Execution start, receipts, and verification are defined in RFC-027.

---

# Storage

```text
investigations/{id}/execution_plans/{snapshot_id}.json
investigations/{id}/execution_approvals/{approval_id}.json
```

Lazy directories, atomic write (temp + rename), missing directory = empty list,
corruption isolation on list. Additive only; no migration of prior objects.

---

# Compatibility

- v0.1–v0.5 objects load unchanged.
- Proposal acceptance remains independent of execution.
- Implementation Records and Measured Outcomes remain distinct and optional after verification.

---

# Security

- Named actor and reason required for approval and consequential transitions.
- Exact Plan revision binding is mandatory.
- No automatic execution.
- Secrets never appear in durable Plans, Approvals, or exports.

---

# Success criteria

1. Execution Plan exists as a first-class object with immutable revisions.
2. Approval binds exact Plan revision, scope, and immutable target snapshot.
3. Runtime rejects target drift before external mutation.
4. Policy is centralized and explainable.
5. Accepted Proposals never auto-execute.
6. CLI and Workspace only call shared Capabilities.
