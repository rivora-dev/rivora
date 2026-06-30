# CLI Memory Interface

> The CLI is the local engineer interface for Rivora's adaptive reliability
> memory loop. Slack remains the team memory interface.

Phase 8 implements a minimal `rivora` binary in `crates/rivora-cli`. It makes
the core loop runnable from a local terminal:

```text
Ask -> Explain -> Remember -> Recall
```

The CLI is local-first, deterministic, and boring by design. It stores Rivora
memory state in JSON files under `.rivora/` in the current working directory.

## What It Does

- Initializes local memory storage with `rivora init`.
- Runs a deterministic local demo with `rivora demo`.
- Creates candidate memories with `rivora remember`.
- Optionally applies approval feedback with `rivora remember --approve`.
- Recalls similar memories with deterministic `AdaptiveMemoryEngine` scoring.
- Applies human feedback with `rivora feedback`.
- Ingests local Git evidence with `rivora ingest git`.
- Ingests read-only GitHub evidence with `rivora ingest github`.
- Ingests deterministic local fixture evidence with `rivora ingest fixture`.
- Lists and shows evidence with `rivora evidence`.
- Routes simple prompts through `rivora ask` without an LLM dependency.
- Routes self-hosted Slack dev messages with `rivora slack dev`.
- Runs the live self-hosted Slack Socket Mode app-mention transport with
  `rivora slack socket`.
- Validates self-hosted Slack setup with `rivora slack doctor`.
- Shows local counts with `rivora status`.
- Stores generated memories, feedback, and receipts locally.

## Local Storage

Phase 8 uses simple JSON files:

```text
.rivora/
memories.json
feedback.json
receipts.json
evidence.json
```

Missing files are initialized as empty arrays. Existing files are not
destroyed by `rivora init`. Empty files and empty stores are handled without
panics.

## Commands

### `rivora init`

Creates the local store if it does not exist and prints memory, feedback, and
receipt counts.

### `rivora demo`

Runs a safe, deterministic local demo:

```bash
rivora demo
rivora demo --scenario checkout-incident
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure
rivora demo --keep
rivora demo --json
rivora demo --store /tmp/rivora-demo-store
```

The default demo creates a temporary store, loads fixture evidence embedded in
the installed CLI, creates a candidate memory, records human approval feedback,
recalls the approved memory, runs an `ask` example, renders a Slack dev
response, and cleans up.
The default maps to the short `basic` scenario. Opt-in scenarios use the same
runner with different deterministic evidence, selected memory candidate, and
ask/recall prompts.

No tokens, network access, GitHub, Slack, or infrastructure credentials are
required. The demo works without a source checkout and does not write
`.rivora/` into the repo root by default. See [DEMO.md](DEMO.md).

### `rivora remember`

Creates a `MemoryStatus::Candidate` memory through `AdaptiveMemoryEngine`.

Common flags:

- `--service <service>`
- `--summary <summary>`
- `--symptom <symptom>`
- `--tag <tag>`
- `--evidence <evidence>`
- `--source <source>`
- `--confidence <low|medium|high|number>`
- `--approve`
- `--from-evidence <evidence-id>`

Candidates are not approved by default. `--approve` records explicit CLI human
feedback and applies the memory lifecycle transition through the engine.

`--from-evidence` creates a candidate from a stored evidence item. Evidence is
not memory until this explicit step happens.

### `rivora ingest git`

Reads local Git history and stores evidence in `.rivora/evidence.json`:

```bash
rivora ingest git --repo . --limit 20
rivora ingest git --repo . --since 7d
```

The Git connector is read-only. It does not run mutating commands such as
`commit`, `push`, `pull`, `reset`, `checkout`, `rebase`, `merge`, or `clean`.

### `rivora ingest github`

Reads pull requests, issues, workflow runs, releases, and deployments from the
GitHub REST API and stores evidence in `.rivora/evidence.json`:

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
issues, workflow runs, and releases by default.

The GitHub connector is read-only. It only issues `GET` requests and never
calls `POST`, `PUT`, `PATCH`, or `DELETE` endpoints.

`GITHUB_TOKEN` is optional for public repositories but recommended for private
repositories and higher rate limits. Tokens are never stored in `.rivora/`,
never printed, and never written into evidence bodies, receipts, or test
snapshots. See [EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md) for details.

### `rivora ingest fixture`

Reads deterministic fixture evidence from a JSON file and stores it in
`.rivora/evidence.json`:

```bash
rivora ingest fixture --path examples/demo/evidence.json
```

Fixture ingestion is local-only, deduplicates by evidence id, and is intended
for demos, tests, and launch recordings. It does not require network access.

### `rivora evidence`

Reviews stored local evidence:

```bash
rivora evidence list
rivora evidence show <evidence-id>
```

Evidence output includes kind, summary, inferred topic, changed files, and the
command for turning the evidence into a candidate memory.

### `rivora recall`

Runs deterministic recall over local memories:

```bash
rivora recall checkout-api --symptom latency --tag inventory
rivora recall checkout-api --include-candidates
```

Output includes score, confidence, status, match reasons, and evidence
references. If no memory matches, the CLI prints a safe next step and takes no
action.

### `rivora feedback`

Applies typed feedback to an existing memory:

```bash
rivora feedback <memory-id> approve
rivora feedback <memory-id> reject
rivora feedback <memory-id> correct --note "Root cause was connection pool exhaustion"
rivora feedback <memory-id> useful
rivora feedback <memory-id> not-useful
rivora feedback <memory-id> needs-more-evidence
```

Feedback is appended to `.rivora/feedback.json`, receipts are appended to
`.rivora/receipts.json`, and the memory record is updated in
`.rivora/memories.json`.

### `rivora ask`

Routes simple natural-language-ish prompts without an LLM:

- `have we seen checkout latency before?` routes to recall.
- `recall checkout` routes to recall.
- `what should we remember about checkout?` explains the fields required to
  create a candidate.
- `what changed?` and `what changed in checkout?` read from local evidence
  (Git or GitHub), avoid root-cause claims, and suggest
  `rivora remember --from-evidence`.
- `what changed in github?` shows recent GitHub evidence only.
- `what merged recently?` shows GitHub PR-merge evidence.
- `what failed recently?` shows GitHub workflow-failure evidence.

Unknown prompts return examples instead of pretending to understand.

### `rivora status`

Prints local counts for total, candidate, active, rejected, and corrected
memories, plus feedback and receipt entries.

### `rivora slack doctor`

Validates the self-hosted Slack setup before running the live adapter:

```bash
rivora slack doctor
rivora slack doctor --live
```

`doctor` checks environment variables (`SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`,
`SLACK_SIGNING_SECRET`), the local `.rivora/` store, and token validity. It
reports setup issues and suggests next steps for missing states. The `--live`
flag performs a live Socket Mode handshake check against the Slack API.

### `rivora slack`

Runs the self-hosted Slack adapter boundary with setup validation, socket
startup diagnostics, reconnect resilience, and envelope hardening:

```bash
rivora slack dev --text "what changed?"
rivora slack dev --text "have we seen checkout latency before?"
rivora slack socket
```

`slack dev` uses local `.rivora/` memory and evidence without connecting to
Slack. `slack socket` validates `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and
`SLACK_SIGNING_SECRET`, then runs the live foreground Socket Mode listener.
It handles app mentions and posts plain-text thread replies; interactive
buttons remain deferred.

Socket startup diagnostics surface connection failures immediately. The
adapter recovers from transient disconnects with exponential backoff and
reconnect resilience. Envelope parsing hardens against malformed payloads
by validating structure before dispatch. If any required state is missing,
setup guidance is printed directing the user to `rivora slack doctor`.

Use `RIVORA_STORE_DIR` to point the adapter at a local store directory. Tokens
are never written into `.rivora/`.

## Safety Boundary

The CLI may propose memory, recall similar situations, and apply human
feedback. It does not execute remediation, rollback, deployment,
infrastructure mutation, long-running agent loops, or autonomous production
actions. The Slack adapter runs within a hardened runtime boundary that
validates envelopes, enforces token isolation, and rejects malformed input.

Phase 8 intentionally did not include connectors. Phase 10 added local Git
evidence ingestion. Phase 11 added read-only GitHub evidence ingestion. Phase
12 adds the self-hosted Slack adapter boundary and dev mode. Phase 13 adds
deterministic demo and fixture ingestion paths. Phase 13.5 adds local demo
scenario variants. Phase 13.6 packages those fixture sets into the CLI binary.
Phase 14 adds `rivora slack doctor` for setup validation, socket startup
diagnostics, reconnect resilience, envelope hardening, and a hardened runtime
boundary. The CLI still does not include
the official Slack Marketplace app, hosted OAuth, live Socket Mode transport,
cloud connectors such as AWS/Kubernetes/Datadog, Ability Runtime, LLM routing,
daemon mode, cloud sync, hosted service behavior, dashboards, or autonomous
infrastructure actions.

## Relationship to Slack

Slack is the primary team memory interface. The CLI is the local engineer
interface. Both surfaces use the same adaptive memory engine and receipt-backed
memory model, but neither surface takes control away from engineers.

Related: [13-CLI-UX.md](13-CLI-UX.md) ·
[ADAPTIVE_MEMORY_ENGINE.md](ADAPTIVE_MEMORY_ENGINE.md) ·
[DEMO.md](DEMO.md) ·
[SLACK_APP.md](SLACK_APP.md) ·
[SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md) ·
[18-Roadmap.md](18-Roadmap.md)
