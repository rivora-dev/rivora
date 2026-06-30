# Launch Demo Script

> Short script for recording a launch demo video.
> Target length: 3–5 minutes.

---

## Opening (10 seconds)

**Narration:** "Open Rivora is adaptive reliability memory for engineering
teams. It learns from your evidence, helps you decide what to remember, and
recalls it when it matters again."

---

## Scene 1: Demo in 60 seconds (60 seconds)

```bash
rivora demo
```

**Narration:** "Rivora starts with evidence, not automation."

>Show evidence ingestion output.

**Narration:** "Evidence is not memory until a human approves it."

>Show memory candidate creation and approval.

**Narration:** "Rivora recalls approved operational memory later."

>Show recall results.

**Narration:** "No infrastructure actions are taken."

---

## Scene 2: Richer scenario (60 seconds)

```bash
rivora demo --scenario checkout-incident
rivora demo --scenario checkout-incident --json
```

**Narration:** "Rivora comes with built-in scenarios that show how memory
works in real engineering situations. The JSON form makes the same loop easy
to validate or capture in tooling."

>Show the full evidence → candidate → approval → recall flow.

**Narration:** "Each scenario is deterministic, local, and token-free."

---

## Scene 3: Local workflow (60 seconds)

```bash
rivora init
rivora ingest git --repo . --limit 20
rivora ask "what changed?"
```

**Narration:** "Rivora ingests your local Git history and helps you
understand what changed."

>Show evidence list.

```bash
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
```

**Narration:** "You decide what becomes memory. Rivora doesn't decide for
you."

---

## Scene 4: Slack (60 seconds)

```bash
rivora slack doctor
rivora slack dev --text "what changed?"
rivora slack dev --text "have we seen checkout latency before?"
```

**Narration:** "Slack is the team interface. CLI is the local engineer
interface. Both use the same memory store."

>Show Slack dev response.

**Narration:** "Rivora is self-hosted. Tokens are not stored, and operational
memory stays in the local Rivora store. Live Slack messages still pass through
your configured Slack workspace."

---

## Closing (10 seconds)

**Narration:** "Open Rivora. Adaptive reliability memory. Human first. Open
forever."

---

## Key beats

- "Rivora starts with evidence, not automation."
- "Evidence is not memory until a human approves it."
- "Rivora recalls approved operational memory later."
- "No infrastructure actions are taken."
- "Slack is the team interface; CLI is the local engineer interface."
- "Tokens are not stored. Operational memory stays local."

---

## Commands reference

```bash
# Demo
rivora demo
rivora demo --scenario checkout-incident
rivora demo --scenario checkout-incident --json
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure

# Local workflow
rivora init
rivora ingest git --repo . --limit 20
rivora ask "what changed?"
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve

# Slack
rivora slack doctor
rivora slack dev --text "what changed?"
rivora slack dev --text "have we seen checkout latency before?"
rivora slack socket
```
