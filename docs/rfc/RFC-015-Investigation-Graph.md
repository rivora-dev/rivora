# RFC-015: Investigation Graph

**Status:** Implemented
**Target Version:** v0.2

# Purpose

RFC-013 establishes that "Knowledge may create relationships between
Investigations without merging them" and that Investigations may
"reference related Investigations." RFC-011 names the Capability
"Find Similar Investigations." Neither defines the mechanics.

This RFC defines the Investigation Graph: the durable, explainable
record of relationships between Investigations and their Engineering
Objects.

The Investigation Graph exists so Rivora can answer:

> Why are these Investigations related?

# Philosophy

* Relationships provide context. They never rewrite history.
* A relationship must never exist merely because an opaque similarity
  score says it does. Every relationship carries inspectable evidence.
* The graph is derived state. Primary Investigation Memory remains the
  source of truth and the graph can be rebuilt from it.
* Explicit human links and deterministic derivations are both
  first-class, and they are always distinguishable.

# Relationship Model

An **Investigation Relationship** connects exactly two Investigations.

Each relationship records:

* a stable identifier
* source Investigation ID
* target Investigation ID
* relationship kind
* confidence or strength
* supporting evidence (descriptions plus the Engineering Objects that
  justify the relationship)
* creation timestamp
* provenance
* derivation method (versioned, like Knowledge derivations)
* human confirmation state (unconfirmed, confirmed, or dismissed)

## Relationship Kinds

The v0.2 relationship vocabulary is deliberately small and inspectable:

* `shared_repository` — both Investigations observed the same repository
* `shared_commit` — both observed the same commit
* `shared_pull_request` — both observed the same pull request
* `shared_file_path` — both observed the same changed file path
* `shared_connector_source` — both were observed through the same
  connector source
* `similar_observations` — Observation kinds and text overlap beyond a
  documented deterministic threshold
* `shared_evaluation_category` — Evaluations share an assessment type
  and severity
* `related_verification_outcome` — Verification Receipts share an
  outcome
* `repeated_failure_signature` — the same normalized failure signature
  appears in both Investigations
* `related_recommendation` — Recommendations share a deterministic
  type signature
* `related_learning_outcome` — Learning Outcomes share a disposition
  over similar Recommendations
* `explicit_link` — a human created the relationship directly

New kinds require an RFC update before implementation.

## Directionality

Derived relationships are undirected; source and target are stored in
canonical order. Explicit links preserve the direction chosen by the
user. Lookups return relationships regardless of endpoint order.

## Evidence and Explainability

Every relationship includes at least one evidence item. Each evidence
item contains a human-readable description and the identifiers of the
Engineering Objects on either side that justify it. The Runtime must be
able to produce a complete explanation of any relationship from its
stored fields alone.

## Identity and Idempotency

Derived relationships have deterministic identifiers computed from
kind, canonical endpoint order, and a stable evidence key. Re-running
derivation over unchanged data produces the same relationships with the
same identifiers. Refresh is therefore idempotent.

Explicit links receive random identifiers; they are never created by
derivation and never removed by refresh.

# Graph Storage

Relationships persist under a dedicated graph area of the local store,
separate from per-Investigation directories:

```text
root/graph/relationships/{relationship-id}.json
```

Guarantees:

* relationships survive Runtime restart
* deleting a relationship never deletes or modifies an Investigation
* relationship updates preserve the original provenance
* graph corruption cannot corrupt primary Investigation Memory; the
  graph area is rebuilt from durable source records by refresh
* derived relationships are replaceable; explicit links are durable
  until explicitly removed

# Investigation Independence

Every Investigation remains an independent historical record.

The graph never:

* merges Investigation Memory
* moves Engineering Objects between Investigations
* rewrites previous conclusions
* collapses two Investigations into one
* modifies Learning Outcomes because a new relationship is found

# Derivation

Derivation is deterministic and lives entirely in the Runtime. For each
pair of Investigations the Runtime compares durable Engineering Objects
and emits relationships for every supported signal, each with a
versioned derivation method (for example `shared_repository_v1`,
`repeated_failure_signature_v1`, `explicit_link_v1`).

Refresh re-derives all derived relationships for one Investigation
against every other Investigation, replacing stale derived relationships
and preserving explicit links and human confirmation state where the
underlying evidence still holds.

# Capabilities

The graph is exposed only through Capabilities:

* Link Investigations
* Unlink Investigations
* List Related Investigations
* Explain Investigation Relationship
* Refresh Investigation Relationships
* Confirm Investigation Relationship
* Dismiss Investigation Relationship

Capabilities coordinate the Runtime. They contain no derivation logic.

# What the Investigation Graph Does Not Do

* It does not merge, move, or rewrite Investigations.
* It does not reason about current Investigations; it records
  relationships and evidence.
* It is not a general-purpose graph database; edges are typed domain
  relationships only.
* It does not use opaque similarity scores without evidence.

# Architectural Guarantees

* Every relationship carries evidence and a derivation method.
* Every relationship is explainable from stored fields.
* Derived relationship identity is deterministic; refresh is
  idempotent.
* Graph storage is separate from, and cannot corrupt, primary
  Investigation Memory.
* Explicit links are human-created and preserved across refreshes.
* Investigation histories remain unchanged by graph operations.

If these guarantees change, this RFC must be updated before
implementation.

# Summary

The Investigation Graph records why Investigations are related. It is
durable, explainable, deterministic, and derived. It connects
Investigations without ever rewriting them, and it is the foundation
for search, recall, and reusable engineering knowledge in v0.2.
