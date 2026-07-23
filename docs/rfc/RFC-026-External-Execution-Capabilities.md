# RFC-026: External Execution Capabilities

**Status:** Accepted  
**Target Version:** v0.6  
**Depends on:** RFC-011, RFC-012, RFC-014, RFC-025

---

# Purpose

This RFC defines the typed contract for bounded external write capabilities, risk
classification, dry-run, idempotency, preconditions, and the separation between
read-only observation connectors and execution adapters.

---

# Problem

Connectors today only observe. Execution requires mutation, but mutation must not:

- leak into read-only observation traits;
- be invoked directly from CLI or Workspace;
- bypass Runtime policy, approval, or verification.

---

# Non-goals

- Arbitrary shell execution
- Force-push, branch deletion, merge, repository deletion
- Arbitrary Kubernetes or cloud mutation
- Production credential management systems
- High-risk writes (merge, infrastructure delete) as supported operations

---

# Architectural Separation

```text
ReadCapability / Observation Connector
  — observe and normalize only

ExecutionCapability
  — typed, bounded external mutations
  — never owns policy, lifecycle, or approval
```

Do not add write methods to existing observation connectors without a distinct
execution interface.

Runtime invokes execution only through registered `ExecutionCapability` adapters.
CLI and Workspace never call external APIs for mutation.

---

# Risk Levels

```text
ReadOnly
LowRiskWrite
BoundedWrite
HighRiskWrite
Prohibited
```

v0.6 supports only:

```text
ReadOnly
LowRiskWrite
BoundedWrite
```

`HighRiskWrite` and `Prohibited` are denied by policy even if requested.

Examples:

| Action | Risk |
| --- | --- |
| Add/remove GitHub issue label | LowRiskWrite |
| Post GitHub issue comment | LowRiskWrite |
| Create GitHub issue | BoundedWrite |
| Create draft PR from existing branch | BoundedWrite |
| Dispatch named GitHub Actions workflow | BoundedWrite |
| Merge PR | HighRiskWrite (denied) |
| Force-push / delete repo / delete infrastructure | Prohibited |

---

# Capability Contract

Each execution capability declares:

| Field | Meaning |
| --- | --- |
| `capability_id` | Unique stable identifier |
| `version` | Capability contract version |
| `risk_level` | Declared risk |
| `supported_actions` | Allowed action names |
| `required_inputs` | Input schema description |
| `supports_dry_run` | Meaningful dry-run without mutation |
| `idempotency_behavior` | How duplicates are detected |
| `reversibility` | Whether/how reversible |
| `verification_method` | How immediate state is checked |
| `credential_requirements` | What credentials are needed |
| `target_restrictions` | Allowed targets/environments |
| `failure_semantics` | How failures are reported |

### Invocation interface (conceptual)

```text
descriptor()
dry_run(action, inputs) → DryRunResult (never mutates)
execute(action, inputs, idempotency_key) → ExternalResult
observe_state(query) → ObservedState (for verification)
```

---

# Dry Run

When supported, dry-run returns:

- normalized action and target;
- expected mutation;
- required permissions;
- current state when available;
- predicted resulting state;
- risks and policy decision;
- missing preconditions;
- verification steps;
- rollback options.

Dry-run must never mutate the target system.
When the external system cannot simulate, use plan validation (not false certainty).

---

# Idempotency

Every attempt carries an idempotency key.

Retry safety classification:

```text
Safe
ConditionallySafe
Unsafe
Unknown
```

Unsafe and Unknown retries are refused without a new Plan revision and approval.
No hidden automatic retries.

---

# Preconditions

Before execute, Runtime validates:

- approval present and valid for exact revision;
- target identity and environment;
- required credentials available to the capability adapter;
- scope and expiration;
- idempotency state;
- capability-declared preconditions;
- verification availability.

Failed preconditions block execution and are recorded.

---

# Initial Capabilities (v0.6)

## GitHub Issue Operations

| Capability ID | Actions | Risk |
| --- | --- | --- |
| `github.issue.comment` | `create_comment` | LowRiskWrite |
| `github.issue.label` | `add_label`, `remove_label` | LowRiskWrite |
| `github.issue.create` | `create_issue` | BoundedWrite |

## GitHub Pull Request

| Capability ID | Actions | Risk |
| --- | --- | --- |
| `github.pull_request.create_draft` | `create_draft_pr` | BoundedWrite |

From an already existing branch only. No force-push, merge, or branch deletion.

## GitHub Actions

| Capability ID | Actions | Risk |
| --- | --- | --- |
| `github_actions.workflow_dispatch` | `dispatch_workflow` | BoundedWrite |

Explicitly named workflow only. No workflow definition mutation.

## Mock (tests / local dry adapters)

| Capability ID | Actions | Risk |
| --- | --- | --- |
| `mock.record` | `record_mutation` | LowRiskWrite |

In-process mutable store for tests. Not a production external system.

### Prohibited (denied)

Force-push, branch delete, merge, repository delete, secret modification,
workflow file modification, unrestricted shell, arbitrary kubectl apply/delete.

---

# Credential Boundary

- Credentials are supplied to adapters at registration time (e.g. environment).
- Plans and receipts store only redacted permission requirements, never secrets.
- Missing credentials fail preconditions; they never leak into durable objects.

---

# Capabilities (shared)

- `list_execution_capabilities`
- `show_execution_capability`
- `preview_execution_plan` (uses dry-run when available)

---

# Success criteria

1. Observation connectors remain free of write methods.
2. Execution adapters declare risk, dry-run, idempotency, and verification.
3. Runtime alone invokes adapters.
4. High-risk and prohibited actions are denied by policy.
5. Initial bounded capabilities are fully testable with a mock adapter.
