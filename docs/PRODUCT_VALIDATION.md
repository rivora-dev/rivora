# Product Validation

> How contributors and design partners can validate Rivora end-to-end.

---

## What this validation covers

Phase 18.5 validates the complete Rivora product loop across all evidence
sources implemented so far:

* Git (local history)
* GitHub (pull requests, issues, workflow runs, releases, deployments)
* Vercel (deployment evidence)
* Cloudflare Pages (deployment evidence)
* Cloudflare Workers (deployment evidence)
* Sentry (metadata-first issue/error evidence)
* PlanetScale (metadata-first database branch and deploy-request evidence)

This phase proves that Rivora works as an end-to-end adaptive reliability
memory product before adding more provider connectors.

The validation covers:

* Multi-source evidence ingestion
* Cross-source evidence grouping and timeline display
* Memory candidate quality across provider types
* Human feedback and approval lifecycle
* Deterministic recall of approved memory
* Slack dev mode with mixed providers
* Safety: no root-cause claims, no infrastructure actions, no token leakage

---

## Quick start: multi-source demo

The fastest way to see the full product loop:

```bash
cargo install --path crates/rivora-cli
rivora demo --scenario multi-source-release
```

This uses deterministic fixture data embedded in the CLI binary. No tokens,
no network, no data leaves your machine.

The demo simulates a checkout release window across multiple evidence sources:

* GitHub PR merged for checkout retry policy
* GitHub workflow failed after merge
* Vercel production deployment completed for checkout-web
* Cloudflare Pages preview deployment failed for checkout-web
* Cloudflare Worker deployment completed for checkout-worker
* Sentry TypeError issue observed for checkout-api
* PlanetScale deploy request observed for checkout-db

You will see:

```text
Evidence → Memory Candidate → Human Approval → Recall
```

Evidence is not memory until a human approves it. No infrastructure actions
are taken.

---

## Step-by-step validation

### 1. Initialize a local store

```bash
rivora init
```

Creates `.rivora/` with `memories.json`, `feedback.json`, `receipts.json`,
and `evidence.json`.

### 2. Ingest multi-source fixture evidence

```bash
rivora ingest fixture --path examples/demo/scenarios/multi-source-release/evidence.json
```

This loads seven synthetic records across GitHub, Vercel, Cloudflare, Sentry,
and PlanetScale.

### 3. Ask questions

```bash
rivora ask "what changed?"
rivora ask "what deployed recently?"
rivora ask "what failed recently?"
rivora ask "what happened during the release?"
rivora ask "what changed across providers?"
```

When evidence comes from multiple providers, Rivora groups the response by
source:

```text
Recent evidence

GitHub
- PR #142 merged: checkout retry policy update — ...

Vercel
- Vercel production deployment for checkout-web completed — ...

Cloudflare Pages
- Cloudflare Pages preview deployment for checkout-web failed — ...

Cloudflare Workers
- Cloudflare Worker deployment for checkout-worker completed — ...

Sentry
- Sentry issue CHECKOUT-9001 (error) — ...

PlanetScale
- PlanetScale deploy request #42 — ...

Recent evidence was found across providers.
Nearby evidence may be related.

This may be worth remembering.
Evidence is not memory until approved.
No infrastructure actions were taken.
```

PlanetScale sections also state that no database actions were taken.

### 4. Review evidence

```bash
rivora evidence list
rivora evidence show <evidence-id>
```

Evidence list shows the source provider and status for each item. Evidence
show displays the source, kind, status, topic, and timestamp.

### 5. Create a memory candidate

```bash
rivora remember --from-evidence <evidence-id>
```

The memory candidate summary includes the provider source:

```text
Memory candidate created from GitHub evidence.

Source: GitHub PR merged
Summary: GitHub evidence: PR #142 merged to update the checkout retry policy...
Status: Candidate
```

### 6. Approve the memory

```bash
rivora feedback <memory-id> approve
```

### 7. Recall approved memory

```bash
rivora ask "have we seen this before?"
rivora ask "have we seen checkout deploy failures before?"
```

### 8. Test Slack dev mode

```bash
rivora slack dev --text "what changed?"
rivora slack dev --text "what deployed recently?"
rivora slack dev --text "what failed recently?"
rivora slack dev --text "have we seen checkout deploy failures before?"
```

Slack dev mode does not connect to Slack. It simulates the Slack interface
locally. Output is grouped by provider when multiple sources are present.

---

## Other demo scenarios

```bash
rivora demo --scenario basic
rivora demo --scenario checkout-incident
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure
rivora demo --scenario multi-source-release
```

All scenarios use synthetic fixture data. No tokens, no network.

---

## Real provider ingestion

With real tokens, you can ingest from live providers:

```bash
# Git
rivora ingest git --repo . --limit 20

# GitHub (GITHUB_TOKEN optional for public repos)
export GITHUB_TOKEN=
rivora ingest github --repo owner/name --limit 20

# Vercel
export VERCEL_TOKEN=
rivora ingest vercel --project <project> --limit 20

# Cloudflare Pages
export CLOUDFLARE_API_TOKEN=
rivora ingest cloudflare pages --account <account-id> --project <project-name> --limit 20

# Cloudflare Workers
rivora ingest cloudflare worker --account <account-id> --script <script-name> --limit 20

# Sentry issues (event:read token)
export SENTRY_AUTH_TOKEN=...
rivora ingest sentry --org <org-slug> --project <project-slug> --limit 20
rivora ask "what errors happened recently?"

# PlanetScale branches and deploy requests (read_branch + read_deploy_request)
export PLANETSCALE_SERVICE_TOKEN_ID=...
export PLANETSCALE_SERVICE_TOKEN=...
rivora ingest planetscale --org <org-slug> --database <database-name> --limit 20
rivora ask "what database changes happened recently?"
rivora ask "what schema changes happened recently?"
```

Sentry defaults to unresolved issues when `--query` is omitted and caps one
issue-list page at 100 records. Evidence show identifies Sentry as
metadata-first and confirms that sensitive event data is not stored.

PlanetScale is metadata-first and never connects to the customer database,
runs SQL, reads customer rows or branch passwords, or ingests connection
strings, raw query results, full schema dumps, schema diffs, or raw DDL. It
never creates, approves, or deploys deploy requests or mutates branches.

All provider integrations are read-only. Tokens are never stored in
`.rivora/`.

---

## How to sanitize logs before sharing feedback

Rivora redacts tokens in diagnostic output, but review any text before
pasting it into an issue:

1. Remove any `xoxb-`, `xapp-`, `ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_`
   prefixed values.
2. Remove `VERCEL_TOKEN`, `CLOUDFLARE_API_TOKEN`, `SENTRY_AUTH_TOKEN`,
   `SENTRY_TOKEN`, `PLANETSCALE_SERVICE_TOKEN_ID`, `PLANETSCALE_SERVICE_TOKEN`, and
   `PLANETSCALE_AUTH_TOKEN` values.
3. Remove signing secrets and private keys.
4. Remove internal hostnames, customer identifiers, and production incident
   timelines that include sensitive data.
5. Replace real private repository URLs with `owner/name` placeholders.

---

## Local diagnostics

Before ingesting from live providers, verify your environment:

```bash
rivora doctor
```

`rivora doctor` checks:

* `.rivora/` store exists and is valid
* `.gitignore` includes `.rivora/`
* Provider tokens are set for configured connectors (GitHub, Vercel,
  Cloudflare, Sentry, PlanetScale)

PlanetScale service-token diagnostics report both
`PLANETSCALE_SERVICE_TOKEN_ID` and `PLANETSCALE_SERVICE_TOKEN` as `set` or
`not set`; OAuth fallback diagnostics report `PLANETSCALE_AUTH_TOKEN`. Values
are never printed.

No infrastructure actions are taken. No data leaves your machine.

Every command supports `--help` for detailed usage:

```bash
rivora doctor --help
rivora ingest --help
rivora ask --help
```

After common commands (`init`, `ingest`, `remember`, `feedback`), Rivora now
prints guided next steps so you always know what to do next. Empty states
follow a "What happened? Why? What next?" structure so you are never stuck
without guidance.

---

## Known limitations

* Crates are not published; install from source
* No AWS, GCP, Azure, Render, or Kubernetes connectors yet
* No official Slack Marketplace app
* No hosted OAuth flow
* No Rivora Cloud
* No Ability Runtime
* No Slack correction modals
* No autonomous remediation or infrastructure mutation
* Cross-source grouping is deterministic and timestamp-based; it does not use
  embeddings, LLMs, or probabilistic ranking
* Live Vercel and Cloudflare connectors were not tested against production
  APIs during this phase (tokens were unavailable)
* Live Sentry was not tested during Phase 20A or its safety audit because no
  token was available
* Live PlanetScale was not tested during Phase 20B because no token was
  available

---

## Related

- [DESIGN_PARTNER_ONBOARDING.md](DESIGN_PARTNER_ONBOARDING.md)
- [EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md)
- [DEMO.md](DEMO.md)
- [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md)
- [18-Roadmap.md](18-Roadmap.md)
