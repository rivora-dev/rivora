# Launch Checklist

Use this checklist before a public GitHub release or design-partner demo.
Run it from a clean checkout and keep all credentials in the environment.

## Pre-launch checks

- [ ] `git status --short` is empty.
- [ ] `git ls-files .rivora` returns no files and `.gitignore` contains `.rivora/`.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo doc` passes.
- [ ] `cargo package -p rivora-cli --allow-dirty --list` includes every packaged demo fixture.
- [ ] Before publishing crates, `cargo package -p rivora-cli --allow-dirty`
  passes after its internal Rivora dependencies are publishable. This is an
  expected limitation of the source-only local preview.
- [ ] Package metadata, crate descriptions, version, and license files are accurate.

## Manual demo checks

```bash
cargo run -q -p rivora-cli -- demo
cargo run -q -p rivora-cli -- demo --scenario checkout-incident
cargo run -q -p rivora-cli -- demo --scenario release-regression
cargo run -q -p rivora-cli -- demo --scenario workflow-failure
cargo run -q -p rivora-cli -- slack doctor
cargo run -q -p rivora-cli -- slack dev --text "what changed?"
cargo run -q -p rivora-cli -- --help
```

- [ ] Every scenario completes the evidence → candidate → approval → recall loop.
- [ ] Demo and Slack output state that no infrastructure actions were taken.
- [ ] Output makes no root-cause guarantee or autonomous-action claim.
- [ ] `rivora slack doctor` gives calm setup guidance when tokens are absent.

## Installed binary checks

Run from a directory outside the source checkout to catch missing packaged
fixtures:

```bash
cargo install --path crates/rivora-cli
rivora demo
rivora demo --scenario checkout-incident
rivora --help
rivora slack doctor
```

- [ ] The installed demo does not depend on `examples/` or another repository path.
- [ ] `rivora --help` lists demo, init, ingest, evidence, remember, recall,
  feedback, ask, Slack, and status commands.

## Real local workflow checks

Run these in a disposable Git repository because `rivora init` creates a local
`.rivora/` store:

```bash
rivora init
rivora ingest git --repo . --limit 20
rivora ask "what changed?"
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
rivora ask "have we seen checkout latency before?"
rivora status
```

- [ ] Evidence remains distinct from memory until explicit review.
- [ ] Recall uses approved memory and cites evidence.
- [ ] The disposable `.rivora/` store is removed after the check.

## Slack checks

- [ ] [SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md) matches the current app manifest and commands.
- [ ] Environment-variable examples contain empty placeholders only.
- [ ] `rivora slack dev --text "have we seen checkout latency before?"` is calm and evidence-backed.
- [ ] Doctor and error output redact Slack-shaped token values.
- [ ] No Slack channel-history crawling or workspace-wide ingestion is implied.
- [ ] If credentials are available, `rivora slack doctor --live` passes.
- [ ] If credentials are available, `rivora slack socket` receives an app mention and posts a threaded reply.
- [ ] Record whether live Slack was tested; it is optional for a local preview.

## Documentation checks

- [ ] README explains the product, non-goals, memory-first reasoning, safety, and 60-second demo.
- [ ] README commands match `rivora --help` and the implementation.
- [ ] [CHANGELOG.md](../CHANGELOG.md) covers the local preview and known limitations.
- [ ] [docs/README.md](README.md) links to every launch, demo, connector, memory, Slack, principle, and security document.
- [ ] All relative Markdown links resolve.
- [ ] Roadmap marks completed phases accurately and describes provider work as read-only evidence connectors first.
- [ ] No stale “AI SRE,” “autonomous reliability,” self-healing, auto-remediation, or root-cause-guarantee claim remains.
- [ ] Local `.rivora/` references say that the directory is gitignored and must not be committed.

## Security checks

- [ ] No real or example token value appears in tracked files, logs, receipts, fixtures, or `.rivora/`.
- [ ] GitHub ingestion remains read-only and does not persist `GITHUB_TOKEN`.
- [ ] Slack tokens are environment-only, unpersisted, and redacted from diagnostics.
- [ ] Demo fixtures contain no secrets or identifying production data.
- [ ] Rivora does not intentionally ingest secrets or collect telemetry.
- [ ] No connector, CLI, demo, or Slack path performs remediation, rollback,
  deployment, infrastructure mutation, or an autonomous loop.
- [ ] The private security-reporting route in [SECURITY.md](../SECURITY.md) works.

## Push and release steps

Do not run these steps until the release owner explicitly approves them.

- [ ] Re-read README, Slack docs, security posture, roadmap, and changelog.
- [ ] Re-run the installed demo and all quality gates from the release commit.
- [ ] Verify `.rivora/` is absent and no credentials or tokens are tracked.
- [ ] Verify the annotated tag points to the intended release commit.

```bash
git push origin <branch>
git push origin --tags
```

- [ ] Draft GitHub release notes from `CHANGELOG.md`.
- [ ] Create the GitHub release only after reviewing the tag and notes.
- [ ] Re-run the public installation and demo instructions after release.

## Known limitations

- Crates are not published; the local preview installs from source. A full
  `cargo package` cannot resolve unpublished internal Rivora crates yet.
- No AWS, GCP, Azure, Vercel, Cloudflare, Render, or Kubernetes connectors.
- No official Slack Marketplace app or hosted OAuth flow.
- No Slack correction modals.
- No hosted Rivora Cloud.
- No Ability Runtime.
- No autonomous remediation, infrastructure mutation, or long-running agent loops.
