# RFC-016: Search and Recall

**Status:** Implemented
**Target Version:** v0.2

# Purpose

RFC-013 guarantees that "Completed Investigations remain searchable."
RFC-011 names the Capabilities "Search Knowledge," "Recall Memory," and
"Find Similar Investigations." Neither defines query semantics, ranking,
or explanation.

This RFC defines how the Runtime searches prior Investigations, finds
similar Investigations, recalls related evidence, and explains every
result.

Search exists so Rivora can answer:

> Why did this Investigation appear in the results?

# Philosophy

* Retrieval starts deterministic and inspectable.
* Every result explains itself. A result without an explanation is a
  bug.
* Search is local-first. No hosted search service, no mandatory
  external AI provider.
* Semantic recall is optional, replaceable, and deterministic enough
  for tests.
* Ranking quality is not over-optimized in v0.2; ranking transparency
  is mandatory.

# Search Scope

Search reads durable Engineering Objects across Investigations:

* Investigation title, description, status, timestamps
* repositories, connector sources
* Observations, Memory, Knowledge
* Evaluations, Verification Receipts, Recommendations, Learning
  Outcomes
* relationship metadata
* artifact identifiers: file paths, commits, pull requests

Search never writes. It is a derived read over durable records and
works identically after Runtime restart.

# Search Modes

## Exact and structured search

Filters over Investigation ID, repository, status, date range,
connector source, verification result, recommendation outcome, and
relationship kind.

## Text search

Normalized lexical matching over human-readable fields: titles,
descriptions, Observation and Memory summaries, Knowledge summaries,
Evaluation explanations, Recommendation summaries and rationales, and
Learning notes.

## Similar Investigation discovery

Given an Investigation, rank others by inspectable signals: overlapping
repository, files or components, matching error signatures, Observation
kind overlap, Knowledge overlap, Evaluation category overlap,
Verification results, Recommendation signatures, outcomes, and existing
relationships.

No unexplained single similarity score: the score is always decomposed
into the factors that produced it.

# Semantic Recall

Semantic recall is an optional, local-first refinement of text matching.

The Runtime defines a pluggable embedding abstraction. The default
provider is a deterministic local baseline (hashed term-frequency
vectors with cosine similarity) requiring no network, no model
download, and no external provider. Alternative providers may be
plugged in without changing the search contract.

When a text query is present, the semantic similarity contributes one
documented ranking factor alongside lexical and structural factors.
When no provider is configured the deterministic baseline is used, so
local-first operation never breaks.

Rivora must never depend on one model provider.

# Ranking

Ranking combines weighted, inspectable factors:

| Factor | Weight |
| --- | --- |
| Exact Investigation ID match | 1.0 (short-circuits) |
| Explicit relationship to context Investigation | 0.9 |
| Shared commit / pull request / file path | 0.6 |
| Matching failure signature | 0.55 |
| Shared repository | 0.5 |
| Relationship (derived) to context Investigation | 0.45 |
| Recommendation outcome match | 0.4 |
| Knowledge overlap | 0.35 |
| Text token overlap | up to 0.35 |
| Semantic similarity | up to 0.3 |
| Evaluation category overlap | 0.25 |
| Verification result overlap | 0.2 |
| Recency | up to 0.1 |

A user-confirmed relationship or confirmed recalled relevance
multiplies the total by 1.2. Scores normalize to a maximum of 1.0.
Weights live in one documented location in the Runtime and are covered
by tests.

# Search Results

Every result includes:

* Investigation identifier, title, status, timestamps
* relevance explanation (human-readable)
* matching evidence: the factors that fired, details, and the
  Engineering Objects involved
* relationship information where one exists
* confidence or rank score
* important outcomes
* source provenance

# Recall

Recall returns historical evidence without attaching it to the current
Investigation (attachment is defined by RFC-017):

* Recall Related Evidence — evidence from related Investigations with
  its relationship explanation
* Recall Prior Outcomes — Learning Outcomes across Investigations,
  filterable by repository, similarity, and disposition

# Capabilities

* Search Investigations
* Find Similar Investigations
* Explain Search Result
* Recall Related Evidence
* Recall Prior Outcomes

Capabilities coordinate the Runtime. Interfaces never implement query
parsing, ranking, or recall logic.

# What Search and Recall Does Not Do

* It does not modify Investigations, Memory, or the graph.
* It does not attach historical evidence to a current Investigation
  (see RFC-017).
* It does not require network access or a hosted index.
* It does not rank by opaque scores.

# Architectural Guarantees

* Every result carries an explanation and matching evidence.
* Equivalent inputs produce equivalent results (deterministic for fixed
  durable state).
* Search works entirely locally and after Runtime restart.
* Semantic recall is optional, pluggable, and replaceable.
* Ranking factors are documented in code and tests.

If these guarantees change, this RFC must be updated before
implementation.

# Summary

Search and Recall make completed and active Investigations findable.
Retrieval is deterministic, local-first, and always explainable: every
result shows why it appeared and what evidence supports it.
