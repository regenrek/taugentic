# Tracing And Diagnostics

Use this runbook to collect daemon evidence for incidents and bug reports.

## Log Configuration

Default daemon tracing:

```sh
RUST_LOG=info just daemon
```

Structured file output is always enabled for the daemon. Discover the path:

```sh
just ta daemon status --json
```

Read recent logs:

```sh
just ta daemon logs --tail 300
```

Force JSON stderr and a known log directory:

```sh
TAUGENTIC_LOG_FORMAT=json \
TAUGENTIC_LOG_DIR=/tmp/taugentic-daemon \
RUST_LOG=info \
just daemon
```

Disable stderr while keeping the file sink:

```sh
TAUGENTIC_LOG_STDERR=0 just daemon
```

## RUST_LOG Recipes

Capsule dispatch and run execution:

```sh
RUST_LOG=info,ta_orchestrator::orchestration::run_execution=debug,ta_orchestrator::orchestration::app=debug just daemon
```

JSON-RPC transport and handler errors:

```sh
RUST_LOG=info,ta_jsonrpc=debug,ta_orchestrator::host::rpc=debug just daemon
```

OpenAI OAuth and token lifecycle:

```sh
RUST_LOG=info,ta_auth_openai=trace,ta_provider_llm=debug just daemon
```

GitHub work-source polling:

```sh
RUST_LOG=info,ta_orchestrator::orchestration::app::work_item_poller=trace,ta_work_source=debug just daemon
```

Sandbox capability and launch issues:

```sh
RUST_LOG=info,ta_host_platform=debug,ta_sandbox=debug,ta_linux_sandbox=debug,ta_windows_sandbox=debug just daemon
```

## Diagnostics Snapshot

Mission Control reads `daemon.diagnostics.snapshot`. Raw socket check:

```sh
SOCKET="$(just ta daemon status --json | jq -r '.socketPath')"
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"daemon.initialize","params":{"clientName":"ops-runbook","clientVersion":"0","protocolVersion":"2026-04-stage3","capabilities":{"notifications":false,"eventSubscriptions":false}}}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"daemon.diagnostics.snapshot","params":{}}'
} | nc -U "$SOCKET"
```

Fields to capture:

- `uptimeMs`
- `inFlightRpcCount`
- `inFlightCapsuleRunCount`
- `recentErrorCount`
- `recentErrors`
- `tokenUsage`
- `worktreeCount`
- `claimCount`
- `sandbox`
- `providerHealth`

## Panic And Error Surfacing

JSON-RPC handler panics are caught by the runtime and emitted as typed JSON-RPC error data with `kind: "handler_panicked"`. The daemon also emits tracing events. When a panic or failure affects a run, the run error is visible through Run Detail and included in `daemon.diagnostics.snapshot.recentErrors`.

Collection path:

```sh
just ta daemon logs --tail 500
```

Then capture Mission Control or the raw diagnostics snapshot. Include the exact JSON-RPC method, run id, session id, and timestamp.

## Bug Report Artifact Checklist

Attach:

- `git rev-parse HEAD`
- host OS and version
- daemon status JSON:

```sh
just ta daemon status --json
```

- daemon log tail:

```sh
just ta daemon logs --tail 500
```

- diagnostics snapshot from Mission Control or raw JSON-RPC
- run id and session id
- screenshot or copied text from Run Detail **Logs**, **Timeline**, and **Violation** when present
- relevant environment names, not secret values:

```sh
env | rg '^(TAUGENTIC_|RUST_LOG=|OPENAI_|GH_TOKEN=|GITHUB_TOKEN=)' | sed -E 's/=.*/=<redacted>/'
```

Never attach raw OAuth tokens, PATs, callback codes, or credential-store payloads.
