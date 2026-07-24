# Security Notes (v0.9+)

## Binary installation (v0.9.1+)

- Primary install path: `curl -fsSL https://rivora.dev/install | sh`
- Downloads use HTTPS with TLS verification always enabled
- Selected archive is verified against release `SHA256SUMS` before extract/install
- Installer never invokes `sudo` and never modifies shell profiles
- Prefer inspecting the script before piping to a shell
- GitHub Releases remain the artifact store; `rivora.dev/install` is the stable install contract only

## Guarantees strengthened in v0.9

- Secret redaction in Connector payloads and error strings
- Bounded HTTP response and Observation payload sizes
- No arbitrary shell execution in observation connectors (local/git/kubectl only as fixed commands)
- Execution authority unchanged: plan → policy → approval → confirm → independent Verification
- Diagnostic export is local and sanitized
- Store lock prevents multi-process silent corruption

## Residual risks

- Local filesystem access equals full store access (OS user trust model)
- Tokens in environment variables can appear in process listings
- kubectl/git output must still be treated as untrusted input
- Users can still point Connectors at malicious fixtures or endpoints

## Hard requirements

- Never commit tokens, `.env` with secrets, or production store dumps with credentials
- Prefer fixture modes in CI
- Review `doctor export` before sharing

## Non-goals (still deferred)

- Multi-tenant isolation
- Remote secret management
- Full sandboxing of git/kubectl subprocesses beyond fixed argv
