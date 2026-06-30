# Design Partner Onboarding

> How an early team can try Rivora safely.

---

## Prerequisites

* macOS, Linux, or Windows (WSL2)
* Rust toolchain (for building from source)
* Git
* (Optional) A GitHub personal access token for public repo evidence
* (Optional) A self-hosted Slack app for team use

---

## Install

```bash
git clone https://github.com/Rivora-AI/Open-Rivora.git
cd Open-Rivora
cargo install --path crates/rivora-cli
```

---

## Try the demo

```bash
rivora demo
```

This runs a deterministic, local-only demo. No tokens, no network, no data
leaves your machine.

```bash
rivora demo --scenario checkout-incident
```

---

## Initialize local store

```bash
rivora init
```

Creates `.rivora/` in the current directory with `memories.json`,
`feedback.json`, `receipts.json`, and `evidence.json`.

---

## Ingest Git evidence

```bash
rivora ingest git --repo . --limit 20
```

Reads your local Git history and creates evidence items.

---

## Ingest GitHub evidence

```bash
export GITHUB_TOKEN=
rivora ingest github --repo owner/name --limit 20
```

Reads pull requests, issues, workflow runs, releases, and deployments from
GitHub. `GITHUB_TOKEN` is optional for public repos and never stored. For
authenticated access, set the empty placeholder to a real token only in your
local shell; do not put it in a file or command history.

---

## Create and approve memory

```bash
# List evidence
rivora evidence list

# Create a memory candidate from evidence
rivora remember --from-evidence <evidence-id>

# Approve it
rivora feedback <memory-id> approve
```

---

## Ask questions

```bash
rivora ask "what changed?"
rivora ask "have we seen checkout latency before?"
rivora ask "what merged recently?"
```

---

## Run Slack dev

```bash
rivora slack dev --text "what changed?"
rivora slack dev --text "have we seen checkout latency before?"
```

Slack dev mode does not connect to Slack. It simulates the Slack interface
locally.

---

## Optional: Self-hosted Slack Socket Mode

```bash
# Validate setup
rivora slack doctor

# Set tokens
export SLACK_BOT_TOKEN=
export SLACK_APP_TOKEN=
export SLACK_SIGNING_SECRET=

# Start live Socket Mode
rivora slack socket
```

Then mention Rivora in Slack:

```text
@rivora what changed?
@rivora have we seen checkout latency before?
```

Replace the empty environment values only in your local secret-management
workflow. Rivora reads them at runtime and does not persist them.

---

## What to share as feedback

We'd love to hear:

* Did the memory loop make sense?
* Was "evidence vs memory" clear?
* Did recall feel useful?
* Did Slack feel calm and trustworthy?
* What evidence source would be most valuable next?
* What made you hesitate to trust Rivora?
* What would make you use this daily?

---

## Known limitations

* Crates are not published; install the local preview from source
* No AWS, GCP, Azure, Vercel, Cloudflare, Render, or Kubernetes connectors
* No official Slack Marketplace app
* No hosted OAuth flow
* No Rivora Cloud
* No Ability Runtime
* No Slack correction modals
* No autonomous remediation
* No infrastructure mutation

---

## How to uninstall / clean up

```bash
# Remove the binary
cargo uninstall rivora-cli

# Remove local store
rm -rf .rivora/

# Remove the repo, if this is a disposable clone
cd ..
rm -rf Open-Rivora
```

No system-wide changes are made. No services are installed. No cron jobs are
created. Review the path before removing `.rivora/`; it contains the local
memory and evidence you created during onboarding.
