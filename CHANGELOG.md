# Changelog

  All notable changes to Rivora will be documented in this file.

  The format is based on [Keep a Changelog](https://keepachangelog.com/)
  and this project follows [Semantic Versioning](https://semver.org/).

  ## [Unreleased]

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
  - No AWS, GCP, Azure, Vercel, Cloudflare, Render, or Kubernetes connectors
  - No Ability Runtime
  - No Slack correction modals
  - No autonomous remediation or infrastructure mutation
  - No long-running agent loops

  [Unreleased]: https://github.com/rivora-dev/rivora/compare/v0.1.0...HEAD
  [0.1.0]: https://github.com/rivora-dev/rivora/releases/tag/v0.1.0
