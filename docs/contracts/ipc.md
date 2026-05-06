# IPC

## Desktop boundary

- Renderer to preload: typed API only via `contextBridge`
- Renderer accesses domain desktop IPC only through the canonical renderer
  adapter modules:
  - `apps/desktop/packages/renderer/src/lib/ipc/api.ts`
  - `apps/desktop/packages/renderer/src/lib/ipc/stream.ts`
- Window chrome and window-state access use a separate shell-facing preload
  surface, `window.desktopWindow`, through renderer-local window boundary
  modules such as:
  - `apps/desktop/packages/renderer/src/features/window/state.ts`
  - `apps/desktop/packages/renderer/src/features/window/chrome.tsx`
- Preload to main: explicit IPC channels only
- Main to renderer streams: `MessagePort`
- `@taugentic/desktop-shared` owns the desktop IPC channel names, window
  channel names, and transport-facing TypeScript contracts

## Desktop runtime control

- **Rust owns runtime-control semantics.** Desktop main and renderer are intent
  senders and snapshot consumers only.
- **Local is the default; background is explicit.** The persisted target lives
  in Rust control-plane state, not Electron memory.
- **`startDaemon` / `stopDaemon` / `enableBackgroundService` /
  `disableBackgroundService` / `reconcileDaemon`** all call Rust-owned daemon
  surfaces and return the same typed control snapshot.
  `startDaemon`, `enableBackgroundService`, `disableBackgroundService`,
  `stopDaemon`, and `reconcileDaemon` currently use the bootstrap subprocess
  lane rather than canonical daemon RPC, with `desktop-locator-config` owning
  locator selection and `desktop-bootstrap-config` owning launch-spec and
  child-env shape, while `daemon-bootstrap` only runs the child process and
  parses the control JSON. Only `getDaemonStatus` is the one-shot daemon RPC
  status path.
- **Desktop quit stops the daemon only when `desiredMode` is `local`.** In
  `background`, long-lived process ownership belongs to the OS service manager.
- **Renderer actionability comes from `allowedActions`.** Renderer and Electron
  main must not derive their own second state machine from raw process or
  service observations.

The canonical desktop runtime-control snapshot is
`DaemonControlStatusResult`:

- `backgroundOptIn`
- `desiredMode`
- `actualMode`
- `transitionStatus`
- `reconcileRequired`
- `allowedActions`
- `errorCode`
- `message`
- optional `pendingTransition`
- `socketPath`
- `logPath`
- optional `daemonVersion`
- `protocolVersion`

Snapshot semantics:

- `desiredMode` is the persisted target mode
- `actualMode` is Rust-observed runtime reality: `stopped`, `local`,
  `background`, or `foreign`
- `transitionStatus` is the stable public transition summary:
  `idle`, `applying`, `degradedReconcileRequired`, or `failedNoStateChange`
- `reconcileRequired` means the system is intentionally exposing divergence and
  requires a Rust-owned repair pass
- `pendingTransition` is public only as a compact view; detailed internal step
  machinery remains Rust-internal

## Native backend boundary

- Main to daemon: local JSON-RPC over unix socket or named pipe
- The desktop currently uses two local transport shapes:
  - one-shot RPC for reachability and control-status probing (`getDaemonStatus`)
  - persistent initialized sessions for attached session reads and live streams
- The persistent-session rule applies to session-bound request and stream
  traffic, not to every local daemon call
- Local daemon locator rules are `ta-orchestrator`-owned Taugentic policy over
  platform primitives and must resolve to the same per-user daemon across CLI,
  desktop, and daemon processes
- Desktop main consumes that locator policy through its dedicated pre-RPC
  locator and bootstrap owners, not by re-owning it inside process code
- On macOS, process-local `TMPDIR` may affect OS temp paths but must not
  participate in canonical daemon identity or locator policy
- Clients initialize once, negotiate capabilities and compatibility, then issue
  requests and receive notifications on the same session
- Commands, queries, and subscriptions share one typed runtime surface
- `daemon.agent.runtime.*` is an initialized, daemon-global runtime
  configuration surface; it is not attached-session-scoped
- `daemon.subscribe` is hydration-first: it returns `ready` for live tailing or
  `historyGap` when the client must rehydrate daemon-owned reads before
  trusting live updates
- Long-running output and runtime state changes stream as daemon notifications
  rather than desktop-owned placeholder channels
- Remote or mobile transport will use authenticated websocket with the same
  logical surface and typed contracts
- Until a dedicated TLS or `wss://` project exists, authenticated remote
  websocket remains loopback-only plaintext `ws://`
- Approved access patterns for that transport are host-local access or explicit
  forwarding to loopback, such as SSH port forwarding or a private tunnel or
  proxy that terminates on localhost
- Direct non-loopback or public plaintext websocket exposure is not allowed

Current documented exception:

- `getDaemonStatus` is intentionally the one-shot daemon RPC status path today
- If control-status reads later move onto a persistent control session, update
  this document and remove the one-shot exception rather than silently widening
  the persistent-session claim

## Remote websocket config

- `TAUGENTIC_DAEMON_REMOTE_WS_ENABLED`: set to `1` or `true` to enable the
  authenticated remote websocket listener; default is disabled
- `TAUGENTIC_DAEMON_REMOTE_WS_BIND`: optional bind address; defaults to
  `127.0.0.1:42321`; must remain loopback-only while the transport is plaintext
  `ws://`
- `TAUGENTIC_DAEMON_REMOTE_WS_AUTH_TOKEN`: required when the listener is
  enabled; bearer token is trimmed before storage and must contain at least 16
  printable non-whitespace ASCII characters
- Remote websocket path is fixed at `/rpc`
- Clients must send `Authorization: Bearer <token>`

Other typed daemon config inputs now follow the same owner direction in
`ta-orchestrator`:

- socket name/source resolution
- runtime-mode precedence
- control-token normalization
- observability log-dir / stderr / format inputs

## Ownership rule

Do not leak daemon-internal transition journals or host internals into
Electron. The daemon speaks `ta-protocol` wire types, and desktop consumes the
stable shared snapshot exported from `@taugentic/desktop-shared`.

The desktop may ensure a compatible daemon for the active mode. In **local**
mode it owns stopping that managed daemon on quit. In **background** mode,
ongoing process control is delegated to the platform service when the user has
opted in; do not assume the daemon survives quit unless background is enabled.

## Cross-reference

- Use `docs/architecture/desktop-boundaries.md` for package-level ownership and
  import direction.
- Use `docs/architecture/runtime-ownership.md` if a transport change affects
  long-lived runtime ownership.
