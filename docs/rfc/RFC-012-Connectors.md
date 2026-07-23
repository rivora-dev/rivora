# RFC-012: Connectors

**Status:** Foundational (boundary clarified with RFC-028 / v0.7; canonical coverage in v0.8)\
**Target Version:** Foundation → v0.1; Connector/Capability boundary formalized in v0.7; first-party connector inventory and non-reasoning gates in v0.8

------------------------------------------------------------------------

# Purpose

This RFC defines the Connector model of the Rivora Runtime.

Connectors are responsible for observing external engineering systems
and translating those observations into Rivora's Engineering Object
Model.

If Capabilities define **what Rivora can do**, Connectors define **how
Rivora learns about the outside world**.

------------------------------------------------------------------------

# Philosophy

Connectors observe.

They do not reason.

They do not evaluate.

They do not verify.

They do not learn.

A Connector has one responsibility:

> **Observe external systems and produce standardized Observations.**

All engineering reasoning belongs to the Runtime.

------------------------------------------------------------------------

# Responsibilities

Connectors are responsible for:

-   Connecting to external systems
-   Receiving engineering events
-   Normalizing external data
-   Producing Runtime Observations
-   Preserving source metadata
-   Delivering observations to the Runtime

Everything after that belongs to the Runtime.

------------------------------------------------------------------------

# Connector Pipeline

``` text
External System
        │
        ▼
    Connector
        │
Normalize Observation
        │
        ▼
      Runtime
        │
        ▼
Observation → Memory → Knowledge → Evaluation
```

Connectors stop after creating Observations.

The Runtime performs every downstream operation.

------------------------------------------------------------------------

# Supported Sources

Examples include:

-   GitHub
-   GitLab
-   AWS
-   Kubernetes
-   Cloudflare
-   Terraform
-   CI/CD systems
-   Observability platforms
-   Issue trackers
-   Internal engineering tools

The Runtime should not require special logic for any individual
connector.

------------------------------------------------------------------------

# Normalization

External systems expose different schemas.

Connectors normalize those schemas into Rivora Observations.

For example:

GitHub Pull Request

↓

Observation

Deployment Event

↓

Observation

CI Failure

↓

Observation

The Runtime consumes a single canonical Observation model.

------------------------------------------------------------------------

# Characteristics

## Stateless

Connectors should avoid storing engineering state.

## Isolated

A connector failure should not impact other connectors.

## Replaceable

Connectors can be added, removed, or upgraded independently.

## Observable

Connector activity should itself generate Runtime observations when
appropriate.

------------------------------------------------------------------------

# Relationship to Capabilities

Connectors and Capabilities have distinct responsibilities.

**Connectors provide data. Capabilities produce engineering knowledge.**

The complete architectural relationship is defined in **RFC-028 ---
Connectors and Capabilities**.

Connectors:

-   Observe
-   Normalize
-   Deliver

Capabilities:

-   Remember
-   Evaluate
-   Verify
-   Improve
-   Learn

One brings information into the Runtime.

The other reasons over it.

------------------------------------------------------------------------

# Relationship to the Runtime

The Runtime owns:

-   Memory
-   Knowledge
-   Evaluation
-   Verification
-   Learning
-   Investigations

Connectors own none of these.

They are ingestion adapters.

------------------------------------------------------------------------

# What Connectors Do Not Do

Connectors do not:

-   perform evaluations
-   generate recommendations
-   execute engineering workflows
-   maintain engineering memory
-   bypass Runtime APIs
-   implement business logic

------------------------------------------------------------------------

# Architectural Guarantees

Connectors guarantee:

-   External systems remain independent of the Runtime.
-   Every observation is normalized before entering the Runtime.
-   Engineering reasoning never occurs inside connectors.
-   Connectors are replaceable and independently deployable.
-   The Runtime remains the single source of engineering logic.

If these guarantees change, this RFC must be updated before
implementation.

------------------------------------------------------------------------

# Summary

Connectors are Rivora's observation layer.

They translate diverse engineering ecosystems into a unified stream of
canonical Observations while intentionally remaining free of engineering
reasoning.

By separating observation from understanding, Rivora keeps external
integrations simple, consistent, and replaceable while allowing the
Runtime to build durable engineering understanding from a single
canonical model.

------------------------------------------------------------------------

# Architecture Note

Connectors never contain engineering logic, business reasoning,
evaluation, verification, improvement, or learning behavior. Those
responsibilities belong exclusively to Capabilities.

------------------------------------------------------------------------

# Related RFCs

-   RFC-006 --- Event Model
-   RFC-009 --- Memory
-   RFC-010 --- Verification
-   RFC-011 --- Capabilities
-   RFC-012 --- Connectors
-   RFC-013 --- Learning
-   RFC-028 --- Connectors and Capabilities
