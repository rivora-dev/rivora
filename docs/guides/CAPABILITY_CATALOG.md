# First-Party Capability Catalog (v0.8)

RFC-028 is **validated across the first-party platform** for all registered
execution Capabilities below. Composite/assistance workflows remain Runtime
orchestration catalogs (RFC-018/019) and are not ExecutionCapability adapters.

## Lifecycle coverage matrix

| Capability ID | Memory | Evaluation | Verification | Improvement | Learning |
| --- | --- | --- | --- | --- | --- |
| `mock.record` | Supported | Supported | Supported | Deferred | Deferred |
| `github.issue.comment` | Supported | Supported | Supported | Deferred | Deferred |
| `github.issue.label` | Supported | Supported | Supported | Deferred | Deferred |
| `github.issue.create` | Supported | Supported | Supported | Deferred | Deferred |
| `github.pull_request.create_draft` | Supported | Supported | Supported | Deferred | Deferred |
| `github_actions.workflow_dispatch` | Supported | Supported | Supported | Deferred | Deferred |

Deferred Improvement/Learning is intentional: measured evidence is required
before Learning; low-quality auto-Proposals are not generated to “fill” stages.

## Catalog entries

### `mock.record`

| | |
| --- | --- |
| **Purpose** | In-process mock mutation for tests and local loop validation |
| **Provider / operation** | `mock` / `record` |
| **Inputs** | `resource_key`, `field`, `value` |
| **Actions** | `record_mutation`, `fail_mutation` |
| **Risk** | LowRiskWrite |
| **Verification** | Independent `observe_state` field comparison |
| **Improvement / Learning** | Deferred |
| **Limitations** | Not a real external system |

### `github.issue.comment`

| | |
| --- | --- |
| **Purpose** | Post a comment on a GitHub issue |
| **Provider / operation** | `github` / `comment` |
| **Inputs** | `issue_number`, `body` |
| **Risk** | LowRiskWrite |
| **Accepted routing types** | `issue`, `pull_request` |
| **Verification** | GET exact comment; compare body |
| **Limitations** | Bound to registered `owner/repo`; live needs `GITHUB_TOKEN` |

### `github.issue.label`

| | |
| --- | --- |
| **Purpose** | Add or remove a label on a GitHub issue |
| **Provider / operation** | `github` / `label` |
| **Inputs** | `issue_number`, `label` |
| **Actions** | `add_label`, `remove_label` |
| **Risk** | LowRiskWrite |
| **Verification** | GET issue labels; exact presence/absence |
| **Limitations** | Bound to registered repo; live needs token |

### `github.issue.create`

| | |
| --- | --- |
| **Purpose** | Create a GitHub issue |
| **Provider / operation** | `github` / `create_issue` |
| **Inputs** | `title` (+ optional `body`) |
| **Risk** | BoundedWrite |
| **Verification** | GET issue by number |
| **Limitations** | Duplicates possible if client keys differ |

### `github.pull_request.create_draft`

| | |
| --- | --- |
| **Purpose** | Create a draft PR from an existing branch |
| **Provider / operation** | `github` / `create_draft_pr` |
| **Inputs** | `title`, `head`, `base` |
| **Risk** | BoundedWrite |
| **Accepted routing types** | `pull_request`, `commit`, `git_status` |
| **Verification** | GET PR; draft must be true |
| **Limitations** | No force-push, merge, or branch delete |

### `github_actions.workflow_dispatch`

| | |
| --- | --- |
| **Purpose** | Trigger an explicitly named GitHub Actions workflow |
| **Provider / operation** | `github_actions` / `dispatch_workflow` |
| **Inputs** | `workflow_id`, `ref` |
| **Risk** | BoundedWrite |
| **Accepted routing types** | `workflow_run`, `workflow_dispatch_request`, `check_result` |
| **Verification** | Correlate named workflow runs (not “latest only”) |
| **Limitations** | Dispatch acceptance ≠ workflow success |

## Connector inventory (first-party)

| Connector | Read-only | Canonical kinds | Fixture |
| --- | --- | --- | --- |
| local | yes | repository, git_status, commit, changed_files, test_output, local_event | yes |
| github | yes | repository, pull_request, commit, check_result, issue | yes |
| github_actions | yes | workflow_run, check_result | yes |
| kubernetes | yes | infrastructure | yes |
| sentry | yes | observability | yes |

## Coverage inspection

```sh
rivora capability coverage --json
rivora capability list
rivora capability show github_actions.workflow_dispatch
```

Workspace: **Capability Engineering Loop (v0.8)** → **Capability coverage / health**.

## Honest blockers / non-coverage

- Infrastructure and observability Observations currently route to **zero**
  execution Capabilities (explicit unsupported routing) — correct until a
  first-party capability accepts those types.
- Improvement/Learning remain Deferred for all first-party execution
  Capabilities until measured outcomes exist.
- Composite capabilities (`investigate_engineering_problem`, etc.) orchestrate
  Runtime services and are not ExecutionCapability registry entries.
