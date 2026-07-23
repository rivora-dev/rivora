# Changelog

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
