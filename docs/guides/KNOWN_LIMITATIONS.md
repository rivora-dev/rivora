# Known Limitations (v0.10+)

## Distribution (v0.9.1+)

- No Windows installer or Windows release binaries in this patch
- Shell installer requires `curl` or `wget`, `tar`, and `sha256sum` or `shasum`
- Install directory must be user-writable (no implicit sudo)
- Linux builds target glibc (`unknown-linux-gnu`), not musl
- Cross-platform support claims require published + verified release assets

## Intentional non-goals

- No cloud control plane / multi-tenancy
- No distributed execution
- No autonomous remediation
- No automatic Proposal acceptance or rollback
- No Capability/Connector marketplace or dynamic plugins
- No Web UI
- No daemon architecture

## Operating envelope limits

See `OPERATING_ENVELOPE.md`. Workloads beyond **large_supported** are not validated.

## Remaining product limits

- Search still scans store contents (bounded results, not a full inverted index service)
- Single cross-process writer
- Observation connectors remain single-page for some providers (documented in coverage)
- Workspace is terminal-first (Ratatui Unified Workspace), not a web or IDE host
- Natural-language intent interpretation is deterministic/fixture-first in v0.10 (not a general-purpose chatbot)
- Coding-agent handoff is a typed preview boundary — Rivora does not invoke agents autonomously

## Unresolved risks (accepted for v0.9)

- Hard-link exclusive create may fall back on some filesystems (covered by create_new fallback)
- Process liveness probe uses `kill -0` on Unix
- Very large Investigation timelines can still be slow near envelope upper bounds

## v1.0 blockers

None identified that prevent contract freeze preparation; freeze classifications live in `V1_FREEZE_ASSESSMENT.md`.
