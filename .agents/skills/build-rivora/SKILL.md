---

name: build-rivora
description: Implement, modify, test, or review code in the Rivora repository while preserving its RFC-defined architecture, architectural invariants, release scope, and Red-Green-Refactor workflow. Use for any substantive Rivora coding task.
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

# Build Rivora

Use this skill whenever implementing, modifying, debugging, refactoring, or reviewing Rivora code.

Rivora is a specification-first, open-source Engineering Understanding Platform.

The goal is to move quickly while preserving the architecture. Do not add process or documentation unless it directly improves implementation quality.

## Read the Repository First

Before changing code:

1. Inspect the current repository state.
2. Read the nearest relevant repository instructions.
3. Read:

   * `VISION.md`
   * `PRINCIPLES.md`
   * `ARCHITECTURE.md`, when present
   * `ARCHITECTURAL_INVARIANTS.md`
   * `ROADMAP.md`
   * the current version’s implementation plan
   * only the RFCs relevant to the task
4. Inspect existing code, tests, naming, module boundaries, and local conventions before proposing new structure.

Do not reread every RFC for a small local change. Read enough context to understand the affected architectural boundary.

## Sources of Truth

Use this priority order when instructions appear to conflict:

1. The user’s explicit request
2. Architectural invariants
3. Approved RFCs
4. Current version scope and implementation plan
5. Existing repository conventions
6. This skill

Do not silently override a higher-priority source.

If the requested implementation conflicts with an architectural invariant or approved RFC:

* stop before making the conflicting change
* explain the conflict clearly
* identify the smallest RFC or architecture update required
* do not invent a replacement architecture inside the code

## Essential Architecture Rules

Preserve these rules in every implementation:

### Runtime

* The Runtime is the single source of engineering reasoning.
* Business logic belongs in the Runtime.
* Interfaces remain thin clients.

### Investigations

* Investigations are the primary unit of engineering understanding.
* Every Engineering Object belongs to one primary Investigation.
* Investigation history must remain durable.

### Memory

* Memory is append-only.
* Historical facts are not rewritten.
* Corrections create new records.
* Knowledge is derived from Memory.

### Knowledge

* Knowledge must preserve links to its supporting evidence.
* Knowledge must not become a second source of truth.

### Evaluation and Verification

* Evaluations must be explainable.
* Verification validates conclusions and produces durable receipts.
* Failed or inconclusive verification attempts must remain visible.

### Recommendations and Learning

* Recommendations are proposals, not facts.
* Recommendations must reference supporting evidence.
* Learning influences future reasoning.
* Learning never rewrites historical Investigations.

### Capabilities

* Capabilities express engineering intent.
* Capabilities coordinate Runtime behavior.
* Capabilities do not duplicate Runtime reasoning.
* Every interface uses the same Capability implementations.

### Connectors

* Connectors only observe, normalize, and produce Observations.
* Connectors do not evaluate, verify, recommend, or learn.
* Prefer read-only connectors unless the current version explicitly authorizes mutations.

### Interfaces

* Workspace is the primary interactive experience.
* CLI supports one-shot human and coding-agent execution.
* Interfaces must not contain duplicated business logic.

## Stay Within the Current Release

Before implementation, identify the active release and its approved scope.

Build only what belongs in that release.

For Rivora v0.1, do not introduce later roadmap features such as:

* cross-investigation graphs
* semantic organizational recall
* collaboration
* scheduled automation
* autonomous actions
* plugin marketplaces
* enterprise features
* multi-tenant SaaS infrastructure
* billing
* broad public APIs
* production MCP support

Small internal abstractions may prepare for later work, but do not build speculative frameworks without a current requirement.

Prefer the smallest architecture that correctly supports the current release.

## Required Development Loop

Follow Red → Green → Refactor for behavior changes.

### Red

* Write or update a focused test first.
* Run it.
* Confirm it fails for the intended reason.
* Do not treat compilation errors caused by unfinished test setup as the desired red state.

### Green

* Implement the smallest correct change that makes the test pass.
* Avoid speculative abstraction.
* Run the focused test.
* Run nearby subsystem tests.

### Refactor

* Improve clarity, naming, and structure.
* Remove duplication.
* Preserve architectural boundaries.
* Keep all tests green.

For tiny mechanical changes where a test-first cycle provides no meaningful value, use judgment, but still run the relevant validation.

## Testing Expectations

Use the smallest test layer that proves the behavior, then add broader coverage where risk warrants it.

### Unit tests

Use for:

* domain rules
* lifecycle transitions
* validation
* serialization
* deterministic reasoning
* error behavior

### Integration tests

Use for:

* subsystem boundaries
* persistence
* Capability-to-Runtime behavior
* connector normalization
* shared Workspace and CLI execution

### Architecture tests

Use where practical to ensure:

* interfaces do not own Runtime reasoning
* connectors do not depend on evaluation or learning modules
* Memory mutation paths remain append-only
* dependency direction matches the architecture

### End-to-end tests

Use for critical Investigation flows, especially:

```text
Observation
→ Memory
→ Knowledge
→ Evaluation
→ Verification
→ Recommendation
→ Learning
```

Do not add redundant tests merely to increase test counts.

## Implementation Behavior

Before writing code:

* inspect the affected modules and tests
* identify the RFCs and invariants involved
* determine the narrowest correct implementation
* note any mismatch between docs and current code

During implementation:

* follow existing Rust and repository conventions
* keep modules cohesive
* prefer explicit domain types over loosely structured data
* preserve provenance and stable identifiers
* make failures structured and explainable
* prefer deterministic behavior for MVP functionality
* avoid panic-based control flow
* avoid hidden global state
* avoid unnecessary dependencies
* avoid large unrelated refactors
* do not add placeholder `TODO` implementations and call the feature complete

When the existing code already provides a suitable abstraction, extend it rather than creating a competing subsystem.

## Documentation Rules

Keep documentation proportional to the work.

Update documentation when:

* public behavior changes
* command usage changes
* architecture changes
* a new Engineering Object or Capability is introduced
* release scope or acceptance criteria change
* the existing documentation would otherwise become false

Do not create new planning documents for routine implementation work.

When implementation legitimately changes an approved design, update the relevant RFC or plan in the same change rather than leaving documentation stale. Warp’s implementation skill follows the same principle of keeping specifications and code aligned throughout implementation.

## Validation

Before declaring work complete, run the repository’s canonical validation commands.

At minimum for Rust code:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Also run any repository-specific:

* architecture tests
* integration tests
* end-to-end tests
* release gate
* documentation checks
* installation or CLI smoke tests

Do not claim success when required validation was not run.

If a command cannot be run, state exactly what was not verified and why.

## Definition of Done

Work is complete only when:

* requested behavior is implemented
* the current release scope is respected
* relevant RFCs and invariants remain satisfied
* tests cover the important behavior
* focused and repository-wide validation passes
* public behavior is documented where necessary
* no unsupported claims, placeholders, or accidental architecture changes remain
* the final summary states what changed, what was verified, and any remaining limitations

## Final Report

End substantive implementation work with:

### Implemented

Summarize the completed behavior.

### Architecture

State which RFCs and invariants were involved and whether any architectural changes were required.

### Verification

List the exact validation commands run and their results.

### Remaining

List only genuine remaining work or risks. Do not invent follow-up work when the task is complete.

## Guiding Principle

Build depth before breadth.

For every decision, preserve:

1. An exceptional Runtime
2. A thoughtful Workspace
3. An extensible ecosystem

Rivora should become more capable by extending its coherent architecture—not by accumulating disconnected features.
