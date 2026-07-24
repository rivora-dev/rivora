# Changelog

## 0.9.0 — Production Hardening

### Phase 0–1 — Audit and operating envelope

- Production readiness audit and hardening matrix (`docs/guides/HARDENING_MATRIX.md`)
- Measurable operating envelope profiles: small / medium / large_supported
- Performance budgets and replay contract tables exposed via Runtime domain types and CLI

### Phase 2–6 — Persistence, integrity, replay

- `store.json` schema manifest (schema v1) with future-version rejection
- Exclusive cross-process store lock with same-process refcount and stale-lock recovery
- Durable writes: unique temps + `fsync` before rename; exclusive append creates
- Observation idempotency key indexes (claim-first, rebuildable)
- Corruption isolation for core history listings (observations/memory)
- Store health report, diagnostic export, backup/restore, index rebuild
- Payload size limits (1 MiB) on Observation ingestion

### Phase 7–9 — Concurrency, Connectors, Loop resilience

- Explicit concurrency contract (reject multi-process writers; no silent corruption)
- Connector resilience helpers: timeouts, response bounds, rate-limit/auth mapping, redaction
- GitHub / Actions / Sentry / Kubernetes / Local observation paths hardened
- Engineering Loop and execution authority boundaries preserved (no silent skips)

### Phase 10–13 — CLI, Workspace, security, diagnostics

- Stable CLI exit codes (0–14) and structured JSON errors
- `rivora doctor` surface: health, export, backup, rebuild-indexes, recover-lock, envelope, budgets, replay-contracts, exit-codes
- Default/max search and list bounds for CLI and Workspace
- Secret redaction and sanitized diagnostics (local only; no telemetry)

### Phase 14–19 — Scenarios, scorecard, freeze, docs

- Automated production scenarios in `v0_9_production_hardening` tests
- Architecture gates for budgets, replay contracts, doctor/resilience surfaces
- Production readiness scorecard, architecture debt register, v1.0 freeze assessment
- Operations, troubleshooting, recovery, backup, concurrency, security, and envelope guides

## 0.8.0 — Capability Coverage

### Phase 0–1 — Inventory and standard lifecycle for every first-party Capability

- Complete first-party inventory: `mock.record` plus five GitHub execution Capabilities
- Extended `ExecutionCapabilityDescriptor` with `name`, `provider`, `operation`, `mutating`, `permissions`, `output_types`, and `limitations` (additive serde defaults)
- Every first-party Capability declares explicit Engineering Loop participation and emits typed contributions through shared Runtime orchestration
- Capability-aware Memory / Evaluation / Verification contribution semantics (not generic API echoes)
- Improvement and Learning remain explicitly **Deferred** until measured evidence exists

### Phase 2–3 — Connector canonical inputs and honest boundaries

- All five observation connectors inventoryed with emitted kinds, fixture support, and limitations
- Kubernetes and Local connectors no longer invent health/failure conclusions; they emit observed facts only
- Local connector gains fixture-parity helper; payloads include `canonical_type` where useful
- Connectors remain read-only and never write lifecycle artifacts

### Phase 4–7 — Multi-Capability coverage surface

- CLI and Workspace always register all first-party execution Capabilities (default sandbox target when `RIVORA_GITHUB_REPO` unset; live still needs token + approval)
- Deterministic zero / one / many Observation → Capability routing validated across the full registry
- `rivora capability coverage` and Workspace **Capability coverage / health** surface
- Shared `CapabilityCoverageReport` with completeness gaps, connector inventory, and first-party registration checks

### Phase 8–10 — Gates, tests, documentation

- Architecture gates enforce complete first-party descriptors, connector non-reasoning, and coverage inventory
- v0.8 unit/integration/CLI coverage tests; v0.7 Engineering Loop regressions retained
- Capability guide, Connector guide, first-party Capability catalog, and lifecycle coverage matrix
- RFC-011 / RFC-012 / RFC-028 status updated for platform-wide validation; version `0.8.0`

## 0.7.0 — Engineering Loop Integration

### Phase 1–2 — Capability lifecycle contract and typed contributions (RFC-028)

- Every registered Capability declares explicit Engineering Loop participation (`Supported` / `NotApplicable` / `Unsupported` / `Deferred`) for Memory, Evaluation, Verification, Improvement, and Learning
- Typed `CapabilityLifecycleContributions` carry provenance, correlation, evidence refs, and stage payloads without Capabilities writing Memory or creating Evaluations directly
- Extended `ExecutionCapabilityDescriptor` with `engineering_loop`, `accepted_input_types`, and `provider_independent` (serde-default safe for older records)

### Phase 3–4 — Routing and Runtime orchestration

- Deterministic Observation → Capability routing on stable input type identifiers (not human names alone)
- Runtime-owned `run_capability_lifecycle_for_attempt` validates contributions against declared participation, applies existing Memory/Evaluation/Verification engines, and records durable `CapabilityLifecycleRun` snapshots
- Explicit stage statuses (Completed / Failed / Deferred / Unsupported / NotApplicable / Blocked); partial progress never misrepresented as full success
- Idempotent replay on lifecycle idempotency keys; append-only history preserved

### Phase 5–7 — Vertical slice, CLI, Workspace

- Vertical slice: `mock.record` (and GitHub workflow dispatch descriptors) through Plan → Approval → Attempt → Receipt → independent verification → Engineering Loop
- CLI: `rivora capability list|show|route|lifecycle|lifecycle-list|lifecycle-show|trace` with `--json`
- Workspace: Capability Engineering Loop surface plus status counts; smoke exercises loop replay

### Phase 8–10 — Persistence, tests, documentation

- Lazy `lifecycle_runs/` storage with corruption isolation; v0.1–v0.6 stores open unchanged
- Unit, integration, architecture, CLI, and Workspace tests for contracts, routing, replay, and boundaries
- RFC-011 / RFC-012 / RFC-028, README, ROADMAP, and architectural invariants updated for v0.7

## 0.6.0 — Execution Through External Systems

### Phase 1 — Execution Plans and Authority (RFC-025)

- Durable Execution Plans convert accepted Proposals into ordered external actions
- Immutable plan revisions; exact-revision Execution Approvals with actor, reason, scope, environment
- Immutable target snapshots bind provider, owner, repository, environment, capability, Plan revision, and branch/ref; runtime target drift invalidates approval
- Invalid capabilities, targets, actions, inputs, preconditions, and risk combinations are rejected before approval
- Centralized Execution Policy (`Allowed` / `AllowedWithApproval` / `AllowedDryRunOnly` / `Denied`)
- Lifecycle: Draft → ReadyForReview → Approved → Executing → Executed → Verified → Closed (plus exceptional states)
- Proposal Accepted ≠ Execution Approved ≠ Execution Started ≠ Verified ≠ Outcome Successful

### Phase 2 — Bounded External Capabilities (RFC-026)

- Distinct `ExecutionCapability` contract separate from read-only observation connectors
- Risk levels; v0.6 supports ReadOnly / LowRiskWrite / BoundedWrite only (HighRiskWrite and Prohibited denied)
- Dry-run / plan validation, idempotency keys, retry safety classification, preconditions
- Initial adapters: `mock.record`, GitHub issue comment/label/create, draft PR, workflow dispatch
- Duplicate capability registration is rejected; timeouts and ambiguous transport outcomes remain explicitly uncertain
- No arbitrary shell, force-push, merge, or autonomous remediation

### Phase 3 — Receipts, Verification, CLI/Workspace (RFC-027)

- `Started` Attempts and idempotency reservations are durable before mutation; recovery and duplicate suppression preserve audit history
- Execution Attempts, Receipts, and capability-specific independent Verification records with partial-failure and uncertainty modeling
- Explicit inverse rollback metadata creates a separate draft Plan for normal validation and approval; no automatic rollback
- Idempotent linkage populates Implementation Record and Measured Outcome trace identifiers
- CLI supports ordered multi-action/precondition authoring, cancellation, revision listing, rollback-plan creation, and Receipt export
- Workspace shows the exact revision, target, capability, risk, policy, and approval before live confirmation
- Architecture and regression tests enforce authority, connector boundaries, policy denial, durability, verification, rollback, and traceability

## 0.5.0 — Learning Outcomes

### Phase 1–2 — Implementation Records, Measured Outcomes, Patterns (RFC-022/023/024)

- Durable Implementation Records for external work linked to Improvement Proposals
- Measured Learning Outcomes with expected results, typed evidence, deterministic evaluation, and explicit verification
- Learning Patterns derived from verified Outcomes with historical influence explanations
- Acceptance never implies implementation; evaluation never implies verified; patterns never auto-apply changes

### Phase 3 — CLI, Workspace, and learning experience

- Thin CLI `implementation` and `learn` command trees over CapabilityService
- Preserved v0.1 disposition recording as `record-outcome`
- Workspace Learning Outcomes surface for record → measure → verify → pattern flows
- Markdown/JSON export for Outcomes and Patterns; ranking factors include bounded pattern influence
- End-to-end and CLI learning workflow tests; PROPOSAL and LEARNING boundary messaging

## 0.4.0 — Improvement Proposals

### Phase 1 — Proposal Model and Lifecycle (RFC-020)

- Durable Improvement Proposal Engineering Object, distinct from Recommendation, implementation, and outcomes
- Explicit Draft/Proposed/UnderReview/Accepted/Rejected/Deferred/Superseded/Withdrawn lifecycle
- Immutable revisions preserve content, feedback, actor, reason, timestamps, source evidence, and supersession links
- Lazy per-Investigation Proposal storage with deterministic listing and corrupted-record isolation
- Shared Runtime Capabilities with thin CLI and Workspace Proposal flows

### Phase 2 — Evidence-Backed Generation and Comparison (RFC-021)

- Deterministic local generation of bounded alternatives from current and labeled historical evidence
- Dismissed-context exclusion, visible contradictions, and explicit unverified-Hypothesis assumptions
- Inspectable comparison factors and priority explanations without opaque model-only selection
- Concrete but unexecuted implementation outlines and Verification Plans
- Feedback-driven refinement and bounded `propose_engineering_improvement` Composite Capability

### Phase 3 — Export and Experience (RFC-021)

- Sanitized, deterministic Markdown and JSON-compatible Proposal artifacts
- Bounded coding-agent implementation handoff text without agent invocation
- Investigation-level portfolio filters and evidence-to-Proposal traceability
- CLI stdout-only export and Workspace artifact, handoff, portfolio, and trace views
- Additive storage compatibility with v0.1-v0.3 and explicit no-application architecture tests

## 0.3.0 — Engineering Assistance

### Phase 1 — Composite Capabilities and Assisted Workflows (RFC-018)

- Core versus Composite Capability model with approved Composite definitions
- Durable Assisted Workflows with planned/running/completed/partial/failed/cancelled statuses
- Step records preserve capability, evidence, outputs, failures, skips, and confirmation gates
- Plan, execute, cancel, resume, retry, explain, and summarize workflow Capabilities
- Initial composites: investigate engineering problem, assess deployment readiness, explain failure
- CLI `assist` commands and Workspace Assistance session share CapabilityService

### Phase 2 — Expanded Engineering Connectors (RFC-012)

- GitHub Actions (CI) connector with fixture mode, rate-limit handling, secret redaction
- Kubernetes (infrastructure) connector with fixture mode and optional kubectl observe
- Sentry (observability) connector with fixture mode and secret redaction
- Connector status/test/collect CLI; Workspace connector status panel
- New Observation kinds: WorkflowRun, Infrastructure, Observability
- Read-only boundary enforced; no external mutations

### Phase 3 — Explainable Engineering Assistance (RFC-019)

- Ranked Hypotheses with supporting and contradicting evidence
- Next-best verification suggestions with feasibility and confidence impact
- Deployment readiness assessments (ready/hold/inspect/unknown) with blockers and dimensions
- Risk forecasts with categories, severity, historical comparison, and mitigations
- Probabilistic root-cause guidance (never unverified fact)
- Recommendation prioritization with inspectable ranking factors
- Durable engineering reports generated from Runtime data
- CLI report/assist surfaces and Workspace assistance flows

## 0.2.0 — Investigation Intelligence


### Phase 1 — Investigation Graph (RFC-015)

- Durable Investigation relationships with evidence, provenance, confidence, and human confirmation state
- Deterministic derived relationship kinds (shared repository/commit/PR/file, connector source, failure signatures, evaluation/verification/recommendation/learning overlap)
- Explicit human links, unlink of explicit links, confirm/dismiss without rewriting histories
- Idempotent relationship refresh; graph rebuildable from durable source records
- Capabilities: link, unlink, list related, explain, refresh, confirm, dismiss
- CLI investigation relationship commands and Workspace related-Investigations experience
- Investigation independence: relationships never merge Memory or rewrite conclusions

### Phase 2 — Search and Recall (RFC-016)

- Local-first Investigation search: exact id, structured filters, text token overlap, recency
- Similar Investigation discovery over inspectable signals with explained ranking factors
- Optional pluggable embedding provider with deterministic local token-hash baseline
- Every result explains relevance via matched evidence and weighted factors
- Recall related evidence and prior Learning Outcomes
- Capabilities shared by CLI (`search`, `recall`, `investigation similar`) and Workspace

### Phase 3 — Reusable Engineering Knowledge (RFC-017)

- Recalled Context records owned by the current Investigation (suggested / attached / dismissed)
- Suggest from related/similar Investigations; manual attach from a source Investigation
- Only attached context influences Evaluation and Recommendation; dismissed never does
- Historical influence is labeled in metadata and explanations; prior Recommendations are never auto-repeated
- Verification remains independent of historical context
- On-demand pattern detection with supporting Investigation and object ids
- Minimal historical trends (verification distribution, learning success rate, top repositories and failure signatures)
- CLI: `investigation context*`, `patterns`, `trends`; Workspace context, patterns, and trends views
- End-to-end cross-Investigation intelligence flow covered by tests

## 0.1.0 — Runtime Foundation

### Phase 1 — Core Runtime

- Engineering Object Model: Investigation, Observation, Memory Record, Knowledge, Evaluation, Verification Receipt, Recommendation, Learning Outcome
- Investigation lifecycle: Created → Collecting → Understanding → Evaluating → Verifying → Recommending → Learning → Completed
- Reopen Completed Investigations into Collecting without rewriting history
- Local filesystem persistence with append-only Memory
- Structured errors, stable UUIDs, provenance, serde serialization

### Phase 2 — Engineering Reasoning

- Observation ingestion with validation and idempotency keys
- Append-only Memory, chronological recall, timelines, corrections as new records
- Deterministic Knowledge derivation with evidence links
- Explainable Evaluations (risk, health, confidence, readiness)
- Verification Receipts (pass / fail / inconclusive)
- Evidence-backed Recommendations (proposals only; never auto-applied)
- Learning outcomes that influence future metadata without rewriting history
- End-to-end Observation → Learning pipeline tests

### Phase 3 — Capabilities, Connectors, Interfaces

- CapabilityService shared by CLI and Workspace
- Local project connector (read-only, observation-only)
- GitHub connector (narrow, read-only; live API + fixture mode)
- CLI: investigation, observe, recall, timeline, knowledge, evaluate, verify, recommend, learn, pipeline
- Interactive Workspace with complete Investigation workflow
- Architecture boundary tests and shared Runtime verification
