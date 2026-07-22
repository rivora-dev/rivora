# RFC-002: Architecture

**Status:** Draft (Foundational)  
**Target Version:** Foundation → v0.1

---

# Purpose

This RFC defines Rivora's high-level architecture.

- **RFC-000** explains why Rivora exists.
- **RFC-001** defines the engineering principles that guide decisions.
- **RFC-002** defines the architectural foundation of Rivora.

This document establishes the long-term separation between the Runtime, its interfaces, and the engineering lifecycle.

---

# Architectural Philosophy

Rivora does not replace engineering tools.

It helps them work together as one engineering system.

External tools remain systems of record.

Rivora owns engineering understanding.

---

# High-Level Architecture

```text
                Humans
                   │
                   ▼
      Workspace (Primary Experience)

                   │
                   ▼
             Rivora Runtime

        ▲                     ▲
        │                     │
 CLI (Execution)      APIs / MCP / SDKs
        │                     │
        ▼                     ▼
 Humans, Scripts,      Coding Agents,
 CI, Automation         IDEs, Integrations

                   │
                   ▼
       External Engineering Systems
```

The Runtime is the heart of Rivora.

Everything else is an interface or integration.

---

# Architectural Layers

## Layer 1 — External Engineering Systems

Examples include:

- GitHub
- CI/CD
- Kubernetes
- Cloud providers
- Observability platforms
- AI coding agents
- Documentation systems
- Communication tools

These remain the authoritative systems of record.

---

## Layer 2 — Rivora Runtime

The Runtime owns all engineering understanding.

Responsibilities include:

- Observation ingestion
- Memory
- Knowledge
- Evaluation
- Verification
- Improvement
- Learning
- Capability execution

All core business logic lives here.

---

## Layer 3 — Interfaces

Interfaces expose Runtime capabilities without owning business logic.

### Workspace

The primary interactive experience for engineers.

Optimized for:

- investigations
- conversations
- context
- long-running engineering work
- understanding

### CLI

A fast, stateless execution surface.

Optimized for:

- one-shot commands
- scripting
- automation
- shell workflows
- humans and coding agents

### Future Interfaces

- REST / gRPC APIs
- MCP Servers
- IDE Extensions
- Desktop Applications
- Web UI

Every interface communicates with the same Runtime.

---

# Engineering Lifecycle

Every engineering observation follows the same lifecycle.

```text
Observation
      ↓
Memory
      ↓
Knowledge
      ↓
Evaluation
      ↓
Verification
      ↓
Improvement
      ↓
Learning
      ↺
Future observations are interpreted with improved understanding.
```

This is a continuous engineering lifecycle—not a one-time pipeline.

Memory records history.

Learning improves future interpretation.

Historical memory is never rewritten.

---

# Lifecycle Responsibilities

## Observation

Receive engineering events.

## Memory

Persist immutable engineering facts.

## Knowledge

Derive relationships and context.

## Evaluation

Interpret significance and impact.

## Verification

Validate conclusions with evidence.

## Improvement

Generate recommendations.

## Learning

Measure outcomes and improve future evaluations.

---

# Consumption Models

Rivora supports three complementary ways of being used.

## Interactive

Workspace

Persistent engineering context.

Primary experience for humans.

## Programmatic

CLI

Fast, stateless execution for people and automation.

## Embedded

APIs, MCP, SDKs and integrations.

Allows coding agents and external tools to consume Rivora's engineering understanding without requiring users to open the Workspace.

---

# Dependency Direction

```text
Workspace
CLI
APIs / MCP
      │
      ▼
Rivora Runtime
      │
      ▼
External Engineering Systems
```

Interfaces depend on the Runtime.

The Runtime never depends on interface implementations.

---

# Architectural Guarantees

Rivora guarantees:

- External systems remain systems of record.
- The Runtime owns engineering understanding.
- Interfaces remain thin clients.
- The Workspace is the primary human experience.
- The CLI is optimized for execution and automation.
- Memory is append-only.
- Knowledge is derived from memory.
- Evaluation precedes improvement.
- Recommendations are evidence-backed.
- Learning improves future behavior without modifying historical facts.
- Every interface communicates through the Runtime.

If these guarantees change, this RFC must be updated before implementation.

---

# Summary

Rivora is built around a Runtime that creates shared engineering understanding.

The Workspace is the primary experience for engineers.

The CLI provides fast execution for humans and automation.

Future APIs and integrations allow coding agents and external systems to consume the same Runtime.

Every engineering observation flows through a continuous lifecycle:

**Observation → Memory → Knowledge → Evaluation → Verification → Improvement → Learning**

This architecture keeps Rivora modular, composable, and independent of any single coding agent or interface.
