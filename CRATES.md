# Crates

Open Rivora is a Cargo workspace. Foundational crates live under `crates/`.

| Crate | Path | Responsibility | Key public types / APIs |
|---|---|---|---|
| `rivora-errors` | `crates/rivora-errors` | Typed, actionable errors | `RivoraError`, `ErrorKind`, `Result<T>` |
| `rivora-types` | `crates/rivora-types` | Domain-agnostic typed primitives | `TypedId<Tag>`, `IdTag`, `Version`, `NonEmptyString` |
| `rivora-core` | `crates/rivora-core` | Domain vocabulary + logging | `ObservationId`, `AbilityId`, `ReceiptId`, `ServiceId`, `DeploymentId`, `IncidentId`, `ContextId`, `OrganizationId`, `SchemaVersion`, `AbilityVersion`, `ConnectorVersion`, `init_logging()`, `LoggingConfig` |
| `rivora-traits` | `crates/rivora-traits` | Core trait interfaces | `Connector`, `InferenceProvider`, `StorageProvider`, `ReceiptRenderer`, `Clock`, `IdGenerator`, `Logger`, `HealthStatus` |
| `rivora-receipts` | `crates/rivora-receipts` | Typed reliability receipts: schema, validation, builders, renderers | `Receipt`, `ReceiptKind`, `ReceiptStatus`, `Evidence`, `ReasoningStep`, `Confidence`, `Risk`, `SuggestedAction`, `ReceiptRenderer` implementations (`JsonRenderer`, `MarkdownRenderer`) |
| `rivora-graph` | `crates/rivora-graph` | Typed Context Graph knowledge model | `ContextGraph`, `Node`, `Edge`, `NodeKind`, `EdgeKind`, `GraphProvenance`, `GraphConfidence`, `GraphSnapshot`, `GraphMetadata` |
| `rivora-memory` | `crates/rivora-memory` | Adaptive reliability memory model | `MemoryRecord`, `MemoryKind`, `MemoryScope`, `MemoryStatus`, `MemoryProvenance`, `MemoryConfidence`, `MemoryRetention`, `MemoryIndex`, `MemorySnapshot`, `MemoryRecallQuery`, `MemoryRecallResult`, `HumanFeedback`, `FeedbackKind`, `FeedbackTargetType`, `FeedbackSource` |
| `rivora-adaptive` | `crates/rivora-adaptive` | Pure Adaptive Memory Engine | `AdaptiveMemoryEngine`, `MemoryCandidateRequest`, `MemoryCandidateResult`, `RecallQuery`, `RecallResult`, `RecallMatch`, `RecallScore`, `MemoryRecommendation`, `FeedbackApplicationResult` |
| `rivora-slack` | `crates/rivora-slack` | Pure Slack reliability memory surface | `SlackReliabilityMemoryApp`, `SlackMentionRequest`, `SlackMemoryAnswer`, `SlackRecallCard`, `SlackMemoryCandidateCard`, `SlackFeedbackAction`, `SlackActionResponse` |
| `rivora-cli` | `crates/rivora-cli` | Local CLI memory interface, packaged deterministic scenario demo runner, self-hosted live Slack Socket Mode adapter, and `rivora` binary | `LocalMemoryStore`, `StoreSnapshot`, `Command`, `DemoOptions`, `DemoScenario`, `RememberOptions`, `RecallOptions`, `FeedbackOptions`, `FixtureIngestOptions`, `SlackCommand`, `SlackDevOptions`, `SlackDoctorOptions`, `SlackAppMentionEvent`, `SlackPostMessageRequest`, `SlackTokenConfig`, `run`, `run_command` |
| `rivora-connectors` | `crates/rivora-connectors` | Read-only evidence connectors | `EvidenceConnector`, `EvidenceItem`, `EvidenceKind`, `EvidenceSource`, `EvidenceIngestRequest`, `EvidenceIngestResult`, `LocalGitConnector`, `GitHubConnector`, `GitHubClient`, `HttpGitHubClient`, `FixtureGitHubClient`, `GitHubIngestRequest`, `GitHubIngestResult`, `GitHubRepositoryRef`, `GitHubAuthConfig`, `redact_token` |
| `rivora-config` | `crates/rivora-config` | Config loading + validation | `Config`, `SecretRef`, `Secret` |
| `rivora-testing` | `crates/rivora-testing` | Shared testing infra | `fixtures`, `tempfs::TempWorkspace`, `snapshot`, `property`, `mock::FakeClock`, mock trait impls |
| `rivora-examples` | `crates/rivora-examples` | Runnable examples | `config_loading`, `typed_ids`, `logging`, `error_handling` |

## Dependency direction

```
rivora-errors      ← no internal deps
rivora-types       ← rivora-errors
rivora-core        ← rivora-types + rivora-errors
rivora-traits      ← rivora-types + rivora-errors  (trait definitions only)
rivora-receipts    ← rivora-errors + rivora-types + rivora-core + rivora-traits
rivora-graph       ← rivora-errors + rivora-types + rivora-core + rivora-receipts
rivora-memory       ← rivora-errors + rivora-types + rivora-core + rivora-receipts + rivora-graph
rivora-adaptive     ← rivora-errors + rivora-types + rivora-memory + rivora-receipts
rivora-slack        ← rivora-errors + rivora-types + rivora-memory + rivora-receipts + rivora-adaptive
rivora-cli          ← rivora-errors + rivora-memory + rivora-receipts + rivora-adaptive + rivora-connectors + rivora-slack + tungstenite
rivora-connectors   ← rivora-errors + serde + serde_json
rivora-config      ← rivora-core + rivora-types + rivora-errors
rivora-testing     ← all seven above
rivora-examples    ← all eight above
```

Provider crates (connectors, inference providers, storage backends) will depend
**upward** on `rivora-traits`, never the reverse. This enforces the
provider-agnostic dependency direction defined in
[docs/04-Architecture.md](docs/04-Architecture.md).

## Out of scope

The following are **not** present in the current phase and are deferred per
[docs/16-Implementation-Plan.md](docs/16-Implementation-Plan.md):

- Kubernetes, AWS, Datadog, or other cloud connectors
- Official Slack Marketplace app, hosted OAuth, and Slack interactive delivery (interactive buttons now work for basic actions)
- Inference provider implementations
- Storage backend implementations
- Abilities
- Daemon mode, cloud sync, hosted service, dashboards, and autonomous actions
