# Run The Daemon

Use this runbook when operating the local Taugentic daemon directly. The daemon is the runtime owner for sessions, runs, approvals, work-source polling, diagnostics, and provider auth state.

## Start Locally

Builds are lazy through the repo `justfile`.

```sh
just daemon
```

Useful environment overrides:

```sh
TAUGENTIC_DAEMON_SOCKET_NAME=ta-daemon-ops just daemon
TAUGENTIC_LOG_FORMAT=json RUST_LOG=info,ta_orchestrator=debug just daemon
TAUGENTIC_LOG_DIR=/tmp/taugentic-daemon TAUGENTIC_LOG_STDERR=0 just daemon
```

Default local IPC:

- macOS/Linux: a Unix domain socket named `ta-daemon.sock` under the per-user runtime directory. On macOS, long socket paths are shortened under `/tmp/taugentic/s/`.
- Windows: named pipe `\\.\pipe\ta-daemon`.
- Override the endpoint with `TAUGENTIC_DAEMON_SOCKET_NAME` or `just ta --socket <name> ...`.

Optional remote websocket transport is loopback-only while plaintext `ws://` is used:

```sh
TAUGENTIC_DAEMON_REMOTE_WS_ENABLED=1 \
TAUGENTIC_DAEMON_REMOTE_WS_BIND=127.0.0.1:42321 \
TAUGENTIC_DAEMON_REMOTE_WS_AUTH_TOKEN="$(openssl rand -hex 24)" \
just daemon
```

The websocket path is `/rpc`.

## Logs

The daemon always configures a structured file sink. Discover the active log path:

```sh
just ta daemon status --json
```

Tail logs through the CLI:

```sh
just ta daemon logs --tail 200
```

Default file name is `ta-daemon.log.jsonl`. Set `TAUGENTIC_LOG_DIR` to choose the directory. File output is JSON even when stderr is pretty.

Use `RUST_LOG` for filtering:

```sh
RUST_LOG=info,ta_orchestrator=debug,ta_jsonrpc=debug just daemon
RUST_LOG=info,ta_orchestrator::orchestration::app::work_item_poller=trace just daemon
RUST_LOG=info,ta_auth_openai=trace,ta_provider_llm=debug just daemon
```

## Attach Desktop To A Running Daemon

The desktop app connects to the daemon endpoint selected by `TAUGENTIC_DAEMON_SOCKET_NAME`.

```sh
TAUGENTIC_DAEMON_SOCKET_NAME=ta-daemon-ops just daemon
```

Then launch the desktop process with the same endpoint name:

```sh
TAUGENTIC_DAEMON_SOCKET_NAME=ta-daemon-ops just desktop-dev
```

In the app, use the daemon top bar to confirm `actualMode`, transition status, socket, log path, and daemon version. If the app reports a degraded daemon, use **Reconcile** from the daemon controls or:

```sh
just ta daemon background reconcile
just ta daemon status --json
```

## Graceful Shutdown

For a foreground daemon, press `Ctrl+C`. For a CLI-managed daemon, request shutdown:

```sh
just ta daemon stop
```

Shutdown cancels the work-source poller, wakes the local accept loop, and lets active runtime tasks observe cancellation. If a capsule is stuck after shutdown starts, inspect Mission Control and the Run Detail replay controls before deleting worktrees manually.

## Health Check

Basic readiness:

```sh
just ta daemon status
just ta daemon status --json
```

There is no `just ta diagnostics` command. Call `daemon.diagnostics.snapshot` directly over the JSON-lines socket after discovering the socket path:

```sh
SOCKET="$(just ta daemon status --json | jq -r '.socketPath')"
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"daemon.initialize","params":{"clientName":"ops-runbook","clientVersion":"0","protocolVersion":"2026-04-stage3","capabilities":{"notifications":false,"eventSubscriptions":false}}}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"daemon.diagnostics.snapshot","params":{}}'
} | nc -U "$SOCKET"
```

On systems without `nc -U`, use `socat`:

```sh
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"daemon.initialize","params":{"clientName":"ops-runbook","clientVersion":"0","protocolVersion":"2026-04-stage3","capabilities":{"notifications":false,"eventSubscriptions":false}}}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"daemon.diagnostics.snapshot","params":{}}'
} | socat - "UNIX-CONNECT:$SOCKET"
```

The diagnostics RPC requires a prior `daemon.initialize` on the same connection. The CLI health path remains `just ta daemon status`; raw JSON-RPC is for operators who need the full diagnostics payload.

## Common Failure Modes

Socket already in use:

```sh
just ta daemon status --json
just ta daemon logs --tail 200
TAUGENTIC_DAEMON_SOCKET_NAME=ta-daemon-recovery just daemon
```

If the socket belongs to a live daemon, attach to it or stop it with `just ta daemon stop`. If the socket is stale and no daemon responds, remove only the stale socket file after confirming no process owns it.

Panic recovery:

```sh
RUST_LOG=info,ta_jsonrpc=debug,ta_orchestrator=debug just ta daemon logs --tail 300
```

JSON-RPC handler panics are caught and surfaced as typed JSON-RPC errors, tracing events, and recent diagnostic errors when they affect runs. Include the log tail and `daemon.diagnostics.snapshot.recentErrors` in bug reports.

Stuck capsule run:

1. Open Mission Control.
2. Inspect in-flight capsule count and recent errors.
3. Open the Run Tree, select the run, and inspect **Logs**, **Timeline**, and **Raw**.
4. Use **Replay** only for terminal runs with a known fork point; it forks from durable run events instead of mutating the original run.
