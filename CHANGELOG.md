# Changelog

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
