# Runtime Ownership

Use this document to decide who owns state, lifecycle, policy, and long-running
behavior in the current Taugentic architecture.

## Renderer owns

- layout state
- tabs and panel visibility
- temporary form state
- optimistic UI projections
- feature-local orchestration that exists only to drive presentation

Renderer ownership splits further into three explicit sub-owners:

- app/bootstrap ownership for startup hydration and persisted UI selection restore
- TanStack Query ownership for renderer-side remote snapshots derived from daemon
  reads and daemon-backed live updates
- `@xstate/store` or XState ownership for local UI state and stream or command
  lifecycle that exists only inside the renderer

Renderer code must not create a second durable owner for daemon-derived domain
data. If the daemon owns a list or snapshot, the renderer may cache or project
it, but must not invent a parallel canonical store for the same domain.

### Renderer bootstrap owns

- persisted session selection restore and initial validation against daemon-owned
  session results
- app-level startup hydration that must run before feature leaves assume current
  selection or theme state
- shell-level current session routing state via the workspace shell store

Renderer bootstrap must not be hidden inside feature leaf models or panel hooks.
Persisted selection restore belongs at the app boundary, not inside
`useSessionsPanelModel()` or other panel-local hooks.

### TanStack Query owns

- `sessions`
- `sessionOverview`
- session `runs`
- session `activity`
- session `approvals`
- session `artifacts`
- `agentRuntime`

TanStack Query is the renderer SSOT for daemon-owned remote data. Query keys and
cache updates must be the canonical renderer read model for these domains.

### `@xstate/store` and XState own

- current route and current selected session id
- theme mode
- local drafts and panel-local form state
- pending command state
- stream connection lifecycle
- reconnect, decode-failure, and history-gap orchestration
- local selection that exists only to drive presentation, such as a currently
  focused artifact id inside one panel

`@xstate/store` and XState must not remain long-term owners of daemon-derived
lists or snapshots when those same domains already live in Query.

## Preload owns

- the `contextBridge` surface exposed to the renderer
- narrow request and stream entrypoints over explicit IPC channels
- no domain policy, retry policy, cache ownership, or long-lived business state

## Electron main owns

- desktop application lifecycle
- native window creation
- direct calls into the Rust-owned runtime-control surface
- IPC handler registration and shell-facing window controls exposed to preload
- desktop-side connection lifecycle and bridging stream ports into the renderer

Electron main does **not** own runtime product semantics anymore. It acts as a
thin adapter over the Rust control plane:

- desktop sends intents: `start`, `stop`, `enableBackground`,
  `disableBackground`, `reconcile`
- desktop consumes one Rust-derived control snapshot
- desktop must not derive ownership, degraded state, or recovery policy from
  raw process facts
- desktop quit still stops the daemon only when Rust says the desired mode is
  `local`
- if a dedicated desktop-side runtime-control serializer is introduced later, it
  must be documented here explicitly rather than implied as an ambient rule

## Runtime owns

- sessions
- runs
- steps
- live assistant-turn and tool-progress stream semantics
- approvals
- checkpoints
- execution policy
- agent runtime provider, auth, profile, and extension configuration as one
  daemon-global runtime configuration surface
- runtime lane scheduling, pending-turn lifecycle, and provider-neutral stream
  event normalization
- runtime capability derivation from host facts
- agent lifecycle and turn lifecycle
- typed harness selection and heavy background processes
- ACP runtime profile normalization and provider registration wiring
- compatibility and capability negotiation for connected clients

The runtime is the product core. Desktop, mobile, CLI, and future menu-bar
surfaces are thin clients over the same daemon-owned state and execution model.

### Current renderer remote-domain rule

The duplicate renderer ownership hard cut for these daemon-owned domains has
landed:

- `runs`
- `activity`
- `approvals`
- `artifacts`

The canonical renderer rule is now:

- Query is the only renderer owner of these remote domains
- stream or actor layers must not keep second remote-domain lists or snapshots
- stream layers keep lifecycle, reconnect, retry, history-gap, decode-failure,
  and pending-command orchestration only
- local presentation state may remain actor-owned when it does not duplicate
  daemon truth, for example currently selected artifact id
- live stream messages rehydrate, patch, or invalidate Query-owned data rather
  than becoming a second domain store

Future work such as `t-lr4l` must preserve this split rather than reintroducing
stream-owned mirrors of daemon data.

## Policy boundary

- `ta-protocol` owns the canonical approval scope taxonomy for
  file/process/network
- `ta-policy` evaluates those protocol-owned scopes into
  allow/deny/require-approval decisions

## Store boundary

- `ta-store` owns persistence interfaces, record schemas, and current-shape
  durable integrity
- runtime consumers should talk to `ta-store` repositories, not directly to
  record bags

## Orchestrator daemon host owns

- process shell
- JSON-RPC host adapter
- transport session hosting
- client subscription fan-out
- seq, cursor, backlog, replay, and bounded-subscriber fan-out for live runtime
  lane events
- persisted runtime control plane
- background service control helpers used by CLI or packaged desktop surfaces
- canonical typed daemon config loading, normalization, and launch projection for
  runtime mode, socket/log policy, observability, control token, and remote WS

### Live lane delivery policy

- durable lane frames are the replay boundary
- assistant deltas and tool-progress frames are live-only best-effort transport
- backlog eviction prefers transient frames before durable frames
- per-subscriber queues are bounded; an overflowed subscriber is dropped and the
  JSON-RPC session is closed rather than blocking publish
- reconnect recovery uses the existing backlog or `historyGap` path; there is no
  separate in-band lag marker today

This is aligned with the current Codex app-server narrow-lossless tier. In the
local 2026-04-19 Codex checkout, `codex-rs/app-server/src/in_process.rs`
`server_notification_requires_delivery(...)` requires delivery only for
`TurnCompleted`, so agent-message deltas are no longer treated as lossless
upstream.

The daemon host defines the canonical runtime control contract:

- `background_opt_in` is persisted product consent and only Rust may write it
- `desired_mode` is the persisted target runtime mode
- `pending_transition` is persisted whenever a multi-step runtime mutation is
  in flight or degraded
- `last_error` records the last control-plane divergence
- `generation` is the monotonic mutation epoch
- `actual_mode`, `allowed_actions`, `transition_status`, and
  `reconcile_required` are Rust-derived snapshot fields, not desktop-derived
  heuristics
- `local` means a directly started local daemon process
- `background` means an explicitly enabled OS-managed service
- desktop and CLI may present controls, but only Rust proves lifecycle
  invariants and decides whether destructive control is allowed
- a daemon in `foreign` ownership is inspect-only from the public control
  surface

Public clients consume one stable snapshot:

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

## Adapters own

- vendor protocol details
- process startup and message mapping
- capability detection for their specific lane
- translation from vendor-native stream/progress events into protocol-owned
  runtime lane event shapes

Adapters do not own runtime profile semantics or harness selection. ACP
adapters consume `AcpProviderSpec` and adapter-local launch data, while
`ta-orchestrator` remains the owner that chooses `AgentExecutionHarness`.

## Cross-reference

- Use `docs/architecture/acp-runtime.md` for ACP typed harness dispatch,
  descriptor-owned provider registration, approval projection, and future
  sandbox/capsule boundaries.
- Use `docs/architecture/desktop-boundaries.md` for TypeScript package and
  import rules inside `apps/desktop`.
- Use `docs/contracts/ipc.md` for transport and desktop IPC contracts.
