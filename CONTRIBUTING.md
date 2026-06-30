# Contributing

Open Rivora is built collaboratively by engineers and AI.

## How to contribute

- **Read first:** [README.md](README.md), [AGENTS.md](AGENTS.md) (or
  [CODEX.md](CODEX.md) for AI agents), then the
  [documentation index](docs/README.md).
- **Docs-first.** When behavior changes, update the relevant doc in the same
  PR. Architecture must never drift from implementation (see
  [docs/15-Engineering-Standards.md](docs/15-Engineering-Standards.md)).
- **TDD.** Red → Green → Refactor. No production code before a failing
  test. See [docs/15-Engineering-Standards.md](docs/15-Engineering-Standards.md).
- **Small PRs.** One feature at a time. Explain *why* the change exists.
- **Tests required.** Unit, integration, safety (read-only), and receipt
  validation where applicable.
- **Major decisions** go through an ADR in [docs/adr/](docs/adr/) or an RFC
  using [docs/rfc/TEMPLATE.md](docs/rfc/TEMPLATE.md). There is exactly one
  ADR store; do not create a second.
- **Safety first.** Read-only by default. No silent mutations. Every
  recommendation produces a receipt. See [SECURITY.md](SECURITY.md).

Every improvement should make engineering more understandable, more
reliable, and more human.