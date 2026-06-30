# Demo

> Phase 13 makes Rivora easy to understand locally in under five minutes.
> Phase 13.5 adds opt-in, fixture-backed scenario variants for richer demos.
> Phase 13.6 packages those fixtures into the CLI binary.

The demo proves the local memory loop:

```text
Evidence -> Memory Candidate -> Human Approval -> Recall
```

It uses deterministic fixture data embedded at compile time from
`crates/rivora-cli/fixtures/demo/`. It works from an installed binary without
the source checkout. It does not require GitHub, Slack, tokens, network
access, cloud infrastructure, or a running service. No data leaves the
machine.

Human-readable copies remain under `examples/demo/scenarios/`; tests require
those copies to exactly match the packaged fixtures.

## What The Demo Proves

- Rivora can ingest local fixture evidence.
- Evidence remains evidence until a human chooses to remember it.
- `remember --from-evidence` creates a candidate memory.
- `feedback <memory-id> approve` turns that candidate into approved memory.
- `recall`, `ask`, and `slack dev` can use the same local memory/evidence
  store.
- Rivora does not take infrastructure actions.

## What It Does Not Prove

- Live Slack Socket Mode transport.
- The official Slack Marketplace app.
- Hosted OAuth, Rivora Cloud, dashboards, daemon mode, or cloud sync.
- AWS, Kubernetes, Datadog, or other infrastructure connectors.
- LLM routing or Ability Runtime.
- Autonomous remediation, rollback, deployment, scaling, or restart.

## Run The Built-In Demo

```bash
rivora demo
```

The default demo creates a temporary store, runs the memory loop, prints a
summary, and cleans up automatically. It maps to the short `basic` scenario
and remains suitable for a first run in under 60 seconds.

The demo does not resolve fixture paths at runtime. This works from any
working directory after installation:

```bash
cargo install --path crates/rivora-cli
cd /tmp
rivora demo --scenario workflow-failure
```

## Scenario Variants

Use an opt-in scenario for a more realistic design-partner or launch-video
flow:

```bash
rivora demo --scenario basic
rivora demo --scenario checkout-incident
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure
```

- `basic` is the default, compact checkout memory loop.
- `checkout-incident` connects a latency note, release evidence, a failed
  validation workflow, and a merged concurrency change without claiming root
  cause.
- `release-regression` records a release, retry-policy change, and smoke-test
  sequence without suggesting release orchestration.
- `workflow-failure` turns billing migration validation evidence into reusable,
  human-approved memory.

Keep artifacts for inspection:

```bash
rivora demo --keep
```

Use an explicit store root:

```bash
rivora demo --store /tmp/rivora-demo-store
```

Emit a compact machine-readable summary:

```bash
rivora demo --json
rivora demo --scenario checkout-incident --json
```

Scenario JSON includes the scenario name, evidence count, selected evidence
id, memory id and final state, recall match count, Slack dev rendering state,
and safety summaries. Temporary paths are omitted.

## Run The Recording Script

The launch-video flow is scripted:

```bash
scripts/demo-local-memory-loop.sh
scripts/demo-local-memory-loop.sh checkout-incident
```

The script delegates to the same packaged `rivora demo --scenario` path. It
does not read raw fixture paths at runtime and remains suitable for recording
the evidence, approval, recall, ask, and Slack dev flow.

Keep the generated store:

```bash
RIVORA_DEMO_KEEP=1 scripts/demo-local-memory-loop.sh
```

The environment-variable form is also supported:

```bash
RIVORA_DEMO_SCENARIO=release-regression scripts/demo-local-memory-loop.sh
```

Use an already-built binary:

```bash
RIVORA_BIN=target/debug/rivora scripts/demo-local-memory-loop.sh
```

## Manual Flow

```bash
rivora init
rivora ingest fixture --path examples/demo/evidence.json
rivora evidence list
rivora evidence show github:pr:demo/checkout:128
rivora remember --from-evidence github:pr:demo/checkout:128
rivora feedback <memory-id> approve
rivora recall checkout-api --symptom latency --tag inventory
rivora ask "what changed?"
rivora ask "have we seen checkout latency before?"
rivora slack dev --text "what changed?"
```

## Launch Video Demo Flow

Use the checkout incident scenario for the clearest evidence-to-memory story:

```bash
rivora demo --scenario checkout-incident
```

For a recording that shows each public CLI command separately:

```bash
scripts/demo-local-memory-loop.sh checkout-incident
```

The scenario shows that Rivora found evidence, proposed a memory, recorded a
human approval, and recalled that memory later. It does not claim diagnosis or
perform any production action.

## Cleanup

`rivora demo` cleans up automatically unless `--keep` or `--store` is used.
The script cleans up automatically unless `RIVORA_DEMO_KEEP=1` is set.

If you used an explicit store, remove that directory when finished.

## Safety Notes

- No tokens are required.
- No data leaves your machine.
- Fixture evidence is fake and local.
- Fixture evidence is packaged into the installed CLI.
- Slack dev mode does not connect to Slack.
- Evidence is not memory until a human approves it.
- No root cause is claimed without evidence.
- No infrastructure actions are taken.

Related: [CLI_MEMORY_INTERFACE.md](CLI_MEMORY_INTERFACE.md) ·
[SLACK_SELF_HOSTING.md](SLACK_SELF_HOSTING.md) ·
[EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md) ·
[18-Roadmap.md](18-Roadmap.md)
