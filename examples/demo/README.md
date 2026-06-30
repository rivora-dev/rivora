# Rivora Demo Fixtures

This directory contains deterministic, fake evidence for local Rivora demos.
No fixture contains real tokens, customer data, or infrastructure credentials.

The scenario is intentionally small:

- `checkout-api` had latency during inventory synchronization.
- A GitHub PR reduced worker concurrency.
- A workflow failed during validation.
- A local Git commit touched checkout worker configuration.

Canonical packaged fixtures live under
`crates/rivora-cli/fixtures/demo/<name>/evidence.json`. The readable copies in
`examples/demo/scenarios/<name>/evidence.json` are kept byte-for-byte in sync
by tests.

Scenario fixtures include:

- `basic` keeps the default demo short.
- `checkout-incident` shows checkout latency evidence around inventory work.
- `release-regression` shows release and post-release validation evidence.
- `workflow-failure` shows billing migration validation evidence.

Run:

```bash
rivora demo
rivora demo --scenario checkout-incident
rivora demo --scenario release-regression
rivora demo --scenario workflow-failure
```

Or ingest the fixture into an explicit local store:

```bash
rivora init
rivora ingest fixture --path examples/demo/scenarios/basic/evidence.json
```

The root `evidence.json` remains the Phase 13 compatibility fixture.

`rivora demo` does not read these example paths at runtime. The fixture data is
embedded in the CLI binary, so the command works after installation and from
outside the source checkout.

Evidence is not memory until a human chooses to remember and approve it.
No infrastructure actions are taken.
