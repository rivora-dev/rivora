# RFC-021: Proposal Generation, Comparison, Planning, and Export

**Status:** Proposed
**Target Version:** v0.4

# Purpose

RFC-020 defines the durable Improvement Proposal. This RFC defines how the
Runtime generates bounded alternatives, compares them explainably, refines
them from feedback, and renders read-only implementation artifacts.

# Deterministic Generation

The required baseline is local and deterministic. It reads existing durable
state only:

* Investigation objective
* Observations, Memory, and Knowledge
* Evaluations and Verification Receipts
* Hypotheses, readiness blockers, and risks
* Recommendations
* attached Recalled Context and its labeled historical sources
* referenced prior Learning Outcomes and Composite execution records

Generation never auto-derives or replaces source Knowledge, creates source
Evaluations or Verifications, or advances an Investigation lifecycle.
Suggested or dismissed Recalled Context never influences generation.
Unverified Hypotheses remain labeled unverified. Historical evidence remains
labeled historical.

Generation requires at least one durable evidence or assistance input; an
Investigation title alone is not an improvement opportunity. Every input is
recorded separately from its evidence role. Supported or verified Hypotheses
and passing Verification Receipts may support a Proposal. Contradicted or
rejected Hypotheses, failed receipts, and unsuccessful outcomes contradict it.
Inconclusive or otherwise neutral records remain generation inputs without
being mislabeled as support. Attached Recalled Context records themselves are
current provenance inputs, while their selected source objects remain labeled
historical. Prior Proposal-generation workflows are excluded to avoid circular
provenance.

The baseline identifies a verified or evidence-backed opportunity and emits a
smallest-useful-change alternative plus at least one bounded alternative. Each
Proposal records the versioned derivation method and every influencing object.

Optional model-assisted generation may later implement the same structured
contract, but v0.4 requires no hosted model and ships no model-only priority or
confidence source.

# Alternatives and Comparison

Alternatives are independent durable Proposal lineages grouped by a shared
improvement-opportunity identifier. Each records benefit, effort,
implementation risk, verification complexity, reversibility, architectural
fit, evidence strength, and drawbacks. Revisions stay within one lineage and
never collapse alternative histories.

Comparison returns ordered views, not an unexplained winner. Each view exposes
factor name, weight, contribution, and explanation. MVP factors are:

* evidence strength
* contradiction level
* expected impact and priority rationale
* implementation effort
* generation-method architectural fit
* reversibility
* verification feasibility
* labeled historical successful, unsuccessful, rejected, accepted, or ignored outcomes

Scores are presentation aids. Deterministic ties use Proposal id. The Runtime
prefers the smallest alternative that addresses the supported need; it never
selects a proposal as guaranteed correct.

# Priority

Priority is `Critical`, `High`, `Medium`, `Low`, or `Exploratory`. It considers
problem severity, recurrence, current impact, verified evidence, blocked work,
risk reduction, cost, reversibility, and urgency. Confidence alone never sets
priority. The explanation and factors are always inspectable.

# Implementation Outline and Verification Plan

Every generated Proposal contains a bounded expected scope: likely modules or
resources, object/storage/Capability/interface changes, tests, compatibility,
documentation, migrations when required, and release considerations.

Every Verification Plan contains claims, preconditions, tests/fixtures/static
checks/manual workflows, expected evidence, success/failure/inconclusive
conditions, and recovery checks. A Verification Plan is itself proposed work;
v0.4 does not execute the proposed implementation or destructive verification.

# Feedback and Refinement

Feedback records actor, timestamp, category, and comment. Content-changing
feedback creates a new immutable revision only through an explicit refinement
request. Feedback attached to one lineage never silently affects another.

# Composite Capability

`propose_engineering_improvement` is a bounded Composite Capability:

```text
recall existing proposal inputs
→ generate bounded alternatives
→ compare alternatives
→ summarize proposal ranking
```

It uses approved Core Capabilities only. It contains no unrestricted loop, no
acceptance step, and no application or external mutation step.

Each execution compares and summarizes only the alternative group it just
generated. Factor contributions are preserved in the durable workflow step
notes; prior opportunities and manual Proposals cannot enter that ranking.

# Artifacts and Handoff

The Runtime renders deterministic Markdown and structured JSON-compatible
artifacts from a durable Proposal. Artifacts include status, evidence on both
sides, historical labels, assumptions, scope, alternatives, plans, risks,
success criteria, unresolved questions, provenance, revisions, and a visible
no-application statement.

Artifacts contain the sanitized lineage snapshots in revision order. Markdown
renders snapshot and parent identifiers, actor, update time, refinement reason,
transition provenance, and feedback provenance without repeating cumulative
events on later snapshots. Corrupt Proposal revisions are isolated and exposed
as artifact diagnostics with a visible incomplete-history warning. Corrupt or
foreign artifact siblings do not prevent valid artifacts in the owning
Investigation from loading and are returned as listing diagnostics.

The coding-agent handoff is text only and must state:

> This is an implementation proposal. Review repository state and current code
> before acting. Do not treat suggested files or implementation details as
> authoritative without inspecting the repository. Do not exceed the approved
> Proposal scope.

v0.4 does not invoke a coding agent, write a repository, create a branch,
commit, push, open a pull request, deploy, or mutate external systems.

CLI export writes to stdout in Markdown or JSON using existing conventions.
The MVP intentionally does not add arbitrary output-path writes, eliminating
overwrite, traversal, symlink, and source-tree mutation risk.

# Portfolio and Traceability

An Investigation-level portfolio filters Proposals by status, priority,
category, source Recommendation, and affected component. It identifies
unresolved high-priority Proposals without becoming project management.

Traceability exposes the available chain:

```text
Observation → Memory → Knowledge → Evaluation → Verification
→ Recommendation → Improvement Proposal
```

An optional manually supplied external implementation reference may be stored
as inert text metadata. It does not prove implementation and never creates a
Learning Outcome.

# Security and Performance

Artifacts redact secret-like material, never embed raw environment variables,
and describe shell/code fragments as proposed text without executing them.
All operations are local, synchronous, bounded by one Investigation, and have
small deterministic performance tests. No queue, worker, remote database, or
hosted inference service is introduced.

# Acceptance Criteria

* At least two bounded alternatives are generated and compared.
* Every generation input and ranking factor is inspectable.
* Dismissed context is excluded; historical and current evidence remain distinct.
* Contradictions and unverified Hypotheses remain visible.
* Feedback refinement preserves the original revision.
* Verification Plans and implementation outlines are concrete but unexecuted.
* Markdown, structured artifacts, handoff, portfolio, and traceability work.
* CLI and Workspace share Runtime Capabilities.
* No repository, connector source, infrastructure, or external system changes.

# Summary

Rivora v0.4 turns durable engineering understanding into bounded, comparable,
exportable suggestions while keeping implementation under explicit human
control.
