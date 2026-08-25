# Architecture overview

Taugentic uses a Rust daemon as the runtime core. The GPUI desktop and the CLI
are clients of the same daemon state and execution model.

Local runtime is the default. Background service mode requires explicit user
consent.

## Core rules

- The daemon owns sessions, runs, approvals, permissions, persistence,
  execution, model selection, and event history.
- `ta-protocol` owns transport types and JSON-RPC method contracts.
- `ta-store` owns durable record shapes and repositories.
- `ta-policy` owns allow, deny, and require-approval decisions.
- `ta-model-catalog` owns native-harness model metadata and validation.
- `ta-host-platform` owns host facts and platform capability detection.
- `apps/desktop` owns GPUI presentation and temporary UI state.
- GPUIX owns the React-to-GPUI bridge and native interaction behavior.

No desktop module may become a second owner of daemon data or policy.

## Runtime shape

Local clients connect over a Unix socket or a named pipe. The daemon JSON-RPC
contract is the only application transport contract.

The Rust runtime-control owner resolves the socket path and reports it to
clients. Desktop code must not derive socket paths, process ownership, degraded
state, or recovery actions from raw host facts.

The desktop sends runtime-control intents such as start, stop, enable
background mode, disable background mode, and reconcile. The daemon returns one
`DaemonControlStatusResult` snapshot with the allowed actions and the current
state.

Remote clients use the same logical contract over an authenticated WebSocket.
Plaintext `ws://` remains loopback-only. Any non-loopback deployment requires a
separate TLS transport change.

## Crate and app roles

- `ta-orchestrator` owns daemon composition, runtime control, host lifecycle,
  transport handlers, and execution.
- `ta-cli` is a thin client for daemon and runtime-control commands.
- `xtask` owns protocol export and workspace smoke commands.
- `ta-protocol` owns transport-neutral contracts.
- `ta-model-catalog` owns the canonical model catalog.
- `ta-host-platform` owns platform detection.
- `ta-policy` owns permission decisions.
- `ta-store` owns persistence.
- `apps/desktop` is the GPUIX-based macOS app.

## Foundation checks

Run the foundation checks before feature work:

```sh
cargo xtask check-protocol
cargo xtask check-daemon-foundation
cargo test -p ta-orchestrator --test daemon_integration
cargo test -p ta-cli --lib --tests
pnpm --dir apps/desktop check
pnpm --dir apps/desktop test
cargo xtask smoke-local-daemon
```

The desktop test launches the production GPUI host and uses GPUIX automation to
interact with the native window. Daemon tests remain in the Rust owner.

## References

- `docs/architecture/runtime-ownership.md` defines state and lifecycle owners.
- `docs/architecture/desktop-boundaries.md` defines the GPUI desktop boundary.
- `docs/architecture/model-catalog.md` defines model catalog ownership.
- `docs/architecture/acp-runtime.md` defines ACP adapter behavior.
- `docs/testing/strategy.md` defines test placement.
