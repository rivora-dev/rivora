# RFC-017: Recalled Context and Reusable Knowledge

**Status:** Draft (Implemented)
**Target Version:** v0.2

# Purpose

RFC-007 states that Knowledge is "reusable" and "may span multiple
Investigations when relationships exist." RFC-010 charges Learning with
"building organizational engineering experience over time." Neither
defines how historical understanding enters a current Investigation
without corrupting it.

This RFC defines Recalled Context: the explicit, provenance-preserving
mechanism by which a current Investigation references historical
evidence, plus the deterministic pattern and trend summaries derived
across Investigations.

Recalled Context exists so Rivora can answer:

> What did we learn last time, and is it relevant here?

# Philosophy

* Historical intelligence informs current reasoning; it never becomes
  current fact.
* Recalled context is always labeled, always traceable, and always
  dismissible.
* Prior conclusions never automatically become current conclusions.
* Current Evaluations still explain their own reasoning; current
  Verification still verifies current claims.
* Patterns and trends are derived Knowledge, not immutable facts.

# Recalled Context

A **Recalled Context** record belongs to the current Investigation and
references a source Investigation. It records:

* stable identifier
* current Investigation ID (ownership)
* source Investigation ID
* source Engineering Object IDs
* a summary of the selected evidence
* the reason for recall
* the relationship or search explanation that justified it
* confidence
* timestamp and provenance
* origin: automatically recalled or explicitly selected
* state: suggested, attached, or dismissed

Recalled Context persists inside the current Investigation's storage
area, satisfying the single-ownership invariant. It is never appended
to the source Investigation and never appears in the source
Investigation's Memory.

## Lifecycle

* **Suggested** — the Runtime recalled the context automatically from
  related or similar Investigations.
* **Attached** — a human (or explicit caller) confirmed the context as
  relevant input to current reasoning.
* **Dismissed** — a human rejected the context; it remains recorded for
  explainability but never influences reasoning.

Only **attached** context influences current Evaluation and
Recommendation reasoning.

# Knowledge and Learning Reuse

When attached Recalled Context exists:

* Evaluations record the attached context identifiers in metadata and
  note the historical influence in their explanation. Current evidence
  remains the basis of the judgment; historical context is cited, not
  absorbed.
* Recommendations record historical influence in metadata and rationale:
  a prior similar Recommendation that was unsuccessful adds a visible
  warning; a successful prior outcome adds a visible note.
* Previous Recommendations are never automatically repeated.
* Verification is unchanged: it verifies current claims against current
  Memory.

Historical and current evidence remain distinguishable in every output.

# Pattern Detection

Patterns are deterministic derivations across Investigations. A pattern
requires support from at least two Investigations and records:

* pattern kind and normalized signature
* human-readable description
* supporting Investigation IDs and Engineering Object IDs
* occurrence count, confidence, derivation method, provenance

v0.2 pattern kinds:

* `recurring_failure_signature`
* `repeated_component`
* `recurring_recommendation`
* `frequent_inconclusive_verification`
* `repeated_successful_mitigation`
* `repeated_rejected_recommendation`
* `recurring_connector_evidence`
* `repeated_relationship`

Patterns are computed on demand from durable records. They are not
persisted and can always be recomputed.

# Historical Trends

Trends are minimal deterministic summaries over durable records:

* Investigations over time
* Verification result distribution (pass / fail / inconclusive)
* Learning Outcome distribution and recommendation success rate
* recurring repositories and failure signatures

Trends are computed on demand, exposed through a Capability, and
rendered simply. They are not analytics dashboards.

# Capabilities

* Suggest Recalled Context
* Attach Recalled Context
* Dismiss Recalled Context
* List Recalled Context
* Find Historical Outcomes
* Detect Investigation Patterns
* Summarize Historical Trend

Capabilities coordinate the Runtime. Interfaces never implement recall,
pattern, or trend logic.

# What Recalled Context Does Not Do

* It does not merge Investigations or their Memory.
* It does not silently inject historical conclusions into current
  Knowledge.
* It does not rewrite historical Investigations, including the source
  Investigation.
* It does not implement autonomous learning or model training.

# Architectural Guarantees

* Every Recalled Context record preserves provenance to its source
  Investigation and Engineering Objects.
* Historical and current evidence remain distinct in all outputs.
* Dismissed context never influences reasoning.
* Patterns reference the Investigations and evidence that support them.
* Reasoning that used historical context records that influence in
  metadata.
* No historical Investigation is rewritten by recall, patterns, or
  trends.

If these guarantees change, this RFC must be updated before
implementation.

# Summary

Recalled Context lets a current Investigation stand on prior work
without absorbing it. Historical evidence is recalled explicitly,
labeled clearly, confirmed or dismissed by humans, and cited — never
absorbed — by current reasoning. Patterns and trends summarize what the
organization keeps re-learning, always with their supporting evidence
attached.
