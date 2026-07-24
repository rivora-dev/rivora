# Benchmark and Performance Budgets (v0.9)

Budgets assume the **medium** profile on local SSD hardware.
They are regression gates, not marketing SLAs.

| Scenario | Target (ms) | Max (ms) |
|----------|------------:|---------:|
| cli_startup | 300 | 1,000 |
| workspace_startup | 500 | 2,000 |
| store_open | 50 | 250 |
| investigation_list | 100 | 500 |
| investigation_show | 50 | 250 |
| ingestion | 20 | 100 |
| duplicate_ingestion | 10 | 50 |
| routing | 5 | 50 |
| lifecycle_run | 50 | 250 |
| lifecycle_trace | 20 | 100 |
| search | 100 | 1,000 |
| recall | 50 | 500 |
| timeline | 50 | 500 |
| relationship_derivation | 100 | 1,000 |
| pattern_derivation | 100 | 1,000 |
| proposal_generation | 100 | 1,000 |
| persistence_read | 5 | 50 |
| persistence_write | 10 | 50 |
| index_rebuild | 200 | 2,000 |
| diagnostic_export | 100 | 1,000 |

Automated micro-benchmarks live in `crates/rivora/tests/v0_9_production_hardening.rs`.

```bash
rivora doctor budgets --json
cargo test -p rivora --test v0_9_production_hardening micro_benchmarks
```

Any cache must remain derived, rebuildable, and non-authoritative.
