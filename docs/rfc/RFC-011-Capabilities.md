# RFC-011: Capabilities

**Status:** Foundational (extended by v0.7 Engineering Loop; first-party coverage in v0.8)  
**Target Version:** Foundation → v0.1; Engineering Loop participation in v0.7; complete first-party descriptors and coverage surface in v0.8

------------------------------------------------------------------------

# Purpose

This RFC defines the Capability model of the Rivora Runtime.

Capabilities are the Runtime's public interface.

If the Runtime is Rivora's reasoning engine, Capabilities are the
engineering operations through which every interface interacts with that
engine.

------------------------------------------------------------------------

# Philosophy

Capabilities express engineering intent.

They describe **what** Rivora should do, not **how** it is implemented.

Every interface---including the Workspace, CLI, APIs, SDKs, MCP servers,
and future integrations---invokes Capabilities instead of directly
calling Runtime subsystems.

------------------------------------------------------------------------

# Responsibilities

Capabilities are responsible for:

-   Accepting engineering intent
-   Coordinating Runtime subsystems
-   Producing Engineering Objects
-   Returning consistent results across interfaces
-   Hiding Runtime implementation details

Business logic remains inside the Runtime.

------------------------------------------------------------------------

# Capability Model

Every Capability should:

-   Have one engineering responsibility
-   Operate on the Engineering Object Model
-   Produce deterministic results for equivalent inputs
-   Return explainable outputs
-   Be reusable across every interface

Capabilities are composable.

------------------------------------------------------------------------

# Example Capabilities

Examples include:

-   Investigate
-   Verify
-   Learn
-   Recall Memory
-   Search Knowledge
-   Analyze Risk
-   Generate Timeline
-   Generate Report
-   Find Similar Investigations
-   Correlate Events
-   Summarize Investigation

These represent engineering intent rather than internal implementation.

------------------------------------------------------------------------

# Execution Model

Workspace CLI API SDK MCP

        ↓

Capability

        ↓

Runtime

        ↓

Engineering Objects

Every interface executes the same Capability.

Only presentation changes.

------------------------------------------------------------------------

# Composition

Capabilities may invoke other Capabilities.

For example, Investigate might compose:

-   Recall Memory
-   Search Knowledge
-   Analyze Risk
-   Verify
-   Generate Timeline

Complex workflows emerge from simple building blocks.

------------------------------------------------------------------------

# Runtime Relationship

Capabilities orchestrate Runtime behavior by coordinating:

-   Memory
-   Knowledge
-   Evaluation
-   Verification
-   Learning
-   Investigation Manager

Interfaces never invoke these subsystems directly.

------------------------------------------------------------------------

# Relationship to Connectors

Capabilities transform normalized engineering events into engineering
knowledge.

Connectors authenticate, collect events, normalize external data, and
produce canonical Engineering Objects and Observations. Capabilities
never communicate directly with external systems---they operate
exclusively on connector outputs.

The architectural relationship between Connectors and Capabilities is
defined in **RFC-028 --- Connectors and Capabilities**.

> **Architecture Note**
>
> Every Capability participates in the Engineering Loop:
>
> **Memory → Evaluation → Verification → Improvement → Learning**
>
> This lifecycle is shared by every Capability regardless of which
> Connector produced the engineering events.

------------------------------------------------------------------------

# What Capabilities Do Not Do

Capabilities do not:

-   own engineering state
-   duplicate Runtime logic
-   bypass the Runtime
-   directly communicate with external systems
-   implement interface-specific behavior

------------------------------------------------------------------------

# Architectural Guarantees

Capabilities guarantee:

-   A single public execution model.
-   Stable, intent-oriented operations.
-   Centralized Runtime logic.
-   Explainable Engineering Object outputs.
-   Consistent behavior across every interface.

If these guarantees change, this RFC must be updated before
implementation.

------------------------------------------------------------------------

# Summary

Capabilities are Rivora's execution language.

They expose engineering intent through a stable, reusable interface
while keeping the Runtime as the single source of engineering reasoning.

Whether invoked from the Workspace, CLI, API, SDK, or a coding agent,
every Capability executes the same Runtime behavior and returns the same
engineering understanding.

------------------------------------------------------------------------

# Related RFCs

-   RFC-006 --- Event Model
-   RFC-009 --- Memory
-   RFC-010 --- Verification
-   RFC-011 --- Capabilities
-   RFC-012 --- Connectors
-   RFC-013 --- Learning
-   RFC-028 --- Connectors and Capabilities
