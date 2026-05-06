# Daemon Restart Reconciliation

Use this runbook when Mission Control reports `daemon.startup_reconciled` or a run fails with `daemon restarted while run was active`.

## What Happens

On daemon startup, Taugentic reconciles persisted active runs before accepting RPCs or dispatching new work. Any run persisted as `running` or `waitingForApproval` belongs to a previous process and cannot have a live execution handle in the fresh daemon, so startup marks it `failed` and appends a durable `runReconciledOnStartup` event.

The event log and replay cursor are preserved. The daemon does not delete transcript, artifact, approval, or stream history during reconciliation.

## Operator Checks

1. Open Mission Control and check Recent Errors for `daemon.startup_reconciled`.
2. Open the affected Run Detail and verify the run is terminal `failed`.
3. Use the Timeline or Logs tabs to inspect events before the restart.
4. Start a new run or fork from a known replay point if the work should continue.

## Expected Diagnostics

The diagnostics snapshot should include a recent error similar to:

```json
{
  "source": "daemon.startup_reconciled",
  "message": "run-... · daemon restarted while run was active"
}
```

The event log should include both:

- `run` with status `failed` and detail `daemon restarted while run was active`
- `runReconciledOnStartup` with reason `daemonRestartedWhileRunning`

## Escalation

Escalate if the same run is reconciled more than once. Startup reconciliation is idempotent, so repeat entries for one run indicate store corruption or a non-canonical writer.
