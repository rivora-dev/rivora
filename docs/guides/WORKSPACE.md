# Unified Workspace Guide (v0.10)

The Workspace is Rivora’s primary interactive experience.

It is **not** a chatbot that replaces Investigations, Memory, or Runtime authority.

It is a conversation-first terminal application that routes every meaningful action
through typed `WorkspaceIntent` values into `CapabilityService`.

## Launch

```bash
rivora
# or
rivora-workspace
```

Both use the same launcher. A TTY is required. Non-interactive environments
should use CLI subcommands.

## Core loop

1. Type naturally into **Ask Rivora…**
2. Rivora interprets the text into a typed intent
3. Mutating creates ask for confirmation
4. Capabilities produce durable Engineering Objects
5. The conversation **projects** those objects; it does not own them

## Discover actions

| Input | Behavior |
|-------|----------|
| `/` | Searchable action palette (composer) |
| `Ctrl+P` | Global command palette |
| `?` | Help |
| `Tab` | Move focus |
| `Esc` | Close overlay / return home |
| `Ctrl+C` | Cancel task or quit (terminal restored) |

`/` and `Ctrl+P` share one action registry.

## Ordinary workflows

- **Create Investigation** — describe the problem in plain English and confirm
- **Search / open** — select from results; no opaque ID required in ordinary use
- **Evaluate / Verify / Recommend** — require an active Investigation
- **Proposals** — candidates only; acceptance is not execution
- **Execution review** — plans, approvals, receipts; live runs need exact-revision approval
- **Connectors / Doctor** — status and recovery guidance; secrets never printed

## Authority reminders

```text
Natural language ≠ Runtime authority
Proposal accepted ≠ Execution approved
Execution completed ≠ Execution verified
```

## CLI

Keep using explicit commands for scripts and agents:

```bash
rivora investigation create --title "..."
rivora search "kubernetes"
rivora doctor health
```
