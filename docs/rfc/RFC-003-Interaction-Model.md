# RFC-003: Interaction Model

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines how engineers, coding agents, and external tools interact with Rivora.

- **RFC-000** explains why Rivora exists.
- **RFC-001** defines the engineering principles.
- **RFC-002** defines the architecture.
- **RFC-003** defines the interaction model built on top of that architecture.

The goal is to create one consistent interaction model across every interface.

---

# Interaction Philosophy

Rivora is not a chat application.

Rivora is not a collection of CLI commands.

Rivora is an engineering workspace powered by a Runtime that creates shared engineering understanding.

Every interaction should optimize for understanding instead of command execution.

---

# Engineering Work

The primary abstraction in Rivora is **engineering work**.

Engineering work is organized through **Investigations**.

An Investigation is the fundamental unit of engineering work.

Everything produced by Rivora belongs to an Investigation, including:

- Observations
- Engineering Memory
- Derived Knowledge
- Timelines
- Artifacts
- Graphs
- Evaluations
- Verification Receipts
- Recommendations
- Notes
- Capability Executions

Investigations preserve engineering context across time.

---

# Interaction Models

## Interactive

### Workspace

The Workspace is Rivora's primary experience for engineers.

It is optimized for:

- investigations
- conversational interaction
- long-running engineering work
- persistent context
- engineering understanding

---

## Command

### CLI

The CLI is a stateless execution surface.

It is optimized for:

- one-shot commands
- scripting
- shell workflows
- automation
- humans and coding agents

The CLI executes work and exits.

---

## Embedded

Coding agents may consume Rivora without users opening the Workspace.

Examples include:

- Claude Code
- Codex
- Cursor
- OpenCode
- Future coding agents

These interact with the same Runtime used by human engineers.

---

## Programmatic

Future APIs, SDKs, MCP servers, and integrations expose the Runtime programmatically.

Every interface shares the same engineering understanding.

---

# Intent-First Interaction

Engineers express intent rather than implementation.

Instead of describing steps, they describe outcomes.

Example:

> Investigate this deployment.

The Runtime determines how to fulfill the request.

---

# Context Persistence

Workspace sessions preserve engineering context.

CLI commands remain intentionally stateless.

Persistent context belongs to Investigations rather than individual prompts.

---

# Runtime Responsibility

Every interaction flows through the Runtime.

No interface owns business logic.

Every interface consumes the same engineering understanding.

---

# Interaction Goals

Every interaction should optimize for:

1. Shared understanding
2. Evidence-backed reasoning
3. Minimal cognitive load
4. Progressive disclosure
5. Human control
6. Explainable recommendations

---

# Interaction Guarantees

Rivora guarantees:

- The Workspace remains the primary engineering experience.
- Engineers interact through intent rather than implementation details.
- Investigations preserve engineering context across sessions.
- CLI commands remain stateless.
- Coding agents consume the same Runtime.
- Every recommendation is explainable.
- Interfaces never bypass the Runtime.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

Rivora manages engineering work through Investigations.

The Workspace is the primary interactive environment.

The CLI provides fast execution.

Coding agents and future integrations consume the same Runtime.

Regardless of interface, every interaction contributes to shared engineering understanding.
