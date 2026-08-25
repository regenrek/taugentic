# Runtime ownership

Use this reference to place state, policy, and lifecycle code.

## Desktop owns presentation

The GPUI desktop owns:

- navigation, panel visibility, and focus
- temporary form state and drafts
- selected rows and other presentation-only state
- projection of daemon snapshots and events into views
- direct transport connection lifecycle
- native window options passed to GPUIX

Desktop state may cache a daemon snapshot for rendering. It must not become a
second durable or canonical store.

The desktop consumes runtime-control results. It does not derive runtime mode,
ownership, recovery policy, socket paths, or allowed actions.

## GPUIX owns native rendering

GPUIX owns the React reconciler, the native element tree, the GPUI bridge,
window rendering, text input, focus, scrolling, events, and native automation.

Taugentic does not fork these mechanics into application code. GPUIX does not
own Taugentic sessions, commands, permissions, or persistence.

## Protocol owns transport contracts

`ta-protocol` owns JSON-RPC methods, wire types, approval scope names, runtime
profile identifiers, and generated TypeScript types.

`apps/desktop/packages/shared` contains only generated protocol output. Do not
add handwritten policy, defaults, platform path logic, or duplicate validators
to that package.

## Daemon owns runtime behavior

`ta-orchestrator` owns:

- sessions, runs, steps, and conversation branches
- assistant and tool event normalization
- approvals and execution policy orchestration
- provider profiles, authentication state, and extensions
- harness selection and child-agent execution
- scheduling, cancellation, replay, and restart recovery
- daemon configuration and runtime-control state
- transport sessions, subscriptions, cursors, and backlog policy

The daemon is the first fix owner when a correct client intent produces wrong
runtime behavior.

## Domain crates own durable rules

- `ta-store` owns record shapes, repositories, and durable integrity.
- `ta-policy` evaluates protocol-owned approval scopes.
- `ta-model-catalog` owns native-harness model metadata, validation, and default
  selection.
- `ta-host-platform` owns host facts, OS detection, and capability probes.

The orchestrator composes these crates. It must not copy their rules into host
or UI code.

## Adapters own vendor translation

Provider adapters own vendor protocol details, vendor process startup, and
translation into protocol-owned runtime events. They do not own runtime profile
semantics, harness selection, approval policy, or model catalog policy.

## Runtime control has one owner

Rust owns runtime-control persistence and derivation:

- `backgroundOptIn` stores user consent.
- `desiredMode` stores the requested mode.
- `pendingTransition` stores an incomplete mutation.
- `generation` identifies the mutation epoch.
- `actualMode`, `allowedActions`, `transitionStatus`, and `reconcileRequired`
  are derived results.

Desktop and CLI present these fields and send allowed intents. Neither client
reimplements the state machine.

## First-fix rule

Place a fix at the first owner that produced the wrong value:

- Fix layout, focus, or temporary selection in `apps/desktop`.
- Fix native rendering or input mechanics in GPUIX.
- Fix a wire mismatch in `ta-protocol`.
- Fix persistence in `ta-store`.
- Fix permission decisions in `ta-policy`.
- Fix model metadata in `ta-model-catalog`.
- Fix runtime orchestration in `ta-orchestrator`.
- Fix vendor translation in the matching provider adapter.

Delete competing implementations when you move a rule to its canonical owner.
