# Security Notes (v0.9)

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
