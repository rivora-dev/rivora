# Security

Rivora is local-first reliability memory. Its connectors observe external
systems using read-only access; the product writes only its local memory store
and never takes infrastructure actions.

## Principles

- Start with evidence, not automation.
- Evidence is not memory until a human approves it.
- Memory operations remain explainable and produce receipts.
- Rivora does not remediate, deploy, roll back, or mutate infrastructure.

## Local-first storage

- All Rivora data lives in `.rivora/` in the current working directory.
- `.rivora/` is gitignored by default.
- Git evidence stays local. GitHub ingestion contacts GitHub only when invoked.
  Live Slack mode exchanges messages with the configured Slack workspace while
  keeping the Rivora memory store local.
- No telemetry is collected.
- `.rivora/` contains local operational data. Do not commit or share it.
- Rivora does not currently encrypt `.rivora/` itself. Protection of this
  directory depends on the host operating system, filesystem permissions, disk
  encryption, and the user's access controls.

## Secrets

- Credentials must be supplied through the environment or another external
  secret store, never through checked-in configuration.
- `GITHUB_TOKEN` is never stored, printed, or written into evidence bodies,
  receipts, or test snapshots. The token is piped to `curl` over stdin and
  `curl` stderr is redacted.
- `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_SIGNING_SECRET` are read
  from the environment and never stored in `.rivora/`. All Slack token values
  are redacted in diagnostic output (`rivora slack doctor`).
- `VERCEL_TOKEN` is never stored in `.rivora/`, never printed, and redacted
  in errors. The Vercel connector is read-only; it uses only `GET` requests
  and never creates, rolls back, or promotes deployments.
- Rivora does not intentionally ingest secrets. Connector evidence can include
  source-authored text such as commit messages or issue bodies, so review source
  content before ingestion and rotate any credential exposed in a source.

## Least privilege

- Git, GitHub, and Vercel connectors are read-only. GitHub API ingestion uses
  only `GET` requests. Vercel API ingestion uses only `GET` requests.
- The Slack adapter uses minimal `app_mentions:read` and `chat:write` bot
  scopes. No channel history ingestion or workspace crawling.
- Use `rivora slack doctor` to validate the self-hosted Slack setup without
  printing token values (see
  [docs/SLACK_SELF_HOSTING.md](docs/SLACK_SELF_HOSTING.md)).

## What Rivora does not do

- Rivora does not execute remediation, rollback, deployment, or
  infrastructure mutation.
- Rivora does not run autonomous agent loops.
- Rivora does not ingest Slack channel history.
- Rivora does not store tokens in `.rivora/`.
- Rivora does not collect telemetry.

## Reporting

Security issues should be reported to the maintainers before public
disclosure. Use the repository's
[private vulnerability reporting form](https://github.com/rivora-dev/rivora/security/advisories/new).
If that form is unavailable, ask a maintainer for a private contact channel
without including vulnerability details in a public issue.

Please do not open a public issue containing vulnerability details. Include
affected versions, reproduction steps, impact, and any suggested mitigation in
the private report. Allow maintainers reasonable time to investigate and
release a fix before public disclosure.

## Supported versions

Security fixes are provided for the latest released version of Rivora.
Pre-release builds and unreleased development branches are supported on a
best-effort basis.

## Related

- [AGENTS.md](AGENTS.md) · [CODEX.md](CODEX.md)
- [docs/01-Manifesto.md](docs/01-Manifesto.md)
- [docs/adr/0002-human-in-the-loop.md](docs/adr/0002-human-in-the-loop.md) ·
  [docs/adr/0003-read-only-default.md](docs/adr/0003-read-only-default.md) ·
  [docs/adr/0009-explain-everything.md](docs/adr/0009-explain-everything.md)
