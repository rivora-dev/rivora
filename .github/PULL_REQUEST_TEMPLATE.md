# Pull request

## Summary

<!-- What does this change do, and why? One or two sentences. -->

## Safety boundary checklist

- [ ] This change does not execute infrastructure actions.
- [ ] This change does not persist tokens or secrets.
- [ ] This change keeps evidence separate from approved memory.
- [ ] This change preserves human approval for memory promotion.

## Does this change touch connectors?

<!-- If yes, confirm the connector is read-only and uses no mutating API calls. -->

- [ ] No, this change does not touch connectors.
- [ ] Yes, and the connector is read-only (GET requests only / no mutating commands).

## Can this change mutate infrastructure?

<!-- Rivora must not deploy, roll back, remediate, or mutate infrastructure. -->

- [ ] No, this change has no infrastructure mutation path.

## Tokens and secrets

<!-- Tokens are read from the environment and never stored in .rivora/. -->

- [ ] No tokens or secrets are stored, printed, or written into evidence/receipts.

## `.rivora/` files

<!-- `.rivora/` is local operational data and is gitignored. -->

- [ ] This change does not commit `.rivora/` files.

## Tests run

<!-- List the commands you ran, e.g. cargo fmt --check, cargo test, cargo clippy -- -D warnings -->

## Docs updated

<!-- Which docs were updated? Architecture must never drift from implementation. -->

## Screenshots / output

<!-- Optional. Redact all tokens before pasting. -->
