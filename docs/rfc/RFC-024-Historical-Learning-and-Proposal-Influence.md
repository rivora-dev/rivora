# RFC-024: Historical Learning and Proposal Influence

**Status:** Accepted  
**Target Version:** v0.5  
**Depends on:** RFC-010, RFC-020, RFC-021, RFC-022, RFC-023

---

# Purpose

Define how verified Measured Learning Outcomes become durable historical learning and how
that learning may influence future Proposal ranking—without rewriting history or overriding
current evidence.

---

# Learning Patterns

A **Learning Pattern** is an aggregate derived from verified Measured Learning Outcomes.
It is a derived summary, never a replacement for source Outcomes.

### Fields

- stable pattern id and revision
- title and normalized signature/category
- scope (proposal category, environment/component constraints)
- supporting / contradicting / mixed outcome ids (exact verified revisions)
- counts by classification
- confidence
- applicability constraints and known exceptions
- first/last observed
- status
- provenance

### Status

```text
Emerging → Supported
Supported | Emerging → Contested (contradictory later evidence)
Any active → Retired (reason; not deleted)
```

Contradictory Outcomes remain visible. Retirement creates history; it never deletes.

---

# Aggregation Rules

- Only Outcomes with `historical_learning_eligible` and status `Verified` may support patterns
- Group by proposal category, affected components, and expected-result signature
- Do not use uncontrolled free-text similarity as the sole grouping key
- Count each Outcome lineage once (latest eligible verified revision); never inflate via revisions
- An Outcome never counts as evidence for its own Proposal ranking circularly

---

# Lessons

Structured lessons extracted deterministically from verified Outcomes:

- what worked / failed
- conditions
- evidence strength
- proposal category
- implementation characteristics
- verification quality
- exceptions
- applicability constraints

Do not invent universal rules from a single narrow Outcome.

---

# Influence on Proposal Ranking

When generating or prioritizing Proposals, historical learning is **one advisory signal**.

### Principles

1. Current Investigation evidence remains primary  
2. Historical success rate never proves present correctness  
3. Contested or weak Patterns have limited influence  
4. Historical failure may warn or suggest alternatives; it never silently suppresses  
5. Current evidence can override historical success  
6. Influence must be explainable  

### Explanation surface

For a Proposal or ranking result, expose:

- which Patterns were considered
- relevance reason
- supporting and contradicting Outcomes
- influence magnitude and direction
- whether current evidence overrode history

Where feasible, ranking may be compared with and without historical influence.

---

# Capabilities

- `derive_learning_patterns`
- `list_learning_patterns` / `show_learning_pattern` / `compare_learning_patterns`
- `retire_learning_pattern`
- `explain_historical_influence`
- `export_measured_learning_outcome` / `export_learning_pattern`

---

# Storage

```text
learning/
  patterns/{pattern_id}.json
```

Root-level `learning/patterns` is created lazily. Indexes, if any, are rebuildable from
canonical Pattern and Outcome records.

Corruption of one Pattern must not block Investigation load.

---

# Compatibility

- Does not rewrite Recommendation Learning Outcomes
- Does not rewrite historical Investigations or Memory
- Additive only

---

# Limits

- No model training or online learning
- No autonomous application of lessons
- No silent rewrite of prior confidence or conclusions
