# RFC-029: Unified Workspace

**Status:** Implemented  
**Target Version:** v0.10.0

---

# Purpose

This RFC defines Rivora’s conversation-first Unified Workspace as a presentation
and interaction layer over the existing Runtime, Capabilities, and Engineering
Object Model.

It does **not** replace Runtime authority, Memory, Verification, Execution Plans,
or Capability contracts.

---

# Product Position

```text
OpenCode-like interaction quality
+
Rivora’s engineering understanding Runtime
+
typed Investigations, evidence, evaluation, verification, proposals,
execution, receipts, outcomes, and learning
```

Conversation is the **interface**. Durable engineering understanding remains the
**product**.

---

# Interaction Contract

```text
rivora                 → Unified Workspace (TTY required)
rivora <subcommand>    → one-shot CLI
rivora --help / --version
rivora-workspace       → same Unified Workspace launcher
```

Both binaries share `rivora_workspace::run_workspace`.

---

# Boundaries

```text
User text
→ Intent interpretation
→ typed WorkspaceIntent
→ CapabilityService
→ Runtime
→ typed engineering objects
→ Workspace renderer
```

Rules:

1. Natural language never grants authority.
2. Interpretation and execution are separate.
3. Conversation is a projection, not the persistence model.
4. External mutation still requires Execution Plan + exact-revision approval.
5. Interfaces remain thin; Capabilities coordinate; Runtime reasons.

---

# Typed Intents and Action Registry

All discoverable actions (`/` and `Ctrl+P`) share one action registry.

Each action maps to a typed `WorkspaceIntent`. Disabled actions explain missing
context (for example, Evaluate without an active Investigation).

---

# Natural-Language Interpreter

The interpreter produces:

- typed intent
- confidence
- rationale
- required context
- confirmation flag
- provenance (`deterministic` in v0.10 fixture mode)

It must not call storage, connectors, or execution adapters.

Low confidence → clarification.  
“Apply/run this fix” → proposal/plan review, never direct execution.  
Injection language → safe refusal.

---

# Application Architecture

```text
terminal event
→ input mapper
→ WorkspaceIntent or local UI action
→ application update
→ optional Capability effect
→ typed result
→ application state update
→ render
```

Full-screen terminal UI (Ratatui + Crossterm) with panic-safe terminal restore.

Background tasks use generation counters so stale results cannot overwrite newer
context.

---

# Conversation Model

`WorkspaceMessage` references durable object ids rather than duplicating domain
state. Secrets and terminal control characters are sanitized.

---

# Persistence

Workspace UI state (`workspace_ui_state.json`) is additive, versioned, and
corruption-isolated. Missing or corrupt UI state must not block Runtime data.

---

# Coding-Agent Handoff

Handoffs are typed previews from Capability
`generate_coding_agent_handoff`. They exclude secrets and never auto-execute.

---

# Explicit Non-Goals

Web/desktop apps, hosted control planes, autonomous external mutation, and
generic chatbot persistence are out of scope.

---

# Compatibility

v0.1–v0.9 Runtime data remains compatible. Explicit CLI commands are preserved.
