# Supported Operating Envelope (v0.9)

Rivora v0.9 defines a **supported operating envelope** for local and on-prem use.
These limits are measured and tested. Rivora does **not** claim unlimited scale.

## Hardware assumptions

- Modern laptop or workstation (approx. 8+ GB RAM, SSD)
- Single-node local filesystem (not a distributed store)
- One exclusive writer process at a time across processes; same-process handles share a lock

## Profiles

| Profile | Investigations / store | Observations / Investigation | Memory / Investigation | Search scan budget |
|---------|------------------------|------------------------------|------------------------|--------------------|
| **small** | 50 | 100 | 100 | 5,000 |
| **medium** | 500 | 1,000 | 1,000 | 100,000 |
| **large_supported** | 5,000 | 10,000 | 10,000 | 1,000,000 |

Additional hard limits (all profiles):

| Limit | Value |
|-------|------:|
| Max Observation / payload size | 1 MiB |
| Max Connector HTTP response | 1 MiB |
| Max event batch per observe | 500 |
| Default CLI / Workspace list page | 100 |
| Hard max list / search results | 1,000 |
| Connector connect timeout | 5 s |
| Connector request timeout | 30 s |
| Concurrent writers (cross-process) | 1 |
| Supported prior store versions | 0.1–0.8 (additive open) |

Inspect live values:

```bash
rivora doctor envelope --profile medium --json
rivora doctor budgets --json
```

## What is not supported

- Multi-tenant cloud control planes
- Distributed execution / horizontal scaling
- Concurrent multi-process writers on one store
- Unbounded CLI dumps without pagination
- Automatic remediation or Proposal acceptance
- Secret-bearing remote telemetry

See also: `docs/guides/OPERATIONS.md`, `docs/guides/KNOWN_LIMITATIONS.md`.
