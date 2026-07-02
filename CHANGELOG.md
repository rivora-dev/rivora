# Changelog

  All notable changes to Rivora will be documented in this file.

  The format is based on [Keep a Changelog](https://keepachangelog.com/)
  and this project follows [Semantic Versioning](https://semver.org/).

  ## [Unreleased]

  ### Added

  - Phase 20B.1 PlanetScale connector audit covering the live authentication
    contract, curl transport hardening, poisoned allowlisted fields,
    deterministic deduplication, recall approval gating, CLI/Slack/doctor
    parity, and public documentation
  - Phase 20B: read-only, metadata-first PlanetScale branch and deploy-request
    evidence connector (`rivora ingest planetscale`, with `pscale` alias)
  - PlanetScale branch, time-window, and safely capped 1-100 limit filters; stable
    branch/deploy-request evidence IDs; offline fixture client
  - PlanetScale-aware ask, memory candidate, recall, doctor, Slack, and
    cross-source release behavior
  - Exact PlanetScale metadata allowlists plus malicious fixture coverage for
    credentials, rows, connection strings, schema data, query results, and DDL
  - Synthetic PlanetScale deploy request in the multi-source release scenario
  - Phase 20A.1 Sentry connector safety audit covering GET-only enforcement,
    malicious nested payload exclusion, token/debug redaction, CLI and Slack
    parity, evidence rendering, memory recall, and public documentation
  - Phase 20A: read-only, metadata-first Sentry issue evidence connector
    (`rivora ingest sentry`)
  - Sentry environment, query, limit, and `--since` filters with stable
    evidence IDs and offline fixture clients
  - Sentry-aware ask, recall, memory candidate, and cross-source release
    summaries
  - Explicit Sentry metadata/tag allowlists and token/PII redaction
  - Synthetic Sentry issue in the multi-source release scenario
  - Phase 19: `rivora doctor` command for local diagnostics (store,
    `.gitignore`, provider tokens)
  - Subcommand help (`rivora <command> --help`) for all commands
  - Guided next steps after `init`, `ingest`, `remember`, and `feedback`
  - Improved empty-state messages with "What happened? Why? What next?"
    structure
  - Malformed `--since` guidance for provider ingests
  - Output formatting improvements (timestamps in evidence list)
  - Phase 18.5: evidence-to-memory product validation across Git, GitHub,
    Vercel, Cloudflare Pages, and Cloudflare Workers
  - `multi-source-release` demo scenario with cross-source fixture evidence;
    Phase 20A expands it to six records including Sentry
  - `CrossSourceEvidenceSummary` helper for deterministic cross-source evidence
    grouping by provider, timestamp, and status
  - Cross-source ask behavior: `what changed?`, `what deployed recently?`,
    and `what failed recently?` now group by provider when evidence comes
    from multiple sources
  - New ask prompts: `what changed across providers?`,
    `what happened during the release?`,
    `have we seen checkout deploy failures before?`
  - Improved evidence list/show with source provider and status labels
  - Improved memory candidate summaries that identify the provider source
  - `docs/PRODUCT_VALIDATION.md` for end-to-end validation guidance
  - 20+ new end-to-end validation tests covering multi-source ingest,
    cross-source ask, recall, Slack dev, and safety
  - Cloudflare evidence connector: read-only ingestion of Cloudflare Pages and
    Workers deployment evidence via the Cloudflare REST API (`rivora ingest
    cloudflare pages`, `rivora ingest cloudflare worker`)
  - `CLOUDFLARE_API_TOKEN` environment variable support for Cloudflare
    connector authentication (with `CF_API_TOKEN` fallback)
  - Cloudflare-aware ask routing (`what changed on cloudflare?`,
    `what failed in cloudflare?`)
  - `FixtureCloudflareClient` for deterministic testing without network access
  - Vercel evidence connector: read-only ingestion of Vercel deployment
    evidence via the Vercel REST API (`rivora ingest vercel`)
  - `VERCEL_TOKEN` environment variable support for Vercel connector
    authentication
  - Vercel-aware ask routing (`what deployed recently?`,
    `what failed in vercel?`, `what changed in vercel?`)
  - `FixtureVercelClient` for deterministic testing without network access
  - GitHub issue templates: bug report, feedback, evidence connector request,
    Slack setup help, and design partner report
  - Pull request template with a safety boundary checklist
  - Community feedback and feedback analysis documentation
  - GitHub labels guidance
  - ADR 0017: design partner feedback before provider connectors
  - Docs index (`docs/README.md`) and roadmap (`docs/18-Roadmap.md`)

  ### Changed

  - PlanetScale service-token authentication now requires both
    `PLANETSCALE_SERVICE_TOKEN_ID` and `PLANETSCALE_SERVICE_TOKEN` using the
    documented `ID:TOKEN` header; `PLANETSCALE_AUTH_TOKEN` remains an OAuth
    Bearer-token fallback
  - PlanetScale curl requests disable user curl configuration, restrict HTTPS,
    reject unsafe credential characters, and redact broader credential shapes
  - PlanetScale allowlisted fields now use field-specific validation, safe
    permalinks are restricted to `app.planetscale.com`, malformed responses
    fail closed, and duplicate IDs resolve deterministically
  - Natural-language recall no longer includes unapproved candidate memories;
    candidates remain available only through explicit recall flags
  - Cross-source summaries no longer claim a shared time window without
    calculating one
  - Multi-source summaries now group PlanetScale data-layer evidence alongside
    GitHub, Vercel, Cloudflare, and Sentry using root-cause-neutral language
    and an explicit no-database-actions safety statement
  - `rivora doctor` now reports PlanetScale token presence without printing
    values
  - Sentry issue parsing now honors the current `issueType` and numeric
    `userCount` response fields, caps all clients at 100 records, rejects
    malformed JSON, and drops undated evidence when `--since` is used
  - Sentry auth debug output, `Bearer` values, `sntrys_` values, unsafe
    permalinks, invalid counts/timestamps, and private paths are redacted or
    omitted before local persistence
  - Sentry empty-state, evidence show, top-level help, memory candidate, and
    recall output now identify the source and metadata-first safety boundary
  - Multi-source summaries now group Sentry issue evidence alongside GitHub,
    Vercel, and Cloudflare evidence using root-cause-neutral language
  - Phase 18.5: `what deployed recently?` now includes Vercel and Cloudflare
    deployments; `what failed recently?` now includes failed workflow and
    deployment evidence from all providers
  - Evidence list shows source provider and status for each item
  - Evidence show displays source, kind label, status, topic, and timestamp
  - Memory candidate summaries from evidence now include the provider label
  - Design partner onboarding updated with a recommended evaluation path and a
    feedback loop section
  - README links to feedback and connector request templates
  - Internal launch docs moved to a gitignored `docs/internal/` so they are not
    published with the public repo
  - `.gitignore` now includes `.rivora/` as required by the safety model

  ## [0.1.0] - 06-29-2026

  ### Added

  - Adaptive reliability memory with human-reviewed candidates
  - Deterministic, evidence-backed memory recall
  - Local CLI with a gitignored `.rivora/` JSON store
  - Reliability receipts with provenance, reasoning, and confidence
  - Read-only local Git and GitHub evidence connectors
  - Four packaged deterministic demo scenarios
  - Self-hosted Slack integration using Socket Mode
  - `rivora slack doctor` setup validation
  - Contributor, architecture, onboarding, demo, and security documentation

  ### Security

  - Local-first storage with no telemetry
  - Read-only evidence connectors
  - Environment-only GitHub and Slack credentials
  - Token redaction in diagnostic output
  - No autonomous remediation, deployment, rollback, or infrastructure mutation
  - Human approval required before evidence becomes organizational memory

  ### Known limitations

  - Crates are not yet published; install Rivora from source
  - No official Slack Marketplace application or hosted OAuth flow
  - No hosted Rivora Cloud service
  - No AWS, GCP, Azure, Render, or Kubernetes connectors
  - No Ability Runtime
  - No Slack correction modals
  - No autonomous remediation or infrastructure mutation
  - No long-running agent loops

  [Unreleased]: https://github.com/rivora-dev/rivora/compare/v0.1.0...HEAD
  [0.1.0]: https://github.com/rivora-dev/rivora/releases/tag/v0.1.0
