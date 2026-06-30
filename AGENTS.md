# AGENTS.md

# Open Rivora AI Contributor Guide

Welcome to Open Rivora.

This repository is designed to be built collaboratively by both humans and AI coding agents.

Whether you are Codex, Claude Code, Gemini CLI, OpenCode, Goose, Amp, or another AI agent, this document defines how to contribute safely.

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

Open Rivora is an Adaptive Reliability Environment.

We are NOT building:

- Autonomous infrastructure
- Black-box AI
- AI that replaces engineers

We ARE building:

- Human-in-the-loop reliability
- Organization-specific learning
- Explainable recommendations
- Adaptive workflows
- Trust-first engineering tools

Every pull request should reinforce these principles.

---

# Engineering Principles

Prioritize:

- Simplicity
- Reliability
- Explainability
- Safety
- Testability
- Maintainability

Avoid clever implementations that reduce readability.

---

# Human First

Never assume an engineer wants automation.

Always prefer:

Observe

↓

Learn

↓

Recommend

↓

Engineer Approves

Automation is a future capability, not the default.

---

# AI Coding Guidelines

Before writing code:

Understand the problem.

Understand the architecture.

Understand the surrounding code.

Never rewrite working systems unnecessarily.

Prefer extending existing abstractions.

---

# Testing

Open Rivora follows Test-Driven Development (TDD).

Every feature should follow the Red → Green → Refactor workflow.

## Red

Write a failing test that captures the expected behavior.

Do not write production code first.

## Green

Write the minimum amount of code necessary to make the test pass.

Avoid adding unrelated functionality.

## Refactor

Improve readability, structure, and maintainability while keeping all tests passing.

Repeat until the feature is complete.

---

Every feature should include:

- Unit tests
- Integration tests
- Regression tests (when fixing bugs)
- Documentation updates
- Receipt validation
- Error handling

No feature is complete without tests.

Code coverage is useful, but correctness and meaningful behavior are more important than percentage targets.

---

# Safety

Open Rivora defaults to read-only.

Write operations require explicit approval.

Never introduce silent mutations.

Never hide uncertainty.

Always explain reasoning.

---

# Documentation

Documentation is part of the product.

Whenever code changes:

Update relevant documentation.

Architecture should never drift from implementation.

---

# Commit Philosophy

Small commits.

Small pull requests.

One feature at a time.

Explain why the change exists.

---

# AI Behavior

If requirements are ambiguous:

Stop.

Ask questions.

Do not invent product behavior.

Do not assume business requirements.

Do not change philosophy documents without explicit approval.

---

# Definition of Done

A feature is complete when:

- Tests pass
- Documentation is updated
- Receipts are generated
- Safety is preserved
- Human review is possible
- The feature aligns with the Manifesto

---

# Final Principle

AI should make engineers more capable.

Never replace engineering judgment.

That principle is more important than shipping quickly.
