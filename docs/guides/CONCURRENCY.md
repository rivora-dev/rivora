# Concurrency Contract (v0.9)

## Model

| Scenario | Behavior |
|----------|----------|
| Two CLI processes, same store | Second open fails with **lock conflict** (exit 12) |
| CLI + Workspace, same store | Second open fails with lock conflict |
| Same process, multiple handles | Allowed (refcount on lock) |
| Reads during writes (single process) | Supported on filesystem JSON layout |
| Concurrent multi-process writers | **Rejected** (explicit error, no silent corruption) |
| Concurrent migration / index rebuild | Hold exclusive store; do not multi-process rebuild |

## Stale locks

- Lock file: `{data_dir}/.rivora.lock` with `pid` + `created_at`
- Recover when holder is dead or age ≥ 300s: `rivora doctor recover-lock`
- Never reclaim a lock owned by a live foreign process

## Design choice

Smallest model that preserves correctness for local MVP: exclusive cross-process store access. Not a daemon. Not a distributed lock service.
