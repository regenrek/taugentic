# Daemon Restart Reconciliation

Use this runbook when Mission Control reports `daemon.startup_reconciled` or a run fails with `daemon restarted while run was active`.

## What Happens

On daemon startup, Taugentic reconciles persisted runs before accepting RPCs or dispatching new work. The recovery boundary is exact:

- A run persisted as `running` has an opaque provider execution handle that cannot survive process loss. Startup marks it `failed` and appends a durable `runReconciledOnStartup` event.
- A run persisted as `waitingForApproval` has a durable approval request, not a live provider execution handle. Startup preserves the run and its pending approval; the scheduler rehydrates it as the existing approval owner.

The event log and replay cursor are preserved. Attaching after restart returns the same durable event sequence under the new daemon instance. A cursor issued by the old daemon instance receives `HistoryGap`; clients must resume from the attached latest cursor. The daemon does not delete transcript, artifact, approval, or stream history during reconciliation.

## Operator Checks

1. Open Mission Control and check Recent Errors for `daemon.startup_reconciled` only when a run was `running` at process loss.
2. For a reconciled `running` run, open Run Detail and verify it is terminal `failed`.
3. For a `waitingForApproval` run, verify the run remains waiting and its original pending approval is still present.
4. Reattach clients from the latest cursor. Treat `HistoryGap` for an old daemon-instance cursor as the expected restart boundary.
5. Use Timeline or Logs to inspect events before the restart, then start a new run or fork only if work should continue after a terminalized `running` run.

## Expected Diagnostics

The diagnostics snapshot should include a recent error similar to:

```json
{
  "source": "daemon.startup_reconciled",
  "message": "run-... · daemon restarted while run was active"
}
```

For a terminalized `running` run, the event log should include both:

- `run` with status `failed` and detail `daemon restarted while run was active`
- `runReconciledOnStartup` with reason `daemonRestartedWhileRunning`

For a preserved `waitingForApproval` run, there is no startup terminal event or approval resolution. Its existing approval remains pending and its durable activity sequence does not gain reconciliation events.

## Escalation

Escalate if the same run is reconciled more than once. Startup reconciliation is idempotent, so repeat entries for one run indicate store corruption or a non-canonical writer.
