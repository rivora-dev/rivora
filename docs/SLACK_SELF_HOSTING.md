# Slack Self-Hosting

> Self-hosted Slack is for open-source users, design partners, and local demos.
> It is not the official Slack Marketplace app.

Phase 12 added the local adapter boundary. Phase 14 adds live Socket Mode
transport in `rivora-cli`, connecting app mentions to the same local
`.rivora/` memory and evidence files used by the CLI. Phase 15 hardens
startup diagnostics, reconnect resilience, and envelope acknowledgement.

## What This Is

- A self-hosted path for teams running their own Slack app.
- A bridge from Slack app mentions to local Rivora memory.
- A live Socket Mode listener for app mentions and plain-text replies.
- A dev mode for non-network demos and tests:

```bash
rivora demo
rivora slack dev --text "what changed?"
```

## What This Is Not

- Not the official Slack Marketplace app.
- Not Rivora Cloud.
- Not a hosted OAuth install flow.
- Not multi-tenant token storage.
- Not Slack message-history ingestion.
- Not autonomous remediation or infrastructure control.

## Local Storage

The adapter reads and writes the same local store as the CLI:

```text
.rivora/memories.json
.rivora/feedback.json
.rivora/receipts.json
.rivora/evidence.json
```

Use `RIVORA_STORE_DIR` to point the adapter at a different local store
directory:

```bash
export RIVORA_STORE_DIR=.rivora
```

If the store is missing, Slack dev mode returns setup guidance instead of
panicking.

## Slack App Manifest

Create a Slack app from:

```text
examples/slack-app-manifest.yaml
```

The manifest enables:

- app mentions,
- posting messages,
- Socket Mode.

The bot scopes are intentionally minimal:

- `app_mentions:read`
- `chat:write`

No channel or group history scope is requested in this phase.

After creating the app from the manifest:

1. Create an app-level token with the `connections:write` scope.
2. Install the app to the workspace and copy its bot token.
3. Confirm Socket Mode and Event Subscriptions are enabled.
4. Keep the `app_mention` bot event subscription enabled.

Interactive components are disabled because Phase 14 handles mentions only.

The live adapter uses `curl` for the two Slack Web API calls and a Rust TLS
WebSocket client for Socket Mode. Ensure `curl` is available on the host.

## Environment

Set tokens through environment variables:

```bash
# Fill these in locally from your self-hosted Slack app.
export SLACK_BOT_TOKEN=
export SLACK_APP_TOKEN=
export SLACK_SIGNING_SECRET=
export RIVORA_STORE_DIR=.rivora
```

Tokens are never stored in `.rivora/`, never printed, and never written to
evidence, memories, feedback, receipts, docs examples, or snapshots. Errors
redact Slack token-like values.

## Quickstart

Fast local demo:

```bash
rivora demo
```

`rivora demo` uses fixture data, does not require Slack tokens, and exercises
the same memory/evidence path that Slack dev mode uses.

Prepare local memory and evidence:

```bash
rivora init
rivora ingest git --repo . --limit 20
rivora ingest github --repo owner/name --limit 20
```

Run the non-network dev path:

```bash
rivora slack dev --text "what changed?"
rivora slack dev --text "have we seen checkout latency before?"
rivora slack dev --text "what merged recently?"
rivora slack dev --text "what failed recently?"
```

The dev path uses the same local routing and rendering boundary as the
self-hosted adapter without connecting to Slack.

## Setup Validation

Run `rivora slack doctor` before starting Socket Mode to validate your
configuration:

```bash
rivora slack doctor
```

The doctor command checks:

- **Environment variables** -- `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`,
  `SLACK_SIGNING_SECRET`, and `RIVORA_STORE_DIR` are present and non-empty.
- **Local store files** -- `.rivora/memories.json`, `.rivora/feedback.json`,
  `.rivora/receipts.json`, and `.rivora/evidence.json` exist and are readable.
- **Token redaction** -- tokens are never displayed in doctor output; values
  are replaced with redacted placeholders.

By default the check is local and offline. Pass `--live` to perform a live
Socket Mode handshake test against the Slack API:

```bash
rivora slack doctor --live
```

The live check validates that the app-level token can open a Socket Mode
connection before you start the full listener.

## Socket Mode

```bash
rivora slack socket
```

The command validates the environment, calls `apps.connections.open` with the
app-level token, establishes the returned TLS WebSocket connection,
acknowledges every envelope, and reconnects when Slack requests a refresh.

Improved startup diagnostics provide a calm startup summary showing the
resolved store path, token redaction status, and Socket Mode connection state
before any envelopes are processed. Reconnect resilience uses bounded
backoff so the process survives transient Slack outages without exhausting
retry budgets. Every incoming envelope receives an acknowledgement before
processing begins, and duplicate envelopes (re-delivered by Slack) are
protected against with an idempotency guard.

For an `app_mention`, Rivora loads the configured local store, routes the text
through the same deterministic path as `slack dev`, and posts a plain-text
thread reply with `chat.postMessage`. The bot token is used only for message
delivery. The WebSocket URL and tokens are never printed or persisted.

The process runs in the foreground until interrupted. It does not start a
daemon, crawl the workspace, ingest channel history, or perform background
memory changes.

## Supported Mention Text

The adapter normalizes `<@bot>` and `@rivora` mentions, then routes messages
through local memory/evidence:

```text
@rivora have we seen checkout latency before?
@rivora recall checkout
@rivora what changed?
@rivora what merged recently?
@rivora what failed recently?
@rivora what should we remember?
```

Rivora does not infer root cause from evidence. If no evidence exists, it
suggests:

```bash
rivora ingest git --repo . --limit 20
rivora ingest github --repo owner/name --limit 20
rivora demo
```

## Feedback Actions

The adapter maps Slack action kinds to the existing memory feedback model:

- Remember / approve -> `Approved`
- Reject -> `Rejected`
- Correct -> `Corrected`
- Not useful -> `NotUseful`
- Needs more evidence -> `NeedsMoreEvidence`

Interactive button handling for Remember, Reject, and Correct actions is now
implemented in the adapter, so these basic feedback flows work over the live
Socket Mode transport. Modal-backed correction and richer interaction flows
are deferred.

The existing correction guidance remains:

```bash
rivora feedback <memory-id> correct --note "..."
```

## Safety Boundary

- Slack cannot mutate infrastructure.
- The live adapter only responds to explicit app mentions.
- Live transport responds only to `app_mention` events.
- Other Socket Mode envelopes are acknowledged and ignored.
- Evidence ingestion remains read-only.
- Memory updates remain available through explicit CLI or local adapter feedback;
  live Slack feedback delivery is deferred.
- No remediation, rollback, deployment, scale, restart, dashboard, daemon,
  cloud sync, or long-running agent loop exists in this phase.

## Future Work

- Socket Mode interactive button delivery for memory feedback now works for
  basic actions (Remember, Reject, Correct); modals are deferred.
- Verified HTTP request handling for teams that prefer public endpoints.
- Rich Slack Block Kit rendering for the current text responses.
- Modal-backed correction flow.
- Future managed/official Slack Marketplace app, separate from this
  self-hosted path.

## Troubleshooting

### Missing environment variables

`rivora slack socket` exits early if `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`,
or `SLACK_SIGNING_SECRET` is not set. Run `rivora slack doctor` to identify
which variables are missing.

### Socket Mode connection refused

- Confirm the app-level token has the `connections:write` scope.
- Confirm Socket Mode is enabled in the Slack app settings.
- Ensure no other process holds an active Socket Mode connection for the same
  app-level token (Slack allows only one connection per token).

### Bot token rejected when posting replies

- Confirm the bot token starts with `xoxb-`.
- Confirm the bot has `chat:write` scope and has been installed to the
  workspace.

### Local store not found

Set `RIVORA_STORE_DIR` to the correct path or run `rivora init` to create
the default `.rivora/` store before starting the adapter.

### Duplicate or repeated responses

Slack may re-deliver envelopes during transient outages. Phase 15 duplicate
envelope protection deduplicates by envelope ID. If you still see duplicates,
confirm you are running the latest `rivora-cli` build.

Related: [SLACK_APP.md](SLACK_APP.md) ·
[CLI_MEMORY_INTERFACE.md](CLI_MEMORY_INTERFACE.md) ·
[DEMO.md](DEMO.md) ·
[EVIDENCE_CONNECTORS.md](EVIDENCE_CONNECTORS.md) ·
[18-Roadmap.md](18-Roadmap.md)
