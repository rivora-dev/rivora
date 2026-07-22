# Changelog

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
