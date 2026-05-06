# Testing Strategy

Taugentic uses a layered test strategy.

Rule: test each invariant once at the lowest layer that can prove it.

Rule: integration tests exist only for boundary crates and real cross-process
seams.

Rule: workspace smoke stays small. It proves the primary daemon and CLI path
boots and answers, not every edge case.

## Test layers

- unit: pure domain logic, capability derivation, policy decisions, formatting,
  redaction, validation
- roundtrip: serde and wire-shape guarantees for canonical protocol types
- boundary integration: sockets, listener lifecycle, JSON-RPC request or
  response behavior, daemon composition
- CLI contract: command surface, exit codes, stderr or stdout shape, fake-daemon
  compatibility
- workspace smoke: build the real daemon and CLI, boot them together, verify a
  few canonical flows

## Crate matrix

### `crates/ta-protocol`

- keep export and wire roundtrip tests
- own protocol export and adapter wire-shape guarantees
- do not add daemon or CLI integration tests here
- do not own orchestrator domain validation anymore
- do not duplicate serde assertions in higher layers unless the higher layer
  adds its own behavior

### `crates/ta-policy`

- keep unit tests only
- cover allow, deny, and require-approval decisions over policy-owned scopes
- no integration tests unless policy starts depending on external state or
  persistence

### `crates/ta-host-platform`

- keep unit tests only
- cover version parsing, capability derivation, and path fallback logic
- use targeted platform-specific tests when `cfg` branches matter
- no daemon or CLI integration coverage here

### `crates/ta-observability`

- keep unit tests only
- cover observability-specific env parsing, redaction, span fields, and log
  format decisions
- do not treat `ta-observability` as the owner of effective daemon config
  precedence; it is an input parser/helper for the orchestrator-owned loader
- if initialization ordering becomes critical, cover that through daemon startup
  tests instead of crate-local process tests

### `crates/ta-store`

- keep crate-local tests while storage is in-memory or simple
- when durable persistence lands, keep one focused integration suite for
  current-shape SQLite integrity, corruption handling, recovery, and repository
  behavior
- avoid retesting runtime policy or transport behavior here

### `crates/ta-orchestrator`

- keep unit tests for service composition, capability derivation, scheduler
  defaults, policy wiring, and local JSON-RPC transport behavior
- own socket naming, listener bind or rebind behavior, overload handling,
  request parsing, and daemon handler mapping
- own typed daemon-config precedence and normalization:
  socket source, runtime mode, token normalization, remote websocket config,
  effective observability projection, and launch-environment projection
- keep a small daemon integration suite for composition-root behavior that
  cannot be proven lower down
- canonical daemon integration cases:
- real daemon boot responds to `daemon.status`
- CLI-like and desktop-like clients can attach to the same daemon identity, and
  desktop-style reattach does not drift to a second daemon owner
- second client can initialize and attach without interfering with the first
- reconnect or stale-cursor subscribe paths surface the correct `historyGap`
  contract instead of replaying transient deltas as recovery truth
- canonical app-read RPCs such as `daemon.session.list`, `daemon.session.get`,
  `daemon.run.list`, and `daemon.run.get` answer over the real daemon transport
- invalid method returns the expected JSON-RPC error contract
- startup fails cleanly on bind conflict or missing runtime preconditions when
  applicable

### `crates/ta-cli`

- keep command-surface and output contract tests
- use fake-daemon or test-server coverage for machine-readable JSON, stderr,
  exit codes, and request shape
- this is the canonical layer for command UX contracts
- own the proof that CLI consumes orchestrator-owned config results instead of
  re-implementing binary/log-path policy locally
- together with `crates/ta-orchestrator`, own runtime-control lifecycle
  invariants such as ownership checks, control-token enforcement, background
  enable or disable behavior, and stop authority
- prove user-visible lifecycle/logs behavior without re-owning daemon binary or
  offline log-path resolution already proven in `crates/ta-orchestrator`
- do not push transport edge cases up into CLI tests unless user-visible
  behavior depends on them

### `apps/desktop`

- keep desktop tests focused on Electron-owned behavior only:
  `contextBridge` adapters, desktop pre-RPC locator/launch-spec ownership,
  daemon bootstrap or RPC routing, quit orchestration, and renderer state
  derivation from the canonical Rust snapshot
- keep desktop pre-RPC policy tests on `desktop-locator-config` and
  `desktop-bootstrap-config`; do not rebuild that policy in
  `daemon-bootstrap` or other wrapper layers
- do not mirror Rust lifecycle invariants here once `ta-cli` or
  `ta-orchestrator` already proves them
- canonical desktop runtime-control cases are:
  - renderer disables destructive actions for foreign daemons
  - Electron main routes `start` through the canonical bootstrap owner in
    `desktop-bootstrap-config`, while `stop`, `enableBackground`,
    `disableBackground`, and `reconcile` remain canonical RPC-only
  - Electron main summary reads such as `listSessions` and `listRuns` stay on
    the daemon-backed session path, not Electron-owned stub state
  - persistent daemon sessions recover run-stream subscriptions after transport
    loss without losing daemon-owned replay ordering
  - quit re-reads live control state before deciding whether local-mode shutdown
    applies

## Workspace smoke

Use one small workspace smoke layer for the real daemon and real CLI.

Canonical smoke cases:

- daemon boots and `ta daemon status --json` succeeds
- daemon can be restarted cleanly on a fresh socket name
- protocol export is current before client and desktop checks run
- one failure-path smoke only if the failure is product-critical and cannot be
  trusted to lower layers

Do not use workspace smoke as proof for typed config precedence, offline
log-path derivation, or daemon-binary resolution. Those belong in
`crates/ta-orchestrator` and `crates/ta-cli`.

Current canonical command:

```sh
cargo xtask smoke-local-daemon
```

Do not turn workspace smoke into a second integration suite for protocol,
transport, and CLI details. Those belong in their owning crates.

## CI minimum

Fast default Rust signal:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Cross-target compile gate:

```sh
cargo xtask check-platforms
```

Host smoke gate:

```sh
cargo xtask check-daemon-foundation
cargo xtask smoke-local-daemon
```

Foundation freeze gate:

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

Targeted package smoke on CI hosts:

```sh
cargo nextest run -p ta-host-platform -p ta-orchestrator -p ta-cli
```

## Non-goals

- no blanket integration test directory for every crate
- no duplicate wire assertions in protocol, transport, daemon, CLI, and
  workspace smoke
- no broad end-to-end tests for every daemon RPC once transport and handler
  layers already cover them
- no snapshot or golden-output sprawl unless the output is a stable user
  contract

## Change rule

When adding a feature or fixing a bug:

- add the regression at the owning layer
- only add a higher-layer test if the bug escaped because the product seam
  itself was untested
- if a new test duplicates an existing lower-layer guarantee, delete or avoid
  the duplicate instead of keeping both
