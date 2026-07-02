# Rivora

> **Adaptive reliability memory for engineering teams.**

Rivora turns engineering evidence into human-approved operational memory,
so teams can understand what changed and recall what helped before without
giving up control.

---

## What is Rivora?

Rivora is adaptive reliability memory.

It ingests engineering evidence, helps teams decide what should be remembered,
recalls approved operational knowledge when similar situations happen again, and
works through CLI plus a self-hosted Slack interface.

**Core loop:**

```
Ask → Explain → Remember → Recall
```

Rivora is:

* Local-first — your data stays on your machine
* Open source — inspect, modify, self-host
* Human-in-the-loop — evidence is not memory until a human approves it
* Safety-first — no infrastructure actions are taken

Rivora is **not**:

* An autonomous SRE
* A replacement for engineering teams
* A black-box AI platform
* Another observability dashboard

### Why memory before automation?

Automation acts on a system. Memory helps people make better decisions about
that system while preserving context, evidence, and accountability. Rivora
therefore stops at explanation, memory proposals, human feedback, and recall;
it never executes a production action.

---

## Try it in 60 seconds

From a source checkout:

```bash
cargo install --path crates/rivora-cli
rivora demo
rivora demo --scenario checkout-incident
rivora demo --scenario multi-source-release
```

`rivora demo` uses deterministic fixture data embedded in the CLI binary. It
works after installation without a source checkout and does not need tokens,
network access, Slack, GitHub, or infrastructure credentials. No data leaves
the machine.

You will see:

```text
Evidence → Memory Candidate → Human Approval → Recall
```

No infrastructure actions are taken.

Other local and deterministic scenarios:

```bash
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure
rivora demo --scenario multi-source-release
```

---

## Demo scenarios

| Scenario | What it shows |
|---|---|
| `basic` | Short memory loop (default) |
| `checkout-incident` | Checkout latency, PR merge, human approval |
| `release-regression` | Release rollback, incident response |
| `workflow-failure` | CI/CD workflow failure and learning |
| `multi-source-release` | Cross-source evidence from GitHub, Vercel, Cloudflare, Sentry |

All scenarios use synthetic fixture data. No tokens, no network, no data
leaves your machine.

---

## Real local workflow

```bash
rivora init
rivora ingest git --repo . --limit 20
rivora ask "what changed?"
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
rivora ask "have we seen checkout latency before?"
```

Every command supports `--help` for detailed usage:

```bash
rivora doctor --help
rivora ingest --help
rivora ask --help
```

`rivora doctor` runs local diagnostics to verify your store, `.gitignore`, and
provider tokens are configured correctly. No infrastructure actions are taken.

Evidence is not memory until a human chooses to remember and approve it.

---

## Git and GitHub evidence

```bash
# Local Git history
rivora ingest git --repo . --limit 20

# GitHub pull requests, issues, workflow runs (read-only)
rivora ingest github --repo owner/name --limit 20
```

GitHub evidence ingestion uses only `GET` requests. `GITHUB_TOKEN` is
optional for public repos and never stored.

---

## Vercel evidence

```bash
# Vercel deployment evidence (read-only)
export VERCEL_TOKEN=
rivora ingest vercel --project <project-id-or-name> --limit 20
rivora ingest vercel --project <project-id-or-name> --team <team-id-or-slug>
rivora ingest vercel --project <project-id-or-name> --since 7d
```

Vercel evidence ingestion uses only `GET` requests. `VERCEL_TOKEN` is required
and never stored. No deployment, rollback, or promotion actions are taken.

```bash
rivora ask "what deployed recently?"
rivora ask "what failed in vercel?"
rivora ask "what changed in vercel?"
```

---

## Cloudflare evidence

```bash
# Cloudflare Pages deployment evidence (read-only)
export CLOUDFLARE_API_TOKEN=
rivora ingest cloudflare pages --account <account-id> --project <project-name> --limit 20
rivora ingest cloudflare worker --account <account-id> --script <script-name> --limit 20
rivora ingest cloudflare pages --account <account-id> --project <project-name> --since 7d
```

Cloudflare evidence ingestion uses only `GET` requests. `CLOUDFLARE_API_TOKEN`
is required and never stored. No deployment, rollback, promotion, DNS, route,
Worker, Pages, KV, R2, D1, or Queues actions are taken.

```bash
rivora ask "what changed on cloudflare?"
rivora ask "what failed in cloudflare?"
```

---

## Sentry observability evidence

```bash
export SENTRY_AUTH_TOKEN=...
rivora init
rivora ingest sentry --org my-org --project checkout-api --limit 20
rivora ingest sentry --org my-org --project checkout-api --environment production --since 24h
rivora ask "what errors happened recently?"
rivora ask "what failed recently?"
rivora ask "what happened during the release?"
```

Sentry ingestion is read-only and metadata-first. Rivora uses GET requests
with a narrow `event:read` token, stores normalized issue metadata locally,
and never resolves or assigns issues or mutates alerts, projects, releases,
or deploys. Raw stack traces, event payloads, request bodies, headers,
cookies, user emails, IP addresses, replay data, and breadcrumbs are not
ingested. Evidence is not memory until approved. No infrastructure actions
are taken.

When `--query` is omitted, Sentry's default unresolved-issue query applies.
`--limit` defaults to 20 and is capped at 100. `SENTRY_AUTH_TOKEN` takes
precedence over the optional `SENTRY_TOKEN` fallback; neither value is
printed, persisted, or exposed through debug output.

```bash
rivora evidence list
rivora remember --from-evidence <evidence-id>
rivora feedback <memory-id> approve
```

---

## Self-hosted Slack

```bash
rivora slack doctor
rivora slack dev --text "what changed?"
rivora slack socket
```

The Slack interface is self-hosted. It is not the official Slack Marketplace
app. Tokens are read from the environment and never stored in `.rivora/`.

```text
@rivora what changed?
@rivora have we seen checkout latency before?
@rivora what merged recently?
```

See [docs/SLACK_SELF_HOSTING.md](docs/SLACK_SELF_HOSTING.md).

---

## Cross-source evidence

When evidence comes from multiple providers, Rivora groups responses by source:

```bash
rivora ask "what changed?"
rivora ask "what deployed recently?"
rivora ask "what failed recently?"
rivora ask "what happened during the release?"
```

Cross-source summaries are evidence-backed, not root-cause claims. Rivora
never says "X caused Y." It says "these events occurred in the same window"
and "this may be related."

See [docs/PRODUCT_VALIDATION.md](docs/PRODUCT_VALIDATION.md) for the full
validation flow.

---

## Safety model

Rivora may:

* Ingest evidence
* Propose memory candidates
* Recall similar situations
* Apply human feedback

Rivora must not:

* Execute remediation
* Trigger rollbacks
* Deploy code
* Mutate infrastructure
* Run autonomous agent loops

Every memory operation produces a receipt. Human approval changes memory
status only; Rivora has no infrastructure-action path.

---

## Help shape Rivora

Rivora is in a public v0.1 preview. We are listening before building more.

* [Share feedback](https://github.com/rivora-dev/rivora/issues/new?template=feedback.yml)
* [Request an evidence connector](https://github.com/rivora-dev/rivora/issues/new?template=evidence_connector_request.yml)
* [Design partner onboarding](docs/DESIGN_PARTNER_ONBOARDING.md)
* [Security reporting](SECURITY.md)

See [docs/COMMUNITY_FEEDBACK.md](docs/COMMUNITY_FEEDBACK.md) for discussion
categories and [docs/FEEDBACK_ANALYSIS.md](docs/FEEDBACK_ANALYSIS.md) for how
feedback is evaluated.

---

## Contributing

Rivora is built collaboratively by engineers and AI.

Whether you're contributing code, documentation, design, or ideas, we'd love
your help.

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

Rivora is licensed under
[Apache License 2.0](LICENSE)

---

**Adaptive Reliability Memory. Human First. Open Forever.**
