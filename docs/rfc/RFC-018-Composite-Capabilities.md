# RFC-018: Composite Capabilities and Assisted Workflows

**Status:** Implemented  
**Target Version:** v0.3

# Purpose

RFC-011 states that Capabilities are composable and may invoke other
Capabilities. RFC-014 defines a single execution model for every
interface. Neither defines how multi-step engineering intents are
planned, executed, inspected, cancelled, or resumed without inventing
an autonomous agent loop.

This RFC defines **Composite Capabilities** and **Assisted Workflows**:
the durable, inspectable mechanism by which Rivora coordinates existing
Core Capabilities into useful engineering assistance.

Rivora may assist. Rivora must not autonomously remediate.

# Philosophy

* Composite Capabilities express higher-level engineering intent.
* Core Capabilities remain the atomic operations of the Runtime.
* Workflows are planned from approved Core Capability sequences only.
* Every step is durable, explainable, and evidence-linked.
* Partial and failed workflows remain inspectable.
* Human confirmation is required before durable high-impact steps.
* No external system is mutated by a workflow in v0.3.
* CLI and Workspace invoke the same Composite Capability implementations.

# Core versus Composite

## Core Capabilities

Focused operations that already exist or are thin extensions, for
example:

* Recall Memory
* Search Investigations
* Find Similar Investigations
* Derive Knowledge
* Evaluate
* Verify
* Generate Recommendation
* Recall Prior Outcomes
* Detect Patterns
* Suggest Recalled Context
* Summarize Trends

Core Capabilities perform one engineering responsibility.

## Composite Capabilities

Higher-level intents that coordinate Core Capabilities, for example:

* `investigate_engineering_problem`
* `assess_deployment_readiness`
* `explain_failure`
* `generate_engineering_report`
* `recommend_next_verification`
* `summarize_investigation_state`

Each Composite Capability has:

* a stable identifier (string slug)
* declared intent
* required and optional inputs
* ordered Core Capability steps
* confirmation flags per step where required
* no embedded reasoning beyond sequencing and input binding

# Assisted Workflow Model

An **Assisted Workflow** is a durable execution record of a Composite
Capability run against one Investigation.

Fields:

* workflow identifier
* Investigation identifier
* composite capability identifier (intent)
* status
* ordered steps
* start and completion timestamps
* provenance
* optional final summary
* optional cancellation reason
* optional confirmation token / actor for confirmation-required steps

## Workflow status

```text
Planned
Running
Completed
PartiallyCompleted
Failed
Cancelled
```

## Step record

Each step records:

* step index and identifier
* Core Capability invoked
* status (`Planned`, `Running`, `Completed`, `Failed`, `Skipped`, `Cancelled`)
* input references (Investigation id, object ids, parameters)
* output references (object ids, structured result keys)
* evidence object ids used
* start and completion timestamps
* failure details
* skip reason
* whether confirmation is required
* whether confirmation was granted

# Execution Behavior

1. **Plan** — create a `Planned` workflow with ordered steps from the
   Composite Capability definition. Planning does not execute steps.
2. **Inspect** — list steps, status, and inputs before or during run.
3. **Execute** — run steps in order. Persist after each step.
4. **Confirm** — if a step requires confirmation and it has not been
   granted, stop with `PartiallyCompleted` or remain `Running` until
   confirmed or cancelled. Do not silently skip confirmation.
5. **Retry** — a failed safe step may be retried without re-running
   completed predecessors.
6. **Resume** — continue from the first incomplete step when status is
   `PartiallyCompleted` or after confirmation.
7. **Cancel** — mark remaining planned steps cancelled; preserve completed
   step outputs.
8. **Summarize** — produce an explainable summary from step outcomes.

## Failure model

* A step failure does not erase prior step outputs.
* Workflow status becomes `Failed` when an unrecoverable step fails and
  no further progress is possible, or `PartiallyCompleted` when some
  steps completed and others failed or await confirmation.
* Failed workflows remain durable.

## Determinism

Where Core Capabilities are deterministic, Composite execution must be
deterministic for equivalent Investigation state and plan.

The Runtime must not invent arbitrary tools or unbounded steps.

# Human Control

Confirmation is required before a workflow step that would:

* record a Learning Outcome
* transition an Investigation to `Completed`
* create a high-confidence Recommendation intended as a durable
  conclusion when the Composite Capability flags confirmation
* perform any operation defined confirmation-required by this RFC or
  a later approved RFC

No workflow may:

* merge code
* deploy software
* mutate infrastructure
* acknowledge alerts
* apply Recommendations automatically

# Capability Surface

Minimum Capabilities:

* Plan Composite Capability
* Execute / run workflow
* Inspect workflow
* List workflow steps
* Cancel workflow
* Resume workflow
* Retry failed step
* Summarize workflow
* Explain workflow decision (why a step ran, failed, or was skipped)
* List Composite Capability definitions

# Storage

Workflows persist under the Investigation they serve:

```text
investigations/{id}/workflows/{workflow_id}.json
```

This preserves single-Investigation ownership. Workflows never rewrite
source Investigation Memory histories; they only reference outputs.

# Interface Requirements

CLI and Workspace must:

* plan and run the same Composite Capabilities
* show step progress and intermediate evidence
* surface confirmation prompts for confirmation-required steps
* show partial and failed states
* never implement composition logic outside Capabilities

# Initial Composite Flows (MVP)

## Investigate Engineering Problem

```text
Recall Memory
→ Derive Knowledge
→ Find Similar Investigations
→ Suggest Recalled Context
→ Evaluate
→ Verify All
→ Generate Recommendation
→ Summarize Investigation State
```

## Assess Deployment Readiness

```text
Recall Memory
→ Derive Knowledge
→ Evaluate
→ Verify All
→ Assess Readiness (assistance)
→ Generate Recommendation
→ Generate Engineering Report
```

## Explain Failure

```text
Recall Memory
→ Derive Knowledge
→ Search / Find Similar
→ Generate Hypotheses (assistance)
→ Evaluate
→ Recommend Next Verification
→ Summarize Investigation State
```

Assistance Core steps introduced in RFC-019 are invoked by name as Core
Capabilities once implemented. Phase 1 may sequence existing Core
Capabilities only and still prove the workflow architecture.

# Out of Scope

* Autonomous multi-agent loops
* Unrestricted tool invention
* External mutation
* Background scheduling of workflows
* Marketplace of Composite Capabilities
* Cross-Investigation workflow ownership

# Acceptance Criteria

* At least three Composite Capabilities plan and execute end to end
* Every step records Core Capability, status, and evidence references
* Partial and failed workflows remain durable and inspectable
* Cancel and resume work for safe steps
* Confirmation gates block high-impact steps
* CLI and Workspace share Capability implementations
* No external mutation occurs
* Architecture boundary tests remain green

# Summary

Composite Capabilities turn Core Capabilities into inspectable assisted
workflows. Rivora coordinates understanding; humans remain in control.
