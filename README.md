# Rivora

> **Engineering understanding, not engineering automation.**

Rivora is an open-source **Engineering Understanding Platform** built around:

1. An exceptional **Runtime**
2. A thoughtful **Workspace**
3. An extensible **ecosystem** of connectors and capabilities

Instead of replacing GitHub, CI/CD, cloud providers, observability platforms, or coding agents, Rivora helps them work together by building durable engineering memory, shared context, and evidence-backed understanding.

---

## Current Development: v0.7 — Engineering Loop Integration

Rivora v0.7 answers: **Can every Capability participate consistently in the Engineering Loop while the Runtime remains the single source of engineering reasoning?**

```text
Connectors → Normalized facts
      ↓
Capabilities → Intent + typed lifecycle contributions
      ↓
Runtime → Memory → Evaluation → Verification → Improvement → Learning
```

Built on v0.1–v0.6 (Memory, Knowledge, Evaluation, Verification, Proposals, Learning, Observation Connectors, bounded Execution Capabilities, Plans, Approvals, Attempts, Receipts):

- **Lifecycle contract** (RFC-028) — every Capability declares `Supported` / `NotApplicable` / `Unsupported` / `Deferred` for each loop stage
- **Typed contributions** — Capabilities never write Memory or create Evaluations/Verifications themselves; Runtime orchestrates existing engines
- **Typed routing** — Observations route to Capabilities by stable input type ids, not vendor names
- **Durable lineage** — `CapabilityLifecycleRun` snapshots with explicit partial/failed/deferred stages and idempotent replay
- **CLI / Workspace** — `rivora capability …` and Workspace Engineering Loop surface

**Connectors provide normalized engineering data. Capabilities express engineering intent. The Runtime produces engineering knowledge through the Engineering Loop.**

Execution safety from v0.6 is preserved: only explicitly approved, bounded capabilities mutate external systems. Proposal acceptance never starts execution. External API success is not verified success or Outcome success.

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
      ├── Implementation Records / Measured Outcomes / Patterns
      └── Execution Plans / Approvals / Attempts / Receipts / Verification
      │
      ▼
Local Store   (.rivora/data)

Observation connectors ──► Observations only ──► Runtime
Execution adapters ──► bounded mutations (Runtime-invoked only)
```

### Architectural invariants

- The Runtime is the single source of engineering reasoning.
- Memory is append-only; history is never rewritten.
- Knowledge is derived from Memory.
- Evaluations are explainable and evidence-backed.
- Verification produces durable receipts (pass / fail / inconclusive).
- Recommendations are directional assistance, never auto-applied.
- Improvement Proposals are durable suggestions, never implementations.
- Proposal acceptance is explicit and never implies implementation, execution, or verification.
- Implementation Records record external work only; they never apply changes alone.
- Measured Learning Outcomes require explicit evaluation and verification authority.
- Execution requires an explicit plan, exact-revision approval, and centralized policy.
- Observation connectors remain read-only; mutation uses separate ExecutionCapability adapters.
- Learning records outcomes without rewriting history.
- Investigations remain independent historical records; relationships do not merge them.
- Recalled historical context is labeled and distinct from current evidence.
- Connectors only observe and normalize.
- Workspace and CLI share the same Capabilities and Runtime.

See `docs/ARCHITECTURAL_INVARIANTS.md` and `docs/rfc/`.

---

## Install / Build

Requirements: Rust 1.75+ (edition 2021).

```sh
git clone https://github.com/rivora-dev/rivora.git
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

# Author a bounded multi-action execution plan from an accepted Proposal.
# Repeat --action-input and --precondition as needed.
./target/release/rivora execute plan \
  --investigation <ID> \
  --proposal <ACCEPTED_PROPOSAL_ID> \
  --capability mock.record \
  --target-system mock \
  --environment sandbox \
  --action record_mutation \
  --action-input '{"resource_key":"demo/1","field":"label","value":"ready"}' \
  --precondition '{"id":"scope-ok","description":"Target is in approved scope","satisfied":true,"detail":null}'

# Validate, preview, and explicitly approve the exact immutable revision.
./target/release/rivora execute validate --investigation <ID> --plan <PLAN_ID> --reason "inputs and target reviewed"
./target/release/rivora execute preview --investigation <ID> --plan <READY_PLAN_ID>
./target/release/rivora execute approve --investigation <ID> --plan <READY_PLAN_ID> --reason "bounded sandbox mutation approved"

# Live execution requires both approval and --confirm. API success is still
# followed by independent verification.
./target/release/rivora execute run \
  --investigation <ID> \
  --plan <APPROVED_PLAN_ID> \
  --approval <APPROVAL_ID> \
  --idempotency-key example-live-1 \
  --confirm
./target/release/rivora execute verify --investigation <ID> --attempt <ATTEMPT_ID>

# Engineering Loop (v0.7)
./target/release/rivora capability list
./target/release/rivora capability show mock.record
./target/release/rivora capability lifecycle --investigation <ID> --attempt <ATTEMPT_ID>
./target/release/rivora capability trace --investigation <ID> <ATTEMPT_ID>
./target/release/rivora capability lifecycle-list --investigation <ID>
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
| `execute plan` / `revise` / `revisions` | Author ordered actions and preconditions; preserve and inspect immutable plan revisions |
| `execute validate` / `preview` / `policy` | Validate before approval and inspect dry-run and centralized policy decisions |
| `execute approve` / `reject` / `cancel` | Record explicit authority and lifecycle decisions |
| `execute run` / `attempts` / `verify` | Execute only an approved target, inspect durable attempts, and independently verify effects |
| `execute receipts` / `export-receipt` / `trace` | Inspect and export sanitized evidence and trace the complete execution lineage |
| `execute rollback-plan` | Generate a separate draft rollback plan from explicit inverse metadata; never auto-roll back |
| `capability list` / `show` | List registered Capabilities and Engineering Loop participation |
| `capability route` | Deterministic Observation → Capability routing |
| `capability lifecycle` / `lifecycle-list` / `lifecycle-show` | Run and inspect Engineering Loop stage status |
| `capability trace` | Trace Observation/execution → Memory → … → Learning lineage |

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
- author, validate, preview, approve, cancel, and inspect immutable Execution Plan revisions
- review the exact plan revision, target, capability, risk, policy, and approval before confirming a live mutation
- inspect Attempts and Receipts, independently verify effects, and export sanitized receipt JSON
- create a separate rollback Plan from explicit inverse metadata for later validation and approval

The Workspace labels every Proposal as **not applied, not implemented, and not verified**. It has no Apply action and does not invoke coding agents. It never performs automatic rollback.

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

### Bounded GitHub execution adapters (v0.6)

GitHub mutation is provided by separate `ExecutionCapability` adapters, never
by observation connectors. Configure the bounded repository and credentials
before starting the CLI or Workspace:

```sh
export RIVORA_GITHUB_REPO=owner/repository
export GITHUB_TOKEN=...
```

The adapter's normalized owner/repository and applicable branch/ref are bound
into the Plan and Approval target snapshots. Changing this runtime target does
not redirect existing authority; execution is rejected until a new Plan
revision is validated and approved. Tokens are not persisted or exported.

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
  implementations/
  learning_outcomes/
  execution_plans/
  execution_approvals/
  execution_attempts/
  execution_receipts/
  execution_verifications/
.rivora/data/learning/
  patterns/
```

Memory is append-only. Corrections create new records. Proposal, learning, and execution storage is additive and lazy; existing v0.1-v0.5 stores require no destructive migration. Execution Plan revisions and Attempts/Receipts/Verifications remain durable; list operations isolate corrupt records and report diagnostics.

Proposal, Execution Plan, and Receipt export is explicit and stdout-only in the CLI. It never writes into a source tree or silently overwrites a file. Persisted Plans, Approvals, and Receipts contain sanitized metadata, never credentials.

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
| `docs/ARCHITECTURAL_INVARIANTS.md` | Non-negotiable architectural invariants (tracked source of truth) |
| `docs/internal/` | Local working notes only (gitignored; not required for contributors) |
| `ROADMAP.md` | Release progression and future boundary |
| `docs/rfc/RFC-000` … `RFC-028` | Architecture and feature RFCs, including v0.6 execution and v0.7 Engineering Loop (RFC-028) |

---

## v0.6–v0.7 execution and loop boundary

Rivora can invoke only registered, typed, bounded capabilities after exact-revision approval and policy evaluation. It does not run unrestricted shell commands, merge or force-push, delete branches/repositories/infrastructure, edit workflow definitions, auto-execute accepted Proposals, retry hiddenly, or perform automatic rollback/remediation. A successful mutation response is a Receipt, not proof of the expected effect or a successful Measured Outcome.

v0.7 adds formal Engineering Loop participation and inspection; it does not add marketplaces, SDKs, or autonomous remediation. See `ROADMAP.md`, RFC-025 through RFC-027, and RFC-028.

---

## License

Apache-2.0 — see `LICENSE`.
