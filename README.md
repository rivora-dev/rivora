# Rivora

Rivora is an open-source workspace for understanding engineering systems.

It helps your existing tools like GitHub, CI, observability platforms, infrastructure, and coding agents work together through durable investigations, shared context, evidence-backed conclusions, and controlled execution.

Rivora does not replace your engineering tools. It helps them operate as one engineering system.

## Install

```bash
curl -fsSL https://rivora.dev/install | sh
```

Then run:

```bash
rivora
```

This opens the Unified Workspace.

Use plain English to investigate your software, or press `/` to discover available actions.

```bash
rivora --help
rivora --version
rivora <command>
```

The installer supports macOS and Linux and installs both `rivora` and the compatibility `rivora-workspace` binary.

## What you can do

With Rivora, you can:

- create and explore engineering investigations
- collect observations from local projects, GitHub, CI, Kubernetes, and Sentry
- build durable engineering memory
- evaluate evidence and verify conclusions
- generate improvement proposals
- review controlled execution plans
- inspect receipts and measured outcomes
- search prior investigations, patterns, and trends
- hand structured work to coding agents without granting hidden authority

## How it works

```text
Workspace / CLI
      ↓
Capabilities
      ↓
Runtime
      ↓
Engineering Objects
      ↓
Local Store
```

The Runtime is the source of truth.

Conversation is an interface over typed engineering objects—not the persistence model and not an authority boundary.

Proposal acceptance never starts execution. External mutation requires an explicit Execution Plan, exact-revision approval, and centralized policy.

## Quick start

Open the Workspace:

```bash
rivora
```

Try:

```text
Investigate why the latest deployment failed.
```

```text
Show me investigations related to Kubernetes.
```

```text
Verify the strongest conclusion in this investigation.
```

```text
Create an execution plan for the accepted proposal.
```

Press `/` to browse actions or `Ctrl+P` to open the global command palette.

For one-shot CLI workflows:

```bash
rivora investigation create "CI failure on main"
rivora observe --investigation <ID> --local .
rivora evaluate --investigation <ID>
rivora verify --investigation <ID>
rivora proposal generate --investigation <ID>
```

## Documentation

- [Workspace guide](docs/guides/WORKSPACE.md)
- [Installation](docs/guides/INSTALL.md)
- [Capability guide](docs/guides/CAPABILITY_GUIDE.md)
- [Connector guide](docs/guides/CONNECTOR_GUIDE.md)
- [Architecture and RFCs](docs/rfc/)
- [Roadmap](ROADMAP.md)
- [Changelog](CHANGELOG.md)

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo build --workspace --release
```

See `.agents/skills/build-rivora/SKILL.md` for Rivora’s engineering workflow.

## Status

Rivora is under active development.

The current release is `v0.10.0 — Unified Workspace`.

## License

Apache-2.0. See [LICENSE](LICENSE).
