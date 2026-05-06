# Architecture Overview

Taugentic uses a Rust daemon as the shared runtime core, with thin desktop and
mobile clients around it. On desktop, **local** runtime is the default;
**background** (OS service) is an explicit opt-in.

## Core rules

- Electron renderer owns presentation, navigation, and transient drafts.
- In the renderer, TanStack Query is the SSOT for daemon-derived remote data,
  while `@xstate/store` and XState own local UI state plus stream or command
  lifecycle only.
- Electron main owns windows, preload wiring, runtime-control IPC bridging, and
  stream plumbing. It does **not** own runtime-mode semantics.
- The daemon owns sessions, runs, approvals, checkpoints, canonical execution
  semantics, and heavy background work.
- After **desktop quit**, the daemon the app was managing in **local** mode is
  stopped. A daemon that **keeps running** after quit is **only** when the user
  has **explicitly enabled** the background service; that is not the default.

## Runtime shape

- `ta-daemon` is the sole runtime owner for agent lifecycle, turns, harness
  execution, and other heavy processes.
- Desktop, mobile, CLI, and future menu-bar surfaces are clients of the same
  daemon-owned runtime.
- Local clients connect over unix socket or named pipe using the canonical
  daemon JSON-RPC surface.
- Desktop currently mixes:
  - one-shot local RPC for daemon reachability and control-status probing
  - persistent initialized sessions for attached session reads and live streams
- Local daemon address resolution must be canonical per OS and shared across
  daemon, CLI, and desktop clients.
- Process-local temp settings must not create divergent local daemon identities
  for the same user.
- Remote or mobile clients will connect over authenticated websocket using the
  same logical surface and typed contracts.
- While that transport is still plaintext `ws://`, it must remain loopback-only.
- Supported access patterns are host-local use or explicit forwarding to
  loopback, such as SSH port forwarding or a private tunnel or proxy that
  terminates on localhost.
- Any non-loopback exposure requires a separate TLS or `wss://` transport
  project; packaging must not widen the current auth boundary.
- Desktop IPC maps to the Rust-owned control plane:
  **`startDaemon`**, **`stopDaemon`**, **`enableBackgroundService`**,
  **`disableBackgroundService`**, and **`reconcileDaemon`** all return the same
  `DaemonControlStatusResult` snapshot. Renderer-facing actionability comes from
  `allowedActions`, `desiredMode`, `actualMode`, `transitionStatus`, and
  `reconcileRequired`; the same snapshot also carries host-observable metadata
  such as `socketPath`, `logPath`, optional `daemonVersion`, and
  `protocolVersion` (see `docs/contracts/ipc.md`).

## Current crate roles

- `ta-orchestrator`: canonical daemon implementation owner for runtime-control,
  app services, host lifecycle, transport handlers, typed daemon config
  loading/normalization, and launch/service projection
- `ta-cli`: thin client over the canonical daemon contract, with explicit
  background vs local runtime controls
- `xtask`: protocol export, protocol checks, and daemon smoke entrypoint
- `ta-protocol`: canonical transport-neutral contract crate independent from
  `ta-orchestrator`
- `ta-host-platform`: host OS metadata, distro or version probes, and platform
  capability SSOT
- `ta-policy`: allow, deny, require-approval decisions over the canonical
  approval taxonomy
- `ta-store`: persistence boundary with repositories and durable record shape,
  independent from `ta-orchestrator`
- `apps/desktop`: Electron workspace

## Foundation validation

Use these commands as the canonical daemon app-server foundation gate:

```sh
cargo xtask export-protocol
cargo xtask check-protocol
cargo xtask check-daemon-foundation
cargo test -p ta-orchestrator --test daemon_integration
cargo test -p ta-cli --lib --tests
cd apps/desktop && pnpm test -- --run tests/main/daemon.test.ts
cd apps/desktop && pnpm typecheck
cargo xtask smoke-local-daemon
```

What they prove:

- protocol export is the live contract SSOT
- daemon, CLI, and desktop still follow one canonical ownership path with no
  desktop-local app stubs or client imports from runtime-owned boundaries
- real daemon startup, attach, subscribe, and app-read RPCs work over the
  canonical transport
- CLI remains a thin client over the same daemon contract
- desktop main keeps reconnect and summary-read behavior on the daemon path

Foundation gate rules:

- Do not add new feature work until this gate is green.
- If the gate fails, fix the ownership drift or broken seam before adding more
  app surface.
- Keep same-daemon identity and reattach proofs in
  `crates/ta-orchestrator/tests/daemon_integration.rs`.
- Keep desktop tests limited to Electron-owned seams rather than re-proving
  daemon-owned invariants.

## Canonical references

- Use `docs/architecture/acp-runtime.md` when changing ACP provider
  registration, typed harness dispatch, runtime profile normalization, approval
  projection, or future sandbox/capsule boundaries.
- Use `docs/architecture/runtime-ownership.md` when deciding who owns a piece
  of state or behavior, including the renderer split between Query, XState, and
  bootstrap ownership.
- Use `docs/architecture/desktop-boundaries.md` for `main`, `preload`,
  `renderer`, and `shared` package rules.
- Use `docs/decisions/README.md` for the current daemon-first decision index
  until the referenced decision records are published in-repo.
- Use `docs/contracts/ipc.md` for desktop IPC and daemon transport contracts.
- Use `docs/testing/strategy.md` for the test-layer matrix and ownership rules.
