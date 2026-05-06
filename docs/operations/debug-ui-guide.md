# Debug UI Guide

Use this guide to pick the right desktop surface during incident triage. UI state is read from daemon-owned queries and durable run events; do not create parallel notes or local state as the source of truth.

## Mission Control

Mission Control is the operator landing surface for daemon diagnostics.

It surfaces:

- Uptime from `daemon.diagnostics.snapshot`.
- In-flight RPC count.
- In-flight capsule run count.
- Worktree count.
- Claim count.
- Sandbox OS/kind and network capability summary.
- Provider health.
- Recent run errors.
- Quick links to Run Tree, Approval Inbox, and Work Inbox.

Use it first when the daemon is reachable but behavior is unclear.

## Run Tree And Timeline

Use **Run Tree** to inspect durable run structure and current run status.

Open a run and inspect:

- **Result** for terminal output contract results.
- **Timeline** for durable chronological events.
- **Raw** for exact daemon payloads.
- Terminal status fields for `completed`, `failed`, `budgetExceeded`, `cancelled`, and approval-waiting states.

The timeline is backed by daemon run events, not renderer-local reconstruction.

## Capsule Membrane Viewer

Use the **Membrane** tab in Run Detail to inspect capsule result boundaries:

- planned writes
- produced artifacts
- output contract result
- quarantine or receipt context when present

Membrane data is read from durable run detail and replayed run events.

## Logs Tab

Use the **Logs** tab in Run Detail for capsule console/tool activity and agent stream frames associated with a run.

Operator flow:

1. Open Run Tree.
2. Select the run.
3. Open **Logs**.
4. Correlate the latest log lines with **Timeline** and **Recent Errors** in Mission Control.

If logs are missing for a run that should have emitted tool or agent frames, include the run id and daemon log tail in the bug report.

## Approval Inbox

Use **Approval Inbox** when a run is waiting for an operator decision.

Actions:

- review approval request text
- approve or reject
- add decision commentary when needed

CLI fallback:

```sh
just ta approval list --session <session-id>
just ta approval decide --session <session-id> --approval <approval-id> --decision approved
just ta approval decide --session <session-id> --approval <approval-id> --decision rejected --commentary "reason"
```

## Token Panel

Use **Agent Runtime** and related provider health cards to inspect token/auth profile state:

- selected provider
- selected runtime profile
- auth profile connection state
- login/logout action availability
- provider health message

For aggregate token usage, use Mission Control `tokenUsage` from `daemon.diagnostics.snapshot`.

## Work Inbox

Use **Work Inbox** for daemon-owned background work-source items.

Actions:

- **Refresh** queues a daemon-side GitHub poll.
- **Dismiss** marks the item dismissed in the daemon store.
- **Trigger** maps the item into the normal capsule run path.

If the list is empty, confirm `TAUGENTIC_WORK_SOURCE_GITHUB_REPO` and the host-secret GitHub PAT; see `docs/operations/github-work-source.md`.

## Replay Controls

Use **Replay** from Run Detail only on terminal runs with a durable fork point.

Replay:

- uses existing `daemon.run.fork`
- starts from the selected run's durable event sequence
- creates a new run
- does not mutate the original run

Use it when a run failed due to transient provider, approval, or tool behavior and the durable history is sufficient to fork safely. Do not use replay to hide a deterministic contract or sandbox failure; fix the cause or file a PlanDB task.

## Error Surfacing

Run Detail surfaces typed run errors without a second error model:

- output contract violations
- validation errors
- panic/error details propagated through JSON-RPC and run events
- budget and failure status

Triage order:

1. Mission Control **Recent Errors**.
2. Run Detail **Logs**.
3. Run Detail **Timeline**.
4. Run Detail **Violation** or **Raw** when the error is contract-shaped.
5. Daemon log tail:

```sh
just ta daemon logs --tail 300
```
