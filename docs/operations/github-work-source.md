# GitHub Work Source

Use this runbook to operate daemon-owned GitHub issue polling and the Work Inbox. The renderer never polls GitHub directly. Source binding is now owned by `taugentic.workflow/v1`; see `docs/operations/workflow-file.md` for the canonical policy file.

## Configure Repositories

Current implementation supports one GitHub repository per loaded workflow:

```yaml
source:
  kind: github_issues
  repo: owner/name
  code_host_account_id: code-host-account-example
  active_states: ["ready-for-agent", "bug"]
  terminal_states: ["done", "cancelled"]
```

Without a loaded valid workflow, the poller is idle and logs:

```text
background orchestrator idle; no workflow loaded
```

## Connect The Account

`ta-code-host` owns code-host accounts, credential resolution, and GitHub HTTP
policy. `ta-work-source` receives an already resolved GitHub client and token; it
does not define or discover credentials.

Connect a named GitHub account through the desktop **Pull requests > Accounts**
surface. The access token crosses the local typed bridge once, is validated
against GitHub, and is then stored only by the operating-system credential
store. Project and workflow data contain redacted account metadata and an opaque
account ID, never the token. Use that exact ID as `source.code_host_account_id`.

Backend selection:

- macOS: Keychain.
- Linux: Secret Service. Daemon startup fails if Secret Service is unavailable.
- Windows: Credential Manager.

The daemon does not read `GH_TOKEN`, `GITHUB_TOKEN`, or the retired fixed
work-source credential key. It never infers a default account.

## Required PAT Scopes

Use the narrowest token that can read issues on the configured repository:

- Fine-grained PAT: repository access to the configured repo and **Issues: Read-only**.
- Classic PAT for private repos: `repo`.
- Classic PAT for public-only repos: `public_repo`.

No GitHub write scope is needed for v1. Dismiss and trigger mutate only daemon/store state.

## Polling And Rate Limits

The poller calls:

```text
GET /repos/{owner}/{repo}/issues?state=open&per_page=100&page=N
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2026-03-10
If-None-Match: <etag>
```

Behavior:

- Poll cadence and failure backoff use `orchestrator.retry.initial_ms` plus per-daemon jitter.
- Backoff caps at `orchestrator.retry.max_ms`.
- `304 Not Modified` updates the cursor without rewriting items.
- `Retry-After`, primary rate-limit reset, secondary rate limits, `403`, and `429` are honored.
- Network and 5xx failures use exponential backoff with jitter.
- Pull requests returned by the issues API are filtered out.

## Operate Work Items

In the desktop app:

1. Open **Work Inbox**.
2. Use **Refresh** to queue an immediate daemon-side poll.
3. Use **Dismiss** to mark an item dismissed locally.
4. Use **Trigger** to start a capsule run from the selected item.

Triggering a work item uses the normal run start path: session attachment, recipe mapping, claim registry, worktree manager, event stream, approval policy, and budget enforcement are unchanged. The resulting run id is stored back on the work item.

## Troubleshooting

Confirm daemon-side config:

```sh
RUST_LOG=info,ta_orchestrator::orchestration::app::work_item_poller=debug,ta_work_source=debug just daemon
```

Check recent work-source logs:

```sh
just ta daemon logs --tail 300
```

Common messages:

- `workflow not loaded; background orchestrator is idle`: load a valid workflow.
- `GitHub source account is missing`: connect the exact account named by `code_host_account_id` or update the workflow.
- `host secret backend unavailable`: start the required OS credential service.
- `work source poller rate limited`: wait for the logged retry delay.
- `work source poll failed`: inspect the redacted error, repository name, and PAT scope.

Do not add a renderer-side GitHub token or direct GitHub fetch. The daemon/store path is the single owner.
