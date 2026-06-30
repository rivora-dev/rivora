# Slack App

> Slack is the primary reliability memory interface for the team. Phase 9
> implements the pure v0.1 Slack surface in `crates/rivora-slack`. Phase 15
> hardening: setup validation, runtime resilience, interaction handling.

Open Rivora is **adaptive reliability memory**, and Slack is where the
engineering team lives the memory loop together. Where the
[CLI](13-CLI-UX.md) serves the individual engineer, Slack serves the team:
shared memory, collaborative review, and explicit human feedback on every
candidate, correction, and recall.

This document supersedes the earlier Slack surface described in
[19-Slack-Integration.md](19-Slack-Integration.md), which framed Slack as a
recommendation-preview and approval surface. Under the memory-first thesis
([ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md)), Slack is the
**primary team memory interface** — not a secondary approval channel.

> **Status:** Phase 9 pure surface implemented. Phase 12 added a self-hosted
> adapter boundary and non-network dev mode in `rivora-cli`; Phase 14 adds live
> app-mention delivery over Socket Mode. OAuth, hosted deployment, interactive
> button delivery, and autonomous action execution remain out of scope. See
> [SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md).

---

## Why Slack

Engineers already live in Slack. Incidents happen in Slack. Decisions get made
in Slack threads. On-call handoffs happen in Slack. Memory that lives where the
team already works is memory that gets used. Memory that lives in a separate
dashboard rots.

Slack is where the team can:

- **ask** a question and get a memory-backed answer,
- **recall** what happened last time something looked like this,
- **remember** a lesson from a conversation before it scrolls away,
- **correct** a memory that is wrong, without leaving the channel,
- **approve or reject** candidate memories the engine proposes,
- and get a **daily digest** of what was learned, corrected, and recalled.

---

## Core Interactions

The v0.1 implementation supports deterministic natural-language-ish mention
inputs. It does not call an LLM.

| Mention input | Behavior |
|---|---|
| `what changed` | Returns an observation response explaining that connector-backed change data is not wired yet, and points the team toward recall or candidate creation. |
| `have we seen this before` | Builds a `RecallQuery`, calls `AdaptiveMemoryEngine::recall`, and renders similar memory cards. |
| `recall <service/topic>` | Builds a recall query from the supplied service/topic and renders ranked matches. |
| `what should we remember` | Builds a `MemoryCandidateRequest`, calls candidate generation, and renders a candidate card for review. |
| anything else | Returns concise help. |

### Ask

An engineer asks a question. Rivora explains, grounded in recalled memory and
evidence, and posts a compact receipt with a link to the full record.

```
/rivora ask why did the billing deployment fail last night?
```

Response: a memory-backed explanation with confidence, evidence count, and a
receipt link. If no relevant memory exists, Rivora says so honestly rather than
guessing.

### Recall

Surface relevant past memories for a query or situation.

```
/rivora recall billing cpu spike
```

Response: a ranked list of matching memories (active first), each with score,
confidence, match reasons, and evidence references. Recall results are
receipt-backed (`RecallResult`) for reproducibility.

### Remember

Capture a new memory from a Slack conversation before it is lost.

```
/rivora remember we rolled back the payments deploy because of the DB migration conflict; do not ship them together
```

This creates a **candidate** memory with provenance (the engineer, the channel,
the timestamp). It is not active until reviewed. A `MemoryCandidateCreated`
receipt is posted.

In v0.1, `what should we remember` uses available Slack request context and
evidence ids supplied by the caller. If no evidence ids are supplied, the pure
surface creates a deterministic Slack thread reference such as
`slack:<channel>:<timestamp>` so candidate generation remains evidence-backed.

### Correct

Fix a wrong memory directly from Slack.

```
/rivora correct mem_01HX... the real cause was the connection pool, not the migration
```

This transitions the original to `Corrected`, creates a `Correction` record, and
posts a `MemoryCorrected` receipt. The original is preserved for audit; it is
never deleted.

### Approve / Reject

Review engine-proposed candidate memories.

```
/rivora approve mem_01HX...
/rivora reject mem_01HX... cause is wrong, it was the load balancer
```

`approve` transitions the candidate to `Active` and posts a `MemoryApproved`
receipt. `reject` transitions it to `Rejected` (preserved) and posts a
`MemoryRejected` receipt with the reason.

### Daily memory digest

A scheduled daily summary posted to a configured channel:

- **Learned** — new candidate and approved memories.
- **Corrected** — memories that were corrected and why.
- **Recalled** — what was recalled and whether it was useful.

The digest is read-only and built from the same canonical receipts used by the
CLI.

---

## Slack Commands (conceptual)

These command shapes remain conceptual. The implemented Phase 9 contract is the
typed mention/action surface in `rivora-slack`.

| Command | Purpose | Receipt kind |
|---|---|---|
| `/rivora ask <question>` | Ask a question; get a memory-backed explanation. | `explanation` |
| `/rivora recall <query>` | Recall relevant past memories. | `RecallResult` |
| `/rivora remember <content>` | Capture a candidate memory. | `MemoryCandidateCreated` |
| `/rivora correct <memory-id> <correction>` | Correct a wrong memory. | `MemoryCorrected` |
| `/rivora approve <memory-id>` | Approve a candidate. | `MemoryApproved` |
| `/rivora reject <memory-id> <reason>` | Reject a candidate. | `MemoryRejected` |

---

## Implemented Types

`crates/rivora-slack` exposes typed contracts for the pure surface:

- `SlackMentionRequest`
- `SlackMemoryAnswer`
- `SlackRecallCard`
- `SlackMemoryCandidateCard`
- `SlackFeedbackAction`
- `SlackActionResponse`
- `SlackReliabilityMemoryApp`

The crate renders calm, compact memory cards with labels such as
`Observation`, `Similar memory`, `Memory candidate`, `Recommendation`, and
`Needs review`.

## Action Handling

Slack actions map to typed memory feedback:

| Slack action | `HumanFeedback` kind |
|---|---|
| Remember / approve | `Approved` |
| Reject | `Rejected` |
| Correct | `Corrected` |
| Not useful | `NotUseful` |
| Needs more evidence | `NeedsMoreEvidence` |

Action handlers call `AdaptiveMemoryEngine::apply_feedback` and return updated
card state plus generated receipts. They only propose or update memory.

---

## Every Interaction Produces a Receipt

There is no Slack interaction that does not produce a receipt. Asking,
recalling, remembering, correcting, approving, rejecting — all are
receipt-backed and reproducible from a stored memory snapshot. This is the
"explain everything" principle ([ADR-0009](adr/0009-explain-everything.md))
applied to the team surface.

Slack renders a compact form of the canonical receipt (see
[12-Reliability-Receipts.md](12-Reliability-Receipts.md)):

```
*<title>* — confidence 87% (evidence: 14 refs)
<summary>
[Approve] [Reject] [Correct]  ·  full receipt: rivora recall --json --snapshot <id>
```

---

## Safety Boundary

- Slack cannot mutate infrastructure. Approvals in Slack authorize Rivora to
  *remember* a decision, not to act on infrastructure. Action execution is
  deferred (see [PRINCIPLES.md](PRINCIPLES.md)).
- Phase 9 has no Slack network client, no event receiver, no OAuth, no request
  signing verification, and no persistence. It is a pure contract/rendering
  crate wired to the Adaptive Memory Engine.
- It does not build connectors, the CLI, Ability Runtime, autonomous
  remediation, rollback execution, deployment execution, infrastructure
  mutation, or long-running agent loops.
- Credentials for Slack live in the operator's secret store; never in memory,
  never logged.
- Slack integration is **opt-in** and feature-gated; the CLI works fully
  without it.
- Every approve/reject/correct is recorded with identity, timestamp, scope, and
  reason.

---

## Relationship to the CLI

The CLI ([13-CLI-UX.md](13-CLI-UX.md)) and Slack share the same canonical
receipts and the same local memory. Slack is the team surface; the CLI is the
individual engineer surface. Both read from and write to the same
organization-owned, local-first memory.

## Self-Hosted Adapter

Open-source users can create their own Slack app from
`examples/slack-app-manifest.yaml` and run local/dev routing with:

```bash
rivora slack doctor          # validate setup (tokens, secrets, permissions)
rivora slack dev --text "what changed?"
rivora slack socket
```

`rivora slack doctor` performs setup validation: it checks that required tokens
and secrets are present, validates their format, and reports missing or
misconfigured state with actionable guidance before attempting a live connection.

`rivora slack dev` exercises the local adapter path without Slack network
access. `rivora slack socket` opens a live self-hosted connection, handles
`app_mention` envelopes, and sends plain-text thread replies. Tokens are read
from `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_SIGNING_SECRET`; they are
never stored in `.rivora/`.

---

## Testing Strategy

- Mention parsing tests for every supported v0.1 phrase.
- Fallback help response tests.
- Recall card rendering tests including match reasons, confidence, score, and
  evidence references.
- Empty recall tests that render safely without guessing.
- Memory candidate card tests.
- Feedback mapping tests from Slack action to `HumanFeedback` kind.
- Safety tests asserting Slack never renders infrastructure mutation actions
  and actions only propose or update memory.
- Doctor command tests covering setup validation, missing-token guidance,
  and malformed-secret detection.
- Envelope hardening tests for malformed JSON, missing required fields,
  and duplicate envelope deduplication.
- Setup guidance tests for different missing states (no bot token, no app
  token, no signing secret, partial configuration).
- Token redaction tests ensuring secrets never appear in logs, receipts,
  or card output.
- Safety tests for forbidden mutation language in rendered cards and
  confirmation messages.

---

## Status

Phase 9 pure surface is implemented in `crates/rivora-slack`. Phase 12 added
self-hosted adapter/dev-mode routing in `rivora-cli`; Phase 14 adds live Socket
Mode app-mention delivery while keeping interactive actions deferred. Phase 15
adds setup validation via `rivora slack doctor`, runtime resilience for
malformed envelopes, hardened interaction handling, and documentation hardening.
See [SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md) and
[18-Roadmap.md](18-Roadmap.md).

---

Related: [ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md) ·
[ADR-0008](adr/0008-slack-primary-interface.md) ·
[MEMORY_MODEL.md](MEMORY_MODEL.md) ·
[PRINCIPLES.md](PRINCIPLES.md) ·
[13-CLI-UX.md](13-CLI-UX.md) ·
[12-Reliability-Receipts.md](12-Reliability-Receipts.md) ·
[SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md) ·
[19-Slack-Integration.md](19-Slack-Integration.md)
