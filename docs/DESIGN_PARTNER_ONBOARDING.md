# Design Partner Onboarding

> How an early team can try Rivora safely.

---

## Prerequisites

* macOS, Linux, or Windows (WSL2)
* Rust toolchain (for building from source)
* Git
* (Optional) A GitHub personal access token for public repo evidence
* (Optional) A Vercel token for deployment evidence
* (Optional) A Cloudflare API token for Pages/Workers deployment evidence
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
rivora demo --scenario multi-source-release
```

The `multi-source-release` scenario demonstrates cross-source evidence from
GitHub, Vercel, Cloudflare Pages, Cloudflare Workers, and Sentry in a single release
window.

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

## Ingest Vercel evidence

```bash
export VERCEL_TOKEN=
rivora ingest vercel --project <project-id-or-name> --limit 20
```

Reads deployment evidence from Vercel. `VERCEL_TOKEN` is required and never
stored. The connector is read-only; no deployment, rollback, or promotion
actions are taken.

---

## Ingest Cloudflare evidence

```bash
export CLOUDFLARE_API_TOKEN=
rivora ingest cloudflare pages --account <account-id> --project <project-name> --limit 20
rivora ingest cloudflare worker --account <account-id> --script <script-name> --limit 20
```

Reads deployment evidence from Cloudflare Pages and Workers.
`CLOUDFLARE_API_TOKEN` is required and never stored. The connector is
read-only; no deployment, rollback, promotion, DNS, route, Worker, Pages, KV,
R2, D1, or Queues actions are taken.

---

## Optional: ingest Sentry issue evidence

Use a narrow Sentry token with `event:read` to test metadata-first issue
evidence:

```bash
export SENTRY_AUTH_TOKEN=...
rivora ingest sentry --org my-org --project checkout-api --limit 20
rivora ask "what errors happened recently?"
rivora ask "what failed recently?"
rivora ask "what happened during the release?"
```

Rivora only reads normalized issue metadata. It does not ingest raw stack
traces, request data, user emails, IPs, replay data, or breadcrumbs, and it
does not resolve, assign, or otherwise mutate Sentry issues. Evidence stays
local and is not memory until approved. No infrastructure actions are taken.
`SENTRY_AUTH_TOKEN` takes precedence over the optional `SENTRY_TOKEN`
fallback. The default query is `is:unresolved`, and results are capped at 100.

## Create and approve memory

Continue through the normal review flow:

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
rivora ask "what deployed recently?"
rivora ask "what failed recently?"
rivora ask "what happened during the release?"
rivora ask "have we seen checkout latency before?"
rivora ask "have we seen checkout deploy failures before?"
rivora ask "what merged recently?"
```

When evidence comes from multiple providers, responses are grouped by source.

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

## Recommended evaluation path

A short path that exercises the full core loop plus Slack:

```bash
rivora doctor
rivora demo --scenario multi-source-release
rivora init
rivora ingest git --repo . --limit 20
rivora ask "what changed?"
rivora ask "what deployed recently?"
rivora ask "what failed recently?"
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
rivora ask "have we seen this before?"
rivora slack doctor
rivora slack dev --text "what changed?"
```

`rivora doctor` runs local diagnostics before you begin. It checks your store,
`.gitignore`, and provider tokens. No infrastructure actions are taken.

Every command supports `--help` for detailed usage:

```bash
rivora doctor --help
rivora ingest --help
rivora ask --help
```

This stays local and deterministic. No tokens are required for the demo, Git
ingest, or Slack dev mode.

---

## Feedback loop

### What to try first

1. Run `rivora demo --scenario checkout-incident` to see the core loop.
2. Run `rivora init` and ingest your own Git history.
3. Approve one memory candidate from real evidence.
4. Ask `rivora ask "have we seen this before?"` and see what recall surfaces.
5. Try `rivora slack doctor` and `rivora slack dev` to feel the Slack
   interface without connecting to a workspace.

### What feedback to file

We'd love to hear:

* Did the memory loop make sense?
* Was "evidence vs memory" clear?
* Did recall feel useful?
* Did Slack feel calm and trustworthy?
* What evidence source would be most valuable next?
* What made you hesitate to trust Rivora?
* What would make you use this daily?

### Which issue template to use

| Feedback type | Template |
|---|---|
| Something broken | [Bug report](https://github.com/rivora-dev/rivora/issues/new?template=bug_report.yml) |
| General impressions | [Feedback](https://github.com/rivora-dev/rivora/issues/new?template=feedback.yml) |
| Request a connector | [Evidence connector request](https://github.com/rivora-dev/rivora/issues/new?template=evidence_connector_request.yml) |
| Slack trouble | [Slack setup help](https://github.com/rivora-dev/rivora/issues/new?template=slack_setup_help.yml) |
| Structured design partner report | [Design partner report](https://github.com/rivora-dev/rivora/issues/new?template=design_partner_report.yml) |

See [COMMUNITY_FEEDBACK.md](COMMUNITY_FEEDBACK.md) for discussion categories and
[FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md) for how feedback is evaluated.

### How to sanitize logs

Rivora redacts Slack and GitHub tokens in diagnostic output, but review any
text before pasting it into an issue:

1. Remove any `xoxb-`, `xapp-`, `ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_`
   prefixed values.
2. Remove signing secrets and private keys.
3. Remove internal hostnames, customer identifiers, and production incident
   timelines that include sensitive data.
4. Replace real private repository URLs with `owner/name` placeholders.

### How to report security issues privately

Do not open a public issue for security vulnerabilities. Use the repository's
[private vulnerability reporting form](https://github.com/rivora-dev/rivora/security/advisories/new).
See [SECURITY.md](../SECURITY.md) for the full policy.

### What Rivora is trying to learn from design partners

* Where the demo and CLI flow confuse new users.
* Whether "evidence vs memory" feels clear in practice.
* Whether recall surfaces useful past situations.
* Where Slack setup is friction-heavy.
* Which evidence connector would be most valuable next.
* What would make a team use Rivora weekly.

---

## Known limitations

* Crates are not published; install the local preview from source
* No AWS, GCP, Azure, Render, or Kubernetes connectors
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
