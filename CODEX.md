# CODEX.md

# Open Rivora — AI Contributor Guide (Codex entry)

Welcome to Open Rivora. This file is the entry point for AI coding agents
(such as Codex, Claude Code, Gemini CLI, OpenCode, Goose, Amp, and others).
It points at the **same** canonical documentation as
[AGENTS.md](AGENTS.md). There is exactly one documentation source of truth;
this file does not duplicate it.

---

# Read These First

Before making any code changes, read the following documents in order. Every
link below resolves to a real file in this repository, verified against the
[docs index](docs/README.md).

1. [README.md](README.md)
2. [docs/01-Manifesto.md](docs/01-Manifesto.md)
3. [docs/02-Vision.md](docs/02-Vision.md)
4. [docs/03-PRD.md](docs/03-PRD.md)
5. [docs/04-Architecture.md](docs/04-Architecture.md)
6. [docs/16-Implementation-Plan.md](docs/16-Implementation-Plan.md)
7. [docs/06-Ability-SDK.md](docs/06-Ability-SDK.md)
8. [docs/12-Reliability-Receipts.md](docs/12-Reliability-Receipts.md)

Then read the specifications relevant to your task:

- [docs/05-Adaptive-Engine.md](docs/05-Adaptive-Engine.md)
- [docs/07-Context-Graph.md](docs/07-Context-Graph.md)
- [docs/08-Context-Memory.md](docs/08-Context-Memory.md)
- [docs/09-Connector-SDK.md](docs/09-Connector-SDK.md)
- [docs/10-Inference-Providers.md](docs/10-Inference-Providers.md)
- [docs/11-Storage-Abstractions.md](docs/11-Storage-Abstractions.md)
- [docs/13-CLI-UX.md](docs/13-CLI-UX.md)
- [docs/14-Infrastructure.md](docs/14-Infrastructure.md)
- [docs/15-Engineering-Standards.md](docs/15-Engineering-Standards.md)
- [docs/17-Open-Source-Strategy.md](docs/17-Open-Source-Strategy.md)
- [docs/18-Roadmap.md](docs/18-Roadmap.md)
- [docs/19-Slack-Integration.md](docs/19-Slack-Integration.md)

Architecture decisions live in a single store:
[docs/adr/](docs/adr/) (ADR-0001 through ADR-0016). Do not maintain a second
ADR system.

Do not begin implementation without understanding the product philosophy.

---

# Product Philosophy

Open Rivora is an **Adaptive Reliability Environment**. See
[docs/01-Manifesto.md](docs/01-Manifesto.md) for the authoritative statement.

We are NOT building: autonomous infrastructure, black-box AI, or AI that
replaces engineers.

We ARE building: human-in-the-loop reliability, organization-specific
learning, explainable recommendations, adaptive workflows, trust-first
engineering tools.

Every pull request must reinforce these principles.

---

# Engineering Principles

Prioritize: simplicity, reliability, explainability, safety, testability,
maintainability. See [docs/15-Engineering-Standards.md](docs/15-Engineering-Standards.md)
for the authoritative rules.

Avoid clever implementations that reduce readability.

---

# Human First

Never assume an engineer wants automation. Always prefer:

Observe → Learn → Recommend → Engineer Approves.

Automation is a future capability, not the default.

---

# AI Coding Guidelines

Before writing code: understand the problem, the architecture, and the
surrounding code. Never rewrite working systems unnecessarily. Prefer
extending existing abstractions as defined in
[docs/04-Architecture.md](docs/04-Architecture.md).

---

# Testing

Open Rivora follows Test-Driven Development (TDD): Red → Green → Refactor.
See [docs/15-Engineering-Standards.md](docs/15-Engineering-Standards.md)
for the full testing strategy.

No feature is complete without tests.

---

# Safety

Open Rivora defaults to read-only (ADR-0003). Write operations require
explicit approval (ADR-0002). Every conclusion produces a receipt
(ADR-0009). Never introduce silent mutations. Never hide uncertainty.
Always explain reasoning.

See [SECURITY.md](SECURITY.md).

---

# Documentation

Documentation is part of the product. When code changes, update the
relevant doc in [docs/](docs/). Architecture must never drift from
implementation. The docs index is [docs/README.md](docs/README.md).

---

# Commit Philosophy

Small commits. Small pull requests. One feature at a time. Explain why the
change exists.

---

# AI Behavior

If requirements are ambiguous: stop, ask questions. Do not invent product
behavior. Do not assume business requirements. Do not change philosophy
documents without explicit approval.

---

# Definition of Done

A feature is complete when: tests pass, documentation is updated, receipts
are generated, safety is preserved, human review is possible, and the
feature aligns with the [Manifesto](docs/01-Manifesto.md).

---

# Final Principle

AI should make engineers more capable. Never replace engineering judgment.
That principle is more important than shipping quickly.

---

See also: [AGENTS.md](AGENTS.md) · [CONTRIBUTING.md](CONTRIBUTING.md) ·
[SECURITY.md](SECURITY.md) · [docs/README.md](docs/README.md)