# Rivora

> **Engineering understanding, not engineering automation.**

Rivora is an open-source **Engineering Understanding Platform** built around:

1. An exceptional **Runtime**
2. A thoughtful **Workspace**
3. An extensible **ecosystem** of connectors and capabilities

Instead of replacing GitHub, CI/CD, cloud providers, observability platforms, or coding agents, Rivora helps them work together by building durable engineering memory, shared context, and evidence-backed understanding.

---

## Current Development: v0.5 — Measure and Learn

Rivora v0.5 answers: **Which improvements worked, which failed, and why—using durable evidence without applying changes?**

It closes the feedback loop after a Proposal is implemented *outside* Rivora by recording external work, measuring outcomes, verifying conclusions, and deriving historical patterns.

```text
Observe → Remember → Understand → Assist → Propose → Measure → Learn
```

Implemented for v0.5:

- **Implementation Records** (RFC-022) — durable, revisioned records of external work linked to exact Proposal snapshots
- **Measured Learning Outcomes** (RFC-022/023) — expected results, typed evidence, deterministic evaluation, explicit verification with actor + reason
- **Learning Patterns and influence** (RFC-024) — patterns from verified Outcomes; bounded advisory influence on Proposal ranking
- **CLI and Workspace learning experience** — `implementation` / `learn` commands, Learning Outcomes surface, exports, and shared CapabilityService flows

v0.1–v0.4 foundations remain operational. Accepted Proposal ≠ Implementation Record ≠ Verified Measured Outcome ≠ Learning Pattern. Connectors remain read-only and no external system is mutated. Interfaces (Workspace and CLI) invoke the same Capability layer over the same Runtime.

---

## Architecture

```text
Workspace / CLI
      │
      ▼
Capabilities  (engineering intent)
      │
      ▼
Runtime       (single source of reasoning)
      │
      ├── Investigation Manager
      ├── Memory Engine
      ├── Knowledge Engine
      ├── Evaluation Engine
      ├── Verification Engine
      ├── Recommendation Engine
      ├── Learning Engine
      ├── Investigation Graph
      ├── Search and Recall
      ├── Recalled Context / Patterns / Trends
      ├── Assisted Workflows
      ├── Engineering Assistance
      ├── Improvement Proposals
      └── Implementation Records / Measured Outcomes / Patterns
      │
      ▼
Local Store   (.rivora/data)

Connectors ──► Observations only ──► Runtime
  (local, GitHub, GitHub Actions, Kubernetes, Sentry)
```

### Architectural invariants

- The Runtime is the single source of engineering reasoning.
- Memory is append-only; history is never rewritten.
- Knowledge is derived from Memory.
- Evaluations are explainable and evidence-backed.
- Verification produces durable receipts (pass / fail / inconclusive).
- Recommendations are directional assistance, never auto-applied.
- Improvement Proposals are durable suggestions, never implementations.
- Proposal acceptance is explicit and never implies implementation or verification.
- Implementation Records record external work only; they never apply changes.
- Measured Learning Outcomes require explicit evaluation and verification authority.
- Learning records outcomes without rewriting history.
- Investigations remain independent historical records; relationships do not merge them.
- Recalled historical context is labeled and distinct from current evidence.
- Connectors only observe and normalize.
- Workspace and CLI share the same Capabilities and Runtime.

See `docs/internal/ARCHITECTURAL_INVARIANTS.md` and `docs/rfc/`.

---

## Install / Build

Requirements: Rust 1.75+ (edition 2021).

```sh
git clone <repo>
cd rivora
cargo build --workspace --release
```

Binaries:

- `target/release/rivora` — CLI
- `target/release/rivora-workspace` — interactive Workspace

---

## Quick start (CLI)

```sh
# Create an Investigation
./target/release/rivora investigation create "CI failure on main" \
  --description "Investigate recent pipeline failures"

# Copy the printed investigation id, then observe
./target/release/rivora observe \
  --investigation <ID> \
  --summary "CI check failed" \
  --kind check_result \
  --payload '{"status":"failure","error":"assertion failed"}' \
  --idempotency-key ci-1

# Or observe a local project
./target/release/rivora observe --investigation <ID> --local .

# Or observe GitHub (read-only; set GITHUB_TOKEN for private repos)
./target/release/rivora observe --investigation <ID> --github owner/repo --pr 42

# Full reasoning pipeline
./target/release/rivora knowledge --investigation <ID>
./target/release/rivora evaluate --investigation <ID>
./target/release/rivora verify --investigation <ID>
./target/release/rivora recommend --investigation <ID>

# Record outcome and complete
./target/release/rivora learn --investigation <ID> \
  --disposition accepted --notes "Engineer accepted remediation plan"
./target/release/rivora investigation complete <ID>

# Reopen when new observations arrive
./target/release/rivora investigation reopen <ID>

# Cross-Investigation intelligence (v0.2)
./target/release/rivora investigation refresh-relationships <ID>
./target/release/rivora investigation related <ID>
./target/release/rivora investigation similar <ID>
./target/release/rivora search "build failed" --repository acme/app
./target/release/rivora investigation context-suggest <ID>
./target/release/rivora investigation context-attach <ID> --source <PRIOR_ID>
./target/release/rivora patterns
./target/release/rivora trends --repository acme/app

# Propose improvements (v0.4)
./target/release/rivora proposal generate --investigation <ID>
./target/release/rivora proposal list --investigation <ID>
./target/release/rivora proposal compare --investigation <ID> <PROPOSAL_A> <PROPOSAL_B>
./target/release/rivora proposal verification-plan --investigation <ID> <PROPOSAL_ID>
./target/release/rivora proposal implementation-plan --investigation <ID> <PROPOSAL_ID>
./target/release/rivora proposal export --investigation <ID> <PROPOSAL_ID> --format markdown
./target/release/rivora proposal handoff --investigation <ID> <PROPOSAL_ID>
```

Global flags:

- `--data-dir PATH` — local Runtime store (default `.rivora/data`)
- `--json` — structured JSON output

### CLI commands

| Command | Purpose |
| --- | --- |
| `investigation create` | Create Investigation |
| `investigation show` | Show status and object counts |
| `investigation list` | List Investigations |
| `investigation complete` | Complete (must be in Learning) |
| `investigation reopen` | Reopen Completed → Collecting |
| `investigation related` | List related Investigations |
| `investigation link` / `unlink` | Explicit relationship management |
| `investigation relationship` | Explain a relationship |
| `investigation refresh-relationships` | Re-derive graph edges |
| `investigation similar` | Find similar Investigations |
| `investigation context*` | List / suggest / attach / dismiss Recalled Context |
| `observe` | Ingest Observations (manual / local / GitHub) |
| `search` | Search Investigations (text + structured filters) |
| `recall` | Recall Memory, related evidence, or prior outcomes |
| `timeline` | Chronological Memory timeline |
| `knowledge` | Derive Knowledge from Memory |
| `evaluate` | Produce explainable Evaluations |
| `verify` | Produce Verification Receipts |
| `recommend` | Generate evidence-backed Recommendations |
| `learn` | Record Learning outcomes |
| `pipeline` | Run knowledge → evaluate → verify → recommend |
| `patterns` | Detect evidence-backed patterns |
| `trends` | Summarize historical trends |
| `proposal generate` / `alternatives` | Generate deterministic bounded Proposal alternatives |
| `proposal compare` / `prioritize` | Compare Proposals using inspectable factors |
| `proposal show` / `explain` / `provenance` | Inspect Proposal content, evidence, and provenance |
| `proposal feedback` / `refine` / `revisions` | Preserve feedback and immutable revisions |
| `proposal accept` / `reject` / `defer` / `withdraw` | Record explicit human-controlled lifecycle decisions |
| `proposal verification-plan` / `implementation-plan` | Inspect proposed, unexecuted plans |
| `proposal export` / `handoff` | Emit Markdown/JSON artifacts or bounded implementation handoff text |
| `proposal portfolio` / `trace` | Filter an Investigation portfolio and trace evidence to a Proposal |

---

## Workspace

Primary interactive experience:

```sh
./target/release/rivora-workspace
# or with custom store
./target/release/rivora-workspace --data-dir .rivora/data
```

The Workspace lets you:

- create or open an Investigation
- review status, Observations, Memory, Knowledge
- evaluate, verify, recommend
- record outcomes
- complete or reopen
- browse related and similar Investigations
- search prior work and inspect match explanations
- attach or dismiss Recalled Context
- view patterns and minimal historical trends
- generate and compare Improvement Proposal alternatives
- inspect supporting and contradicting evidence, risks, assumptions, implementation outlines, and Verification Plans
- attach feedback, refine while preserving revisions, and explicitly accept, reject, defer, supersede, or withdraw
- export Proposal artifacts or bounded coding-agent handoff text

The Workspace labels every Proposal as **not applied, not implemented, and not verified**. It has no Apply action and does not invoke coding agents.

Non-interactive smoke mode (CI):

```sh
./target/release/rivora-workspace --smoke
```

---

## Connectors

### Local (production-ready for MVP)

Read-only observation of a project directory:

- repository metadata
- git branch / status
- recent commits
- changed files
- optional `test-output.txt` / `.rivora/test-output.txt`
- structured event files under `.rivora/events/*.json`

```sh
rivora observe --investigation <ID> --local /path/to/project
```

### GitHub (narrow, read-only)

- repository metadata
- pull request metadata (`--pr`)
- commits
- check runs
- linked issue references from PR body

```sh
export GITHUB_TOKEN=...   # optional for public data; recommended for rate limits
rivora observe --investigation <ID> --github owner/repo --pr 12
```

Offline fixture mode for tests/demos:

```sh
rivora observe --investigation <ID> --github-fixture path/to/fixture.json
```

Connectors **only** observe → normalize → produce Observations. They never evaluate, verify, recommend, or learn.

---

## Storage

Local filesystem store under `--data-dir` (default `.rivora/data`):

```text
.rivora/data/investigations/{id}/
  investigation.json
  observations/
  memory/
  knowledge/
  evaluations/
  verifications/
  recommendations/
  learning/
  proposals/
  proposal_artifacts/
```

Memory is append-only. Corrections create new records. Proposal storage is additive and lazy; existing v0.1-v0.3 stores require no migration.

Proposal export is explicit and stdout-only in the CLI. It never writes into a source tree or silently overwrites a file.

---

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo build --workspace --release
```

Follow Red → Green → Refactor. See `.agents/skills/build-rivora/SKILL.md`.

### Crate layout

| Crate | Role |
| --- | --- |
| `rivora` | Domain, Runtime, Capabilities, local store |
| `rivora-connectors` | Local + GitHub observation connectors |
| `rivora-cli` | Thin CLI over Capabilities |
| `rivora-workspace` | Interactive Workspace over Capabilities |

---

## Documentation

| Document | Purpose |
| --- | --- |
| `docs/internal/VISION.md` | Product vision |
| `docs/internal/PRINCIPLES.md` | Engineering principles |
| `docs/internal/ARCHITECTURAL_INVARIANTS.md` | Non-negotiable invariants |
| `docs/internal/IMPLEMENTATION_PLAN.md` | Current release implementation plan |
| `ROADMAP.md` | Release progression and future boundary |
| `docs/rfc/RFC-000` … `RFC-021` | Architecture and feature RFCs |

---

## What v0.4 never does

Rivora v0.4 does not edit repositories, write patches to source, create branches or commits, open pull requests, deploy, mutate infrastructure/configuration/tickets, invoke coding agents, execute remediation, or infer Learning Outcomes from accepted Proposals. See `ROADMAP.md` for the release boundary.

---

## License

Apache-2.0 — see `LICENSE`.
