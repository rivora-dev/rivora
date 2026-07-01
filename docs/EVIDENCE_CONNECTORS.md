# Evidence Connectors

> Connectors feed evidence into Rivora's memory loop. Evidence is not memory
> until an engineer chooses to remember it.

Phase 10 added `crates/rivora-connectors` and started with local Git history.
Phase 11 adds a read-only GitHub connector. Phase 13 adds deterministic local
fixture ingestion for demos and tests. Together they make Rivora useful
without cloud credentials, hosted services, or connector secrets beyond an
optional GitHub token.

## What Connectors Do

- Read engineering evidence from local and remote sources.
- Normalize that evidence into serializable `EvidenceItem` values.
- Preserve provenance, confidence, timestamps, authors, changed files, and
  inferred topics where available.
- Store evidence locally for later recall, review, and candidate memory
  creation.

## What Connectors Do Not Do

Connectors do not execute remediation, rollback, deployment, infrastructure
mutation, long-running agent loops, or autonomous production actions.

The Phase 10 Git connector is read-only and does not run mutating Git commands
such as `commit`, `push`, `pull`, `reset`, `checkout`, `rebase`, `merge`, or
`clean`.

The Phase 11 GitHub connector is read-only and only issues `GET` requests
against the GitHub REST API. It never calls `POST`, `PUT`, `PATCH`, or
`DELETE` endpoints.

## Local Git Connector

The local Git connector reads from a repository on disk. It supports:

- recent commits,
- changed files per commit,
- branch evidence,
- tag evidence,
- simple diff summaries,
- inferred topics from file paths.

Example:

```bash
rivora ingest git --repo . --limit 20
rivora ingest git --repo . --since 7d
```

Evidence is stored in:

```text
.rivora/evidence.json
```

The store deduplicates evidence by id.

## GitHub Connector

The GitHub connector reads pull requests, issues, workflow runs, releases, and
deployments from the GitHub REST API and maps them into the same `EvidenceItem`
model used by the local Git connector. GitHub evidence kinds include:

- `GitHubPullRequest`
- `GitHubPullRequestMerged`
- `GitHubIssue`
- `GitHubWorkflowRun`
- `GitHubWorkflowFailed`
- `GitHubWorkflowSucceeded`
- `GitHubRelease`
- `GitHubDeployment`

Example:

```bash
rivora ingest github --repo owner/name
rivora ingest github --repo owner/name --limit 20
rivora ingest github --repo owner/name --since 7d
rivora ingest github --repo owner/name --pull-requests
rivora ingest github --repo owner/name --issues
rivora ingest github --repo owner/name --workflow-runs
rivora ingest github --repo owner/name --releases
```

If no source flags are provided, the connector ingests recent merged PRs,
issues, workflow runs, and releases by default. Deployments are opt-in because
they are noisier and less universally available.

### Authentication

GitHub access is read-only.

- `GITHUB_TOKEN` is optional for public repositories but recommended for
  private repositories and higher rate limits.
- Tokens are never stored in `.rivora/`, never printed, never written into
  evidence bodies, receipts, or test snapshots.
- The token is passed to `curl` through stdin (`--config -`) so it never
  appears in the process argument list and is not visible via `ps`.
- Error messages from `curl` stderr are redacted before they can appear in a
  `RivoraError`.

### Stable Evidence IDs

GitHub evidence deduplicates by stable ids of the form:

```text
github:pr:<owner/name>:<number>
github:issue:<owner/name>:<number>
github:workflow:<owner/name>:<run-id>
github:release:<owner/name>:<release-id>
github:deployment:<owner/name>:<deployment-id>
```

Repeated ingestion of the same repository does not duplicate the same PR or
workflow run.

### Testing Without Network Access

The connector exposes a `GitHubClient` trait with two implementations:

- `HttpGitHubClient` — the real client backed by `curl`.
- `FixtureGitHubClient` — a test double that returns preloaded fixture JSON.

All connector and CLI tests use the fixture client. No test requires live
GitHub network access.

## Vercel Connector

The Vercel connector reads deployment evidence from the Vercel REST API and
maps it into the same `EvidenceItem` model used by the Git and GitHub
connectors. Vercel evidence kinds include:

- `VercelDeployment`

Example:

```bash
export VERCEL_TOKEN=
rivora ingest vercel --project <project-id-or-name> --limit 20
rivora ingest vercel --project <project-id-or-name> --team <team-id-or-slug>
rivora ingest vercel --project <project-id-or-name> --since 7d
```

### Authentication

Vercel access is read-only.

- `VERCEL_TOKEN` is required.
- `VERCEL_TEAM_ID` is optional and used when the token belongs to a team
  account.
- Tokens are never stored in `.rivora/`, never printed, never written into
  evidence bodies, receipts, or test snapshots.
- The token is passed to `curl` through stdin (`--config -`) so it never
  appears in the process argument list and is not visible via `ps`.
- Error messages from `curl` stderr are redacted before they can appear in a
  `RivoraError`.

### Stable Evidence IDs

Vercel evidence deduplicates by stable ids of the form:

```text
vercel:deployment:<project-slug>:<uid>
```

Repeated ingestion of the same project does not duplicate the same deployment.

### Testing Without Network Access

The connector exposes a `VercelClient` trait with two implementations:

- `HttpVercelClient` — the real client backed by `curl`.
- `FixtureVercelClient` — a test double that returns preloaded fixture JSON.

All connector and CLI tests use the fixture client. No test requires live
Vercel network access.

### Example Flow

```bash
rivora ingest vercel --project my-app --limit 20
rivora evidence list
rivora ask "what deployed recently?"
rivora ask "what failed in vercel?"
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
```

## Cloudflare Connector

The Cloudflare connector reads Pages and Workers deployment evidence from the
Cloudflare REST API and maps it into the same `EvidenceItem` model used by the
Git, GitHub, and Vercel connectors. Cloudflare evidence kinds include:

- `CloudflarePagesDeployment`
- `CloudflareWorkerDeployment`

Example:

```bash
export CLOUDFLARE_API_TOKEN=
rivora ingest cloudflare pages --account <account-id> --project <project-name> --limit 20
rivora ingest cloudflare worker --account <account-id> --script <script-name> --limit 20
rivora ingest cloudflare pages --account <account-id> --project <project-name> --since 7d
rivora ingest cf pages --account <account-id> --project <project-name> --limit 20
rivora ingest cf worker --account <account-id> --script <script-name> --limit 20
```

### Authentication

Cloudflare access is read-only.

- `CLOUDFLARE_API_TOKEN` is required. `CF_API_TOKEN` is accepted as a
  fallback. If both are set, `CLOUDFLARE_API_TOKEN` takes precedence.
- Tokens are never stored in `.rivora/`, never printed, never written into
  evidence bodies, receipts, or test snapshots.
- The token is passed to `curl` through stdin (`--config -`) so it never
  appears in the process argument list and is not visible via `ps`.
- Error messages from `curl` stderr are redacted before they can appear in a
  `RivoraError`.
- Create the narrowest Cloudflare API token possible for read-only deployment
  evidence ingestion.

### Stable Evidence IDs

Cloudflare evidence deduplicates by stable ids of the form:

```text
cloudflare:pages-deployment:<project-name>:<deployment-id>
cloudflare:worker-deployment:<script-name>:<deployment-id>
```

Repeated ingestion of the same project or script does not duplicate the same
deployment.

### Testing Without Network Access

The connector exposes a `CloudflareClient` trait with two implementations:

- `HttpCloudflareClient` — the real client backed by `curl`.
- `FixtureCloudflareClient` — a test double that returns preloaded fixture
  JSON.

All connector and CLI tests use the fixture client. No test requires live
Cloudflare network access.

### Example Flow

```bash
export CLOUDFLARE_API_TOKEN=
rivora init
rivora ingest cloudflare pages --account <account-id> --project my-pages-app --limit 20
rivora ingest cloudflare worker --account <account-id> --script my-worker --limit 20
rivora ask "what changed on cloudflare?"
rivora ask "what failed recently?"
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
```

## Diagnostics

Before ingesting from live providers, verify your environment:

```bash
rivora doctor
```

`rivora doctor` checks that your `.rivora/` store is valid, `.gitignore`
includes `.rivora/`, and provider tokens are set for configured connectors.
If a `--since` value is malformed, the CLI now prints the expected format
(e.g. `7d`, `24h`) instead of failing silently.

No infrastructure actions are taken. No data leaves your machine.

## Evidence to Memory

Evidence remains evidence until a human chooses to remember it:

```bash
rivora evidence list
rivora evidence show <evidence-id>
rivora remember --from-evidence <evidence-id>
```

`remember --from-evidence` creates a `MemoryStatus::Candidate` through the
Adaptive Memory Engine. It does not approve the memory automatically. When the
evidence came from GitHub, the candidate summary makes the source explicit:

```text
Memory candidate created from GitHub evidence.

Source: GitHubPullRequestMerged
Summary: PR #128 merged: "Reduce checkout worker concurrency"
Status: Candidate
```

## Fixture Evidence

Phase 13 adds fixture ingestion for deterministic demos and tests:

```bash
rivora ingest fixture --path examples/demo/evidence.json
```

Fixture evidence is local JSON, deduplicates by evidence id, and uses the
same `.rivora/evidence.json` store as Git and GitHub evidence. It is fake demo
data and does not require network access.

## Ask Flow

`rivora ask` reads from local evidence and routes provider-aware prompts:

- `what changed?` and `what changed in checkout?` show recent matching
  evidence. When evidence comes from multiple providers, the response is
  grouped by source (GitHub, Vercel, Cloudflare Pages, Cloudflare Workers,
  Git).
- `what changed across providers?` shows cross-source grouped evidence.
- `what happened during the release?` shows cross-source grouped evidence.
- `what changed in github?` shows recent GitHub evidence only.
- `what merged recently?` shows GitHub PR-merge evidence.
- `what failed recently?` shows failure evidence from all providers (GitHub
  workflow failures, failed Vercel deployments, failed Cloudflare
  deployments).
- `what deployed recently?` shows deployment evidence from Vercel and
  Cloudflare.
- `what changed in vercel?` and `what failed in vercel?` show Vercel evidence.
- `what changed on cloudflare?` and `what failed in cloudflare?` show
  Cloudflare evidence.
- `have we seen checkout deploy failures before?` routes to recall.

Cross-source summaries are evidence-backed, not root-cause claims. Rivora
says "these events occurred in the same window" and "this may be related."
It never says "X caused Y."

The CLI does not claim root cause. It suggests the explicit next step:

```bash
rivora remember --from-evidence <evidence-id>
```

## Intentionally Not Supported Yet

- AWS, Kubernetes, Datadog, Render, or other cloud connectors.
- Slack API or OAuth.
- Ability Runtime.
- LLM routing.
- Daemon mode, cloud sync, hosted service, or dashboards.
- Autonomous infrastructure actions.

Related: [CLI_MEMORY_INTERFACE.md](CLI_MEMORY_INTERFACE.md) ·
[ADAPTIVE_MEMORY_ENGINE.md](ADAPTIVE_MEMORY_ENGINE.md) ·
[DEMO.md](DEMO.md) ·
[18-Roadmap.md](18-Roadmap.md)
