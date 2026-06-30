# Changelog

  All notable changes to Rivora will be documented in this file.

  The format is based on [Keep a Changelog](https://keepachangelog.com/)
  and this project follows [Semantic Versioning](https://semver.org/).

  ## [Unreleased]

  ### Added

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
