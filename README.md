# Rivora

> **Engineering understanding, not engineering automation.**

Rivora is an open-source **Engineering Understanding Platform** built around:

1. An exceptional **Runtime**
2. A thoughtful **Workspace**
3. An extensible **ecosystem** of connectors and capabilities

Instead of replacing GitHub, CI/CD, cloud providers, observability platforms, or coding agents, Rivora helps them work together by building durable engineering memory, shared context, and evidence-backed understanding.

---

## Current Release: v0.1 — Runtime Foundation

Rivora v0.1 proves one coherent Runtime can execute the complete engineering understanding lifecycle:

```text
Observation
→ Memory
→ Knowledge
→ Evaluation
→ Verification
→ Recommendation
→ Learning
```

Interfaces (Workspace and CLI) invoke the same Capability layer over the same Runtime.

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
      └── Learning Engine
      │
      ▼
Local Store   (.rivora/data)

Connectors ──► Observations only ──► Runtime
  (local, GitHub)
```

### Architectural invariants

- The Runtime is the single source of engineering reasoning.
- Memory is append-only; history is never rewritten.
- Knowledge is derived from Memory.
- Evaluations are explainable and evidence-backed.
- Verification produces durable receipts (pass / fail / inconclusive).
- Recommendations are proposals, never auto-applied.
- Learning records outcomes without rewriting history.
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
| `observe` | Ingest Observations (manual / local / GitHub) |
| `recall` | Recall Investigation Memory |
| `timeline` | Chronological Memory timeline |
| `knowledge` | Derive Knowledge from Memory |
| `evaluate` | Produce explainable Evaluations |
| `verify` | Produce Verification Receipts |
| `recommend` | Generate evidence-backed Recommendations |
| `learn` | Record Learning outcomes |
| `pipeline` | Run knowledge → evaluate → verify → recommend |

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
```

Memory is append-only. Corrections create new records.

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
| `docs/internal/IMPLEMENTATION_PLAN.md` | v0.1 implementation plan |
| `docs/rfc/RFC-000` … `RFC-014` | Foundational RFCs |

---

## Roadmap (post v0.1)

Later versions may add Investigation Graphs, cross-investigation knowledge, collaboration, automation, connector SDKs, and enterprise features. These are **out of scope** for v0.1.

---

## License

Apache-2.0 — see `LICENSE`.
