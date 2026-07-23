# Capability Guide (v0.8)

This guide explains how to implement a first-party Rivora Capability so it
participates consistently in the Capability Engineering Loop (RFC-028).

## What a Capability is

A Capability expresses **engineering intent** and domain meaning. It coordinates
Runtime behavior; it does **not** implement engineering reasoning.

For external mutation, Capabilities implement `ExecutionCapability` and are
invoked only by the Runtime after plan, policy, approval, and confirmation.

## Descriptor requirements (v0.8)

Every first-party Capability must expose a complete
`ExecutionCapabilityDescriptor`:

| Field | Purpose |
| --- | --- |
| `capability_id` | Stable unique id |
| `name` | Human-readable name |
| `version` | Contract version |
| `provider` | Provider family (`mock`, `github`, …) |
| `operation` | Primary operation family |
| `risk_level` | ReadOnly / LowRiskWrite / BoundedWrite / … |
| `mutating` | Whether external (or mock) state changes |
| `supported_actions` | Action names the adapter accepts |
| `required_inputs` | Required structured input keys |
| `permissions` | Live permission scopes (names only) |
| `supports_dry_run` | Dry-run / plan validation support |
| `idempotency_behavior` | How duplicates are handled |
| `reversibility` | Rollback / inverse constraints |
| `verification_method` | Independent verification approach |
| `credential_requirements` | Credential names (never values) |
| `target_restrictions` | Allowed targets |
| `failure_semantics` | Partial / uncertain failure behavior |
| `description` | Purpose |
| `output_types` | Canonical result type ids |
| `limitations` | Honest constraints |
| `engineering_loop` | Explicit participation per stage |
| `accepted_input_types` | Canonical routing input type ids |
| `provider_independent` | Routing/contributions use canonical types |

Use `descriptor.is_complete()` / `completeness_gaps()` and
`capability_coverage_report()` to audit.

## Accepted inputs and provider independence

- Routing matches **canonical input type ids** (ObservationKind strings and
  related synthetic types), not human labels or vendor API shapes.
- `provider_independent: true` means matching and contributions consume
  canonical Runtime types after Connector normalization. The adapter may still
  call a provider API for mutation.
- Do not accept raw provider API structures inside Capability routing logic.

## Lifecycle declarations

Declare participation for every stage:

```text
Memory → Evaluation → Verification → Improvement → Learning
```

Allowed values: `Supported`, `NotApplicable`, `Unsupported`, `Deferred`.

Never hide absence behind implicit defaults without documenting and testing them.
v0.8 first-party execution Capabilities use:

| Stage | Participation |
| --- | --- |
| Memory | Supported |
| Evaluation | Supported |
| Verification | Supported |
| Improvement | Deferred |
| Learning | Deferred |

## Typed contributions

Implement `ExecutionCapability::lifecycle_contributions` (or use the shared
default builder). Contributions must:

- Carry provenance / idempotency / evidence refs
- Match declared participation (`Supported` payloads only when declared Supported)
- Never write Memory, Evaluation, Verification, Proposals, or Learning directly

The Runtime validates contributions and applies existing engines.

## Verification, Improvement, Learning

- **Verification** must remain independent of API acceptance.
- **Improvement** must not auto-generate low-quality Proposals just to complete a stage.
- **Learning** requires measured evidence; API success is not Outcome success.

## Persistence and replay

Lifecycle runs use the shared `lifecycle_runs/` store. Replay with the same
idempotency key must not duplicate artifacts. Do not add per-Capability stores.

## CLI and Workspace exposure

Register the Capability through the shared registry used by CLI and Workspace.
Do not invent Capability-specific command families or UI-owned lifecycle state.

Inspect with:

```sh
rivora capability list
rivora capability show <id>
rivora capability coverage
rivora capability route --investigation <ID> --observation <OBS>
rivora capability lifecycle --investigation <ID> --attempt <ATTEMPT>
```

## Tests

At minimum cover:

- Descriptor completeness and unique ids
- Lifecycle participation
- Contribution validation
- Routing accepted inputs
- Dry-run / live isolation
- Independent verification
- Idempotent lifecycle replay
- Architecture gates (no direct loop writes)

## Prohibited behavior

- Connector-style external observation inside a Capability’s reasoning path
- Direct Memory / Evaluation / Verification / Proposal / Learning writes
- Auto-apply Proposals or auto-execute / auto-rollback
- Treating API acceptance as verified success
- Bypassing Runtime orchestration
- String-name-only routing
- Per-Capability persistence formats for lifecycle runs
