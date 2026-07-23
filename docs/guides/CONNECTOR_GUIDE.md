# Connector Guide (v0.8)

This guide explains how first-party Rivora Connectors must behave so they feed
the Capability Engineering Loop with canonical Runtime inputs (RFC-012, RFC-028).

## Connector responsibilities

Connectors may:

- Authenticate to external systems
- Call provider APIs / local tools
- Paginate and batch collection
- Normalize into Rivora Observations
- Sanitize and redact secrets
- Preserve provenance
- Deliver Observations for Runtime ingestion

Connectors may **not**:

- Evaluate engineering quality
- Verify conclusions
- Recommend or propose changes
- Learn or write Learning artifacts
- Write Memory, Evaluation, Verification, Improvement, or lifecycle runs
- Mutate external systems (observation connectors are read-only)

## Normalization and canonical types

Each Observation must include:

- `ObservationKind` (canonical kind id used for routing)
- Summary (factual, non-judgmental)
- Structured payload
- Source system id
- Observed timestamp
- Optional idempotency key
- Provenance actor/source

Prefer including `canonical_type` in payloads equal to the kind id when helpful.

Do not invent a broad taxonomy without a concrete first-party need. Use existing
kinds such as: repository, pull_request, issue, check_result, workflow_run,
git_status, commit, infrastructure, observability, test_output, local_event.

## Provenance and sanitization

- Provenance actor should identify the connector (e.g. `github-connector`).
- Redact tokens, secrets, passwords, and sensitive blobs before delivery.
- Never log credentials.

## Authentication

- Live mode may require environment credentials (`GITHUB_TOKEN`, etc.).
- Fixture mode must work offline for tests without credentials.
- Missing credentials must fail clearly for live observe; they must not invent data.

## Routing compatibility

Observation kinds map to Capability `accepted_input_types` via stable type ids:

```text
ObservationKind.as_str() → CanonicalInputType → Capability match
```

Zero matches → unsupported. One match → single capability. Many matches →
ambiguous (Runtime does not auto-select). Routing never auto-executes.

## Fixtures

Every first-party connector should support offline fixtures (or a documented
fixture-parity path). Local connector: `observe_from_fixture(root)` uses the
same filesystem layout as live mode.

## First-party connectors (v0.8)

| Id | Provider | Kinds | Fixture |
| --- | --- | --- | --- |
| `local` | local | repository, git_status, commit, changed_files, test_output, local_event | yes |
| `github` | github | repository, pull_request, commit, check_result, issue | yes |
| `github_actions` | github | workflow_run, check_result | yes |
| `kubernetes` | kubernetes | infrastructure | yes |
| `sentry` | sentry | observability | yes |

## Prohibited reasoning examples

Do **not**:

- Map Kubernetes phase → `healthy` / `unhealthy` conclusions
- Treat test log substrings as verified failure outcomes
- Score PR quality or incident severity inside the connector
- Call Runtime evaluation / verification / proposal APIs

Emit raw facts (phase, ready counts, token presence flags). Let Evaluation own
meaning.

## Tests

Cover normalization, provenance, redaction, fixtures, and architecture gates
that forbid connector reasoning and mutation HTTP in observation modules.
