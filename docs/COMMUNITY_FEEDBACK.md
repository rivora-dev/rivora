# Community Feedback

> How the Rivora community can share what they learn, and how maintainers
> listen before building more.

---

## Where to share feedback

Rivora is in a public v0.1 preview. The fastest way to be heard is a GitHub
issue using one of the structured templates:

| You want to... | Use this template |
|---|---|
| Report something broken | [Bug report](https://github.com/rivora-dev/rivora/issues/new?template=bug_report.yml) |
| Share what worked and what did not | [Feedback](https://github.com/rivora-dev/rivora/issues/new?template=feedback.yml) |
| Request an evidence connector | [Evidence connector request](https://github.com/rivora-dev/rivora/issues/new?template=evidence_connector_request.yml) |
| Get help with Slack setup | [Slack setup help](https://github.com/rivora-dev/rivora/issues/new?template=slack_setup_help.yml) |
| Send a structured design partner report | [Design partner report](https://github.com/rivora-dev/rivora/issues/new?template=design_partner_report.yml) |

If GitHub Discussions are enabled, the categories below are a good home for
longer conversations. If Discussions are not enabled yet, use GitHub Issues and
label the issue with `feedback` or `design-partner`.

---

## Suggested discussion categories

### Show and tell

Share a workflow where Rivora helped. What evidence did you ingest, what did
you remember, and what did recall surface? Screenshots and output are welcome
after redacting tokens.

### Connector requests

Which provider or platform should Rivora read evidence from next? See the
[evidence connector request template](https://github.com/rivora-dev/rivora/issues/new?template=evidence_connector_request.yml)
for the structured version. Use this category for looser conversations about
priorities and trade-offs.

### Slack setup help

Self-hosted Slack can be fiddly. Share what worked, which scopes you configured,
and where you got stuck. Remember to redact tokens.

### Reliability memory examples

Post an example of evidence becoming approved memory. What did your team
choose to remember, and why? What did recall surface the next time a similar
situation happened?

### Design partner feedback

Early design partners can use the
[design partner report template](https://github.com/rivora-dev/rivora/issues/new?template=design_partner_report.yml)
for a structured report, or start a discussion here for ongoing conversation.

---

## How to sanitize logs before sharing

Rivora redacts Slack and GitHub tokens in diagnostic output, but review any
text before pasting it into an issue or discussion:

1. Remove any `xoxb-`, `xapp-`, `ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_`
   prefixed values.
2. Remove signing secrets and private keys.
3. Remove internal hostnames, customer identifiers, and production incident
   timelines that include sensitive data.
4. Replace real repository URLs with `owner/name` placeholders if the repo is
   private.

---

## How to report security issues

Do not open a public issue for security vulnerabilities. Use the repository's
[private vulnerability reporting form](https://github.com/rivora-dev/rivora/security/advisories/new).
See [SECURITY.md](../SECURITY.md) for the full policy.

---

## What Rivora is trying to learn

During the v0.1 preview Rivora is trying to learn:

* Where the demo and CLI flow confuse new users.
* Whether "evidence vs memory" feels clear in practice.
* Whether recall surfaces useful past situations.
* Where Slack setup is friction-heavy.
* Which evidence connector would be most valuable next.
* What would make a team use Rivora weekly.

Feedback is evaluated using the framework in
[FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md).

---

## Related

- [DESIGN_PARTNER_ONBOARDING.md](DESIGN_PARTNER_ONBOARDING.md)
- [FEEDBACK_ANALYSIS.md](FEEDBACK_ANALYSIS.md)
- [18-Roadmap.md](18-Roadmap.md)
- [SECURITY.md](../SECURITY.md)
