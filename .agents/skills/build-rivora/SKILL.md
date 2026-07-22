---

name: build-rivora
description: Build, review, test, integrate, and release Rivora while preserving its architecture, RFCs, release discipline, and engineering standards. Use for every substantive change to the Rivora repository.
------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

# Rivora Engineering Playbook

This document defines how Rivora is engineered.

It is the canonical engineering workflow for both humans and coding agents.

Its purpose is **not** to maximize code output.

Its purpose is to maximize long-term architectural coherence while allowing Rivora to move quickly as an open-source MVP.

Every substantial implementation, review, integration, and release must follow this playbook.

---

# Engineering Philosophy

Rivora is built around one principle:

> Build depth before breadth.

Every engineering decision should strengthen one coherent Runtime instead of accumulating disconnected features.

When forced to choose between:

* more features
* cleaner architecture

Choose cleaner architecture.

When forced to choose between:

* more abstraction
* simpler implementation

Choose the simpler implementation that satisfies the current requirements.

When forced to choose between:

* speed today
* maintainability next year

Choose maintainability without introducing unnecessary process or premature infrastructure.

The Runtime is the product foundation.

Everything else exists to expose, validate, extend, or improve that Runtime.

Rivora’s three product pillars are:

1. An exceptional Runtime
2. A thoughtful Workspace
3. An extensible ecosystem

---

# Engineering Lifecycle

Every roadmap version follows the same lifecycle:

```text
Plan
    ↓
RFC
    ↓
Implementation
    ↓
Local Review
    ↓
Validation
    ↓
Integration
    ↓
Release
    ↓
Next Version
```

A version is **not complete** when implementation finishes.

A version is complete only after:

* implementation is complete
* local review is complete
* focused and full validation pass
* the exact integrated `main` commit passes validation
* `main` is pushed and verified
* the version is tagged
* the GitHub Release is published
* release installation or build verification passes

Only then should work begin on the next roadmap version.

Implementation completion and release completion are separate states.

---

# Mandatory Context Acquisition

Before proposing architecture, writing an implementation plan, changing code, or creating files, understand the current repository.

Do not begin implementation until the following context-acquisition steps are complete.

## 1. Inspect Repository State

Inspect:

```sh
git status --short
git branch --show-current
git rev-parse HEAD
git rev-parse origin/main
git log --oneline --decorate -10
git remote -v
```

Also inspect:

* repository layout
* workspace and crate structure
* current version metadata
* current roadmap version
* recent implementation history
* files already modified or untracked
* existing branches and release tags where relevant

Determine:

* what release is currently active
* whether the repository is safe to modify
* whether the working tree is clean
* whether local state matches remote state
* whether unfinished work already exists

Never overwrite or discard unrelated work.

---

# Repository Reading Order

Before changing code:

1. Inspect repository state.

2. Read repository instructions.

3. Read, in this order:

   1. `VISION.md`
   2. `PRINCIPLES.md`
   3. `ARCHITECTURE.md`, when present
   4. `ARCHITECTURAL_INVARIANTS.md`
   5. `ROADMAP.md`
   6. the current `IMPLEMENTATION_PLAN.md`
   7. `CHANGELOG.md`
   8. only the RFCs relevant to the current task

4. Inspect the existing:

   * source code
   * tests
   * naming conventions
   * module boundaries
   * storage conventions
   * serialization formats
   * Capability implementations
   * connector boundaries
   * CLI behavior
   * Workspace behavior
   * release and validation scripts

Do not redesign architecture without understanding the existing implementation.

Do not read every RFC automatically for a small local task. Identify the affected subsystems and read every RFC governing those boundaries.

If a task crosses several subsystems, read all relevant RFCs before planning implementation.

---

# RFC Discovery

Determine which RFCs govern the requested work before implementation begins.

Relevant categories may include:

* Vision and principles
* Interaction model
* Engineering Object Model
* Runtime
* Observations and events
* Memory
* Knowledge
* Evaluation
* Verification
* Learning
* Capabilities
* Connectors
* Investigation lifecycle
* Runtime execution
* Investigation Graph
* Search and recall
* Reusable engineering knowledge
* Future release-specific architecture

Use the repository’s actual canonical RFC numbers and titles.

Never rely on remembered RFC numbering when the checked-in repository can be inspected.

If the requested feature introduces a genuinely new architectural concept:

1. Confirm that the concept is not already covered.
2. Determine whether it can be implemented as a natural extension.
3. Create or update the smallest necessary RFC.
4. Keep the RFC proportional to an MVP.
5. Do not create architectural documentation as ceremony.

---

# Existing Implementation Review

Before proposing new modules or abstractions, inspect:

* affected Runtime services
* Engineering Object types
* storage interfaces and paths
* Capability interfaces
* connector implementations
* CLI commands
* Workspace flows
* unit tests
* integration tests
* architecture tests
* end-to-end tests
* migrations and compatibility fixtures

Prefer extending an existing abstraction over creating a competing abstraction.

Do not preserve obsolete pre-restart Rivora behavior merely because code for it exists.

The current architecture, approved RFCs, and architectural invariants define Rivora.

---

# Repository Understanding Report

Before beginning a substantial implementation, produce a concise orientation report.

Include:

## Current Repository State

* branch
* HEAD
* working tree
* relationship to `origin/main`
* current version

## Active Release

* release name
* release goal
* approved phases
* explicit out-of-scope work

## Relevant Architecture

* relevant RFCs
* architectural invariants
* affected Runtime boundaries

## Existing Implementation

* affected crates and modules
* existing Capabilities
* existing storage
* existing CLI and Workspace behavior

## Existing Tests

* relevant test suites
* current expected behavior
* known coverage gaps

## Proposed Approach

* narrowest correct implementation
* likely files or modules affected
* testing strategy
* compatibility requirements

## Risks

* architectural risks
* persistence or migration risks
* provenance risks
* backward-compatibility risks
* UX risks

Keep this report concise. Its purpose is orientation, not additional ceremony.

Only after this context has been acquired should implementation planning begin.

---

# Source of Truth

When instructions conflict, use this priority:

1. Explicit user request
2. `ARCHITECTURAL_INVARIANTS.md`
3. Approved RFCs
4. Current implementation plan
5. Roadmap
6. Existing repository conventions
7. This playbook

Never silently override a higher-priority source.

If an explicit request conflicts with an architectural invariant or approved RFC:

* stop before implementing the conflicting behavior
* explain the conflict clearly
* identify the smallest architectural update required
* do not invent competing architecture inside implementation
* do not silently change the RFC or invariant

Documentation and code must describe the same system.

---

# Architectural Invariants

The following rules must remain true.

## Runtime

* The Runtime owns engineering reasoning.
* Business logic belongs in the Runtime.
* Interfaces remain thin.
* There is one coherent Runtime, not separate reasoning implementations for each interface.

## Investigations

* An Investigation is the primary unit of engineering understanding.
* Every Engineering Object belongs to one primary Investigation.
* Investigation history is durable.
* Investigation histories remain independent.
* Relationships between Investigations never merge or rewrite their histories.

## Memory

* Memory is append-only.
* Historical Memory is never rewritten.
* Corrections create additional records.
* Knowledge derives from Memory.
* Memory remains the historical source of truth.

## Knowledge

* Knowledge references supporting evidence.
* Knowledge never replaces Memory as the source of truth.
* Historical and current Knowledge remain distinguishable.
* Recalled historical context must retain provenance.
* Historical conclusions never silently become current facts.

## Evaluation

* Evaluations are explainable.
* Evaluations are evidence-backed.
* Evaluations preserve supporting references.
* Deterministic behavior is preferred whenever practical for the MVP.

## Verification

* Verification validates conclusions.
* Verification produces durable receipts.
* Failed and inconclusive verification attempts remain visible.
* Historical Verification Receipts may inform current reasoning but do not replace current verification.

## Recommendations

* Recommendations are proposals.
* Recommendations are never facts.
* Recommendations are never automatically applied unless a future approved release explicitly permits it.
* Recommendations preserve supporting evidence and verification references.

## Learning

* Learning influences future reasoning.
* Learning never rewrites historical Investigations.
* Prior outcomes remain labeled as historical context.
* Rejected, failed, ignored, and inconclusive outcomes remain visible.

## Capabilities

* Capabilities express engineering intent.
* Capabilities coordinate Runtime behavior.
* Capabilities never duplicate Runtime reasoning.
* CLI and Workspace use the same Capability implementations.

## Connectors

Connectors may:

* observe external systems
* normalize external data
* produce Observations

Connectors may not:

* evaluate
* verify
* recommend
* learn
* contain Runtime business logic

Prefer read-only connectors unless the active roadmap version explicitly authorizes mutation.

## Interfaces

The Workspace is the primary interactive experience.

The CLI exists for:

* one-shot human execution
* one-shot coding-agent execution
* scripting and structured output where supported

Both use the same Capability and Runtime layers.

Interfaces present and collect information. They do not implement engineering reasoning.

---

# Release Scope Discipline

Before implementation:

1. Identify the active roadmap version.
2. Identify its approved phases.
3. Identify the question that release must answer.
4. Identify explicitly deferred work.
5. Confirm the requested task belongs to the active release.

Build only the active version.

Do not introduce future roadmap functionality.

Prefer the smallest architecture that satisfies current requirements.

Small extension points are acceptable when required by the current design.

Speculative frameworks are not.

Do not add collaboration, automation, SDKs, marketplaces, enterprise features, hosted infrastructure, or other later-version functionality before the roadmap authorizes them.

---

# Development Workflow

Every meaningful behavior change follows:

```text
Red
    ↓
Green
    ↓
Refactor
```

## Red

* Write or update a focused test first.
* Run the test.
* Confirm that it fails for the intended behavioral reason.
* Confirm the failure is caused by missing or incorrect behavior.

Compilation failures, broken test setup, missing imports, and unrelated failures are not valid Red states.

## Green

* Implement the smallest correct behavior.
* Avoid speculative abstraction.
* Run the focused test.
* Run nearby subsystem tests.
* Stop adding code once the intended behavior passes.

## Refactor

Improve:

* naming
* clarity
* structure
* cohesion
* error handling

Remove duplication.

Preserve architectural boundaries.

Keep all tests green.

For tiny mechanical changes where test-first development provides no meaningful value, use judgment, but always run the relevant validation afterward.

---

# Testing Philosophy

Use the smallest test layer that proves the behavior, then add broader tests where risk requires them.

The objective is confidence, not test-count growth.

## Unit Tests

Use for:

* domain rules
* validation
* lifecycle transitions
* serialization
* deterministic reasoning
* provenance
* ranking factors
* idempotency
* errors
* relationship rules
* context state transitions
* pattern and trend calculations

## Integration Tests

Use for:

* persistence
* migrations
* Runtime boundaries
* Capability-to-Runtime behavior
* connectors
* CLI behavior
* Workspace behavior
* shared CLI and Workspace execution
* schema compatibility
* restart and reload behavior

## Architecture Tests

Verify where practical:

* dependency direction
* Runtime ownership of reasoning
* thin interfaces
* observation-only connectors
* append-only Memory
* Investigation independence
* historical and current evidence separation
* Capabilities coordinating rather than implementing reasoning
* storage boundaries
* graph and context operations not rewriting primary Investigation history

## End-to-End Tests

Verify representative engineering workflows.

Each release should include an end-to-end test demonstrating the primary product question for that release.

Do not remove or weaken prior-version regression tests.

Previous releases form the compatibility foundation for future releases.

---

# Documentation Rules

Documentation should remain proportional to the work.

Update documentation whenever:

* public behavior changes
* CLI behavior changes
* Workspace behavior changes
* architecture changes
* Engineering Objects change
* Capabilities change
* connectors change
* persistence or migrations change
* release scope changes
* RFC status changes
* version metadata changes
* installation instructions change

Do not generate new planning documents for routine implementation work.

Documentation must describe what actually ships.

When implementation legitimately changes an approved design, update the relevant RFC or implementation plan in the same coherent change.

Never leave known documentation drift behind without explicitly reporting it.

---

# Implementation Behavior

Before writing code:

* inspect affected modules
* inspect affected tests
* inspect relevant RFCs
* inspect invariants
* inspect persistence and compatibility behavior
* determine the narrowest correct implementation

During implementation:

* follow Rust and repository conventions
* preserve provenance
* preserve stable identifiers
* prefer explicit domain types
* use structured errors
* avoid hidden global state
* avoid unnecessary dependencies
* avoid panic-based control flow
* avoid unrelated refactors
* avoid placeholder implementations
* avoid silent error suppression
* keep deterministic MVP behavior where practical
* maintain local-first operation where required
* keep interfaces and connectors thin

Extend existing abstractions before creating new ones.

Do not call work complete while it contains unresolved placeholder code, unsupported claims, or accidental artifacts.

---

# Local Review Workflow

GitHub is not where correctness is first discovered.

Correctness is established locally before integration.

The absence of a pull request does not reduce the review standard.

Do not open a pull request unless the user explicitly requests one.

## Repository Review

Review:

* repository state
* complete diff against `origin/main`
* every commit
* changed public APIs
* migrations
* persistence formats
* provenance
* tests
* documentation
* version metadata
* release metadata
* generated files
* untracked files

Look for:

* architectural drift
* boundary violations
* duplicated Runtime logic
* Memory rewrites
* Investigation history mutation
* historical/current evidence contamination
* missing provenance
* weak explainability
* opaque ranking or derivation
* migration gaps
* compatibility regressions
* unsafe persistence
* panic-based flow
* swallowed errors
* weak tests
* dead code
* placeholder code
* unnecessary dependencies
* release-scope leakage
* accidental artifacts

Passing tests are necessary.

They are not sufficient.

---

# Structured Engineering Review

Review the completed release from these perspectives:

## Architecture

* RFC compliance
* invariant compliance
* Runtime boundaries
* Capability boundaries
* connector boundaries
* Investigation independence

## Runtime and Persistence

* domain correctness
* storage safety
* provenance
* stable identifiers
* idempotency
* migration behavior
* restart behavior
* data corruption isolation

## Security and Reliability

* untrusted input handling
* path handling
* connector input handling
* corrupted-record behavior
* resource usage
* meaningful errors
* failure isolation

## Tests

* behavioral coverage
* regression coverage
* architecture coverage
* migration coverage
* meaningful edge cases
* end-to-end release flow

## CLI and Workspace UX

* command clarity
* navigation
* explanations
* errors
* consistency
* structured output
* terminal restoration

## Release Scope

* active version implemented
* later-version work absent
* no speculative frameworks
* documentation matches shipped behavior

Classify findings as:

* `BLOCKER`
* `HIGH`
* `MEDIUM`
* `LOW`
* `NIT`

## Required Resolution Policy

Resolve every `BLOCKER`.

Resolve every `HIGH`.

Resolve every `MEDIUM` finding that affects:

* correctness
* provenance
* architectural invariants
* data integrity
* migrations
* backward compatibility
* public APIs
* externally visible behavior
* security
* reliability
* significant CLI usability
* significant Workspace usability

A `MEDIUM` finding may be deferred only when it is genuinely non-blocking for the current MVP release.

Every deferred `MEDIUM` finding must include:

* affected file or subsystem
* description of the risk
* reason it is safe to defer
* expected follow-up action
* backlog or issue reference when one exists

Never use `MEDIUM` as a convenience category for unresolved correctness, provenance, architecture, migration, compatibility, API, security, or reliability problems.

Low findings and nits may be deferred when they do not endanger the MVP, but they must still be reported honestly.

---

# Validation Workflow

Run focused subsystem validation first.

Add regression coverage for every resolved review finding where appropriate.

Then run full repository validation.

At minimum:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo build --workspace --release
```

Also run all available:

* architecture tests
* migration tests
* backward-compatibility tests
* connector tests
* CLI smoke tests
* Workspace smoke tests
* end-to-end tests
* documentation checks
* installation checks
* release gate scripts

Do not weaken linting.

Do not delete valid tests to obtain a green result.

Do not skip required features.

If validation fails:

1. Diagnose the root cause.
2. Add or update a regression test where appropriate.
3. Fix using Red → Green → Refactor.
4. Review the resulting diff.
5. Rerun focused validation.
6. Rerun the full suite.

---

# Manual Verification

Use an isolated temporary directory, data store, or checkout.

Never pollute the canonical checkout or user data.

Verify the representative release workflow, including:

* creation and loading
* persistence
* restart behavior
* backward compatibility
* CLI execution
* Workspace execution
* connector behavior
* error handling
* terminal restoration
* release-specific features
* historical-data integrity
* out-of-scope features remaining absent

Manual verification does not replace automated testing.

Automated testing does not eliminate the need for a representative manual workflow.

---

# Integration Readiness Gate

Do not integrate unless:

* working tree is clean
* repository state is understood
* complete diff was reviewed
* every commit was reviewed
* approved RFCs are implemented
* architectural invariants are preserved
* focused tests pass
* full validation passes
* backward compatibility passes
* migration tests pass where applicable
* manual verification passes
* documentation is accurate
* version metadata is consistent
* no unrelated work exists
* no `BLOCKER` remains
* no `HIGH` remains
* no unresolved `MEDIUM` affects correctness, provenance, architecture, data integrity, migrations, compatibility, public APIs, security, reliability, or significant UX
* every deferred `MEDIUM` is explicitly documented
* the branch is based on current `origin/main`

If any gate fails, do not integrate or push.

---

# Integration Workflow

Integration happens only after local review and validation.

GitHub is the publication destination, not the first quality-control environment.

## 1. Update Local Main

Prefer:

```sh
git switch main
git fetch origin
git pull --ff-only origin main
```

## 2. Integrate the Release Branch

Prefer:

```sh
git merge --ff-only <release-branch>
```

If fast-forward integration is impossible:

* stop
* inspect the divergence
* understand every new commit
* do not automatically create a merge commit
* do not force integration

Do not squash coherent release-phase commits unless repository policy or the user explicitly requires it.

## 3. Inspect Integrated Main

Verify:

```sh
git status
git log --oneline --decorate -10
git diff origin/main...HEAD --stat
```

Confirm that local `main` contains exactly the intended reviewed commits.

## 4. Validate the Exact Merged Main Commit

This step is mandatory.

Run the complete validation suite again on local `main`.

Tests passing on the feature branch do not prove that the exact integrated `main` commit is safe.

Only the exact validated local `main` SHA may be pushed.

## 5. Push Main

Before pushing, confirm:

* current branch is `main`
* working tree is clean
* commits ahead of `origin/main` are exactly the reviewed work
* exact `HEAD` passed every required gate

Then:

```sh
git push origin main
```

Never force-push.

## 6. Verify Remote Equality

After pushing:

```sh
git fetch origin
git status
git rev-parse HEAD
git rev-parse origin/main
```

Confirm:

* local `main` equals `origin/main`
* working tree is clean
* no commits remain ahead or behind

Inspect remote CI when present.

If remote CI fails:

1. Inspect the exact failure.
2. Fix the issue on `main`.
3. Add regression coverage where appropriate.
4. Rerun full local validation.
5. Push the focused fix.
6. Verify remote CI again.

Do not claim successful integration while required remote checks are failing.

---

# Release Workflow

Integration and release publication are separate controlled stages.

A roadmap version is not complete until both stages are complete.

Do not begin the next roadmap version merely because code has reached `main`.

## Release Readiness

Do not tag or publish unless:

* local `main` equals `origin/main`
* working tree is clean
* exact `main` commit passed full validation
* remote CI is green when present
* version metadata is correct
* `CHANGELOG.md` is complete
* README and installation instructions are current
* RFC statuses are correct
* release notes reflect only shipped behavior
* no unresolved release blocker remains

If any condition fails:

* do not tag
* do not publish
* report the blocker

## Version Verification

Inspect:

* workspace and crate versions
* `Cargo.toml`
* `Cargo.lock`
* CLI version output
* Workspace version output
* README references
* installation examples
* changelog heading
* release scripts
* GitHub workflows
* package or installer metadata

Historical version references may remain when clearly historical.

Active release references must be consistent.

## Tag Verification

Before creating a tag:

```sh
git tag --list vX.Y.Z
git ls-remote --tags origin vX.Y.Z
```

If the tag exists:

* inspect its target
* never overwrite it
* never move a published release tag
* report any mismatch

## Create Annotated Tag

Create the tag on the exact validated `main` commit:

```sh
git tag -a vX.Y.Z -m "Rivora vX.Y.Z — <Release Name>"
```

Verify:

```sh
git rev-list -n 1 vX.Y.Z
git rev-parse HEAD
git show vX.Y.Z --no-patch
```

The tag target must equal the exact validated `main` commit.

Push:

```sh
git push origin vX.Y.Z
```

Never force-push or overwrite a tag.

## Publish GitHub Release

Publish a GitHub Release when the release workflow is requested or when completing a roadmap version under this playbook.

Use the approved changelog section as the source for release notes.

Release notes must:

* identify the release goal
* summarize shipped phases
* describe important user-facing behavior
* mention compatibility or migration requirements
* state important constraints
* avoid claiming out-of-scope features

Verify:

* release title
* tag
* target commit
* draft state
* prerelease state
* release notes
* expected assets

Do not invent unsupported release artifacts.

## Fresh Installation Verification

Use a temporary environment or checkout.

Prefer testing the published artifact or installer.

When no published artifact exists, clone or check out the release tag and build from that clean state.

Verify at minimum:

```sh
rivora --version
rivora --help
rivora-workspace --version
```

Also verify where supported:

* Workspace launch
* terminal restoration
* creation of an Investigation
* one representative release workflow
* connector startup
* clean exit behavior

If installation or the release-tag build fails, the release is not complete.

## Final Release Verification

Confirm:

* local `main` equals `origin/main`
* release tag exists locally and remotely
* tag points to validated `main`
* GitHub Release exists
* release is not accidentally a draft or prerelease
* expected assets are present
* fresh-install verification passed
* canonical checkout remains clean

Only then may the next roadmap version begin.

---

# Reporting Requirements

Every substantial implementation should report:

## Repository Review

* branch
* starting SHA
* base `origin/main`
* commits reviewed
* files changed
* working tree

## Repository Understanding

* active release
* relevant RFCs
* affected modules
* existing tests
* proposed approach
* risks

## Review Findings

Report:

* Blocker
* High
* Medium
* Low
* Nit

For every deferred `MEDIUM`, include:

* file or subsystem
* risk
* reason for deferral
* follow-up action

## RFC Compliance

* RFCs implemented
* RFCs changed
* unresolved specification mismatches

## Architectural Invariants

* Runtime boundaries
* Investigation independence
* Memory behavior
* provenance
* historical/current separation
* connector boundaries
* interface boundaries

## Focused Validation

List suites and results.

## Full Validation

List exact commands, test counts, and results.

## Manual Verification

Describe the isolated workflow and results.

## Integration

* integration method
* exact merged `main` SHA
* commits added
* validation of merged `main`
* working tree

## Remote Push

* push result
* local SHA
* remote SHA
* equality
* CI status

## Release

When applicable:

* version
* tag
* tag target
* GitHub Release
* assets
* fresh-install verification

## Remaining Risks

List genuine remaining work only.

Do not invent follow-up work when the release is complete.

---

# Integration Gate Result

For integration work, return exactly one:

```text
PASS — Rivora was reviewed, fully validated, integrated into main, and pushed successfully.
```

or:

```text
FAIL — Rivora was not safely integrated or pushed.
```

If it fails, list every exact blocker.

---

# Release Gate Result

For release work, return exactly one:

```text
PASS — Rivora vX.Y.Z was tagged, published, and verified successfully.
```

or:

```text
FAIL — Rivora vX.Y.Z was not fully published or verified.
```

If it fails, list every exact blocker.

---

# Definition of Done

Implementation work is complete only when:

* requested behavior exists
* active release scope is respected
* approved RFCs remain satisfied
* architectural invariants remain satisfied
* meaningful tests pass
* documentation is current
* no accidental architecture change remains

Integration work is complete only when:

* local review is complete
* required findings are resolved
* feature-branch validation passes
* exact merged `main` validation passes
* `main` is pushed
* local and remote `main` match

A roadmap version is complete only when:

* implementation is complete
* integration is complete
* the version is tagged
* the GitHub Release is published
* release installation or build verification passes
* the canonical checkout is clean

Implementation complete is **not** project complete.

Integration complete is **not** release complete.

Only a verified release completes a roadmap version.

---

# Guiding Principles

For every engineering decision, preserve:

1. An exceptional Runtime
2. A thoughtful Workspace
3. An extensible ecosystem

Understand before implementing.

Test before trusting.

Review before integrating.

Validate before pushing.

Tag and verify before beginning the next version.

Rivora becomes more capable by deepening one coherent architecture—not by accumulating disconnected features.
