# Principles

> The ten principles that govern Open Rivora. Every decision, document, and line
> of code must conform to these. If anything conflicts, these win — alongside
> the [Manifesto](01-Manifesto.md).

Open Rivora is **adaptive reliability memory**. These principles restate and
sharpen the product philosophy through the memory-first lens established in
[ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md).

---

## 1. Memory beats automation

Rivora's primary output is memory, not actions. We remember what worked, what
didn't, and why — so engineers can recall it when it matters. Automation is a
possible future capability, never the default, never the goal of the MVP.

## 2. Humans in control

All memory state transitions require human feedback. No memory becomes active
without a human approving it. No memory is silently rejected or corrected.
Rivora proposes; engineers decide.

## 3. Read-only before action

Observe first. Never mutate infrastructure. Connectors are read-only by
construction. The world is read into memory; memory is never written into the
world without explicit, audited, reversible human approval.

## 4. Evidence over vibes

No memory without provenance and receipts. Every memory carries a link back to
the observation, decision, or feedback that produced it. No conclusion, no
recall, no recommendation exists without evidence.

## 5. Correction is learning

Rejected and corrected memories are first-class, not errors. A rejected
candidate is evidence of what the team considered and declined. A corrected
memory records what was wrong and what replaced it. Nothing is silently
deleted.

## 6. Team context beats generic best practices

Memory is organization-specific. A practice that works for one team can be
disastrous for another. Rivora learns *your* engineering organization — not the
internet's. Generic best practices are a starting point, never the answer.

## 7. Slack is the primary memory interface

Slack is where the engineering team lives the memory loop: ask, recall,
remember, correct, approve, reject. The CLI is the local engineer interface.
Slack is where memory becomes a shared, team-owned asset.

## 8. Explain everything

Every memory, every recall, every recommendation has a receipt. Receipts are
immutable and reproducible from a memory snapshot. No black boxes. No
mysterious scores. Trust requires transparency.

## 9. Adaptive not autonomous

Rivora learns continuously but does not autonomously operate infrastructure.
Adaptive systems build trust; autonomous systems assume trust. Rivora proposes,
engineers decide. (See [ADR-0010](adr/0010-adaptive-not-autonomous.md).)

## 10. BYO everything

No provider lock-in. No infrastructure lock-in. Bring your own AI, your own
infrastructure, your own storage, your own models. Rivora owns the memory —
never the model, never the infrastructure. (See
[ADR-0005](adr/0005-byo-ai.md), [ADR-0006](adr/0006-byo-infrastructure.md),
[ADR-0007](adr/0007-local-first-memory.md).)

---

Related: [ADR-0016](adr/0016-adaptive-reliability-memory-alignment.md) ·
[01-Manifesto.md](01-Manifesto.md) ·
[MEMORY_MODEL.md](MEMORY_MODEL.md) ·
[SLACK_APP.md](SLACK_APP.md)
