# Desktop boundaries

`apps/desktop` contains one macOS desktop app. React describes the interface,
GPUIX renders it through GPUI, and the app connects directly to the Taugentic
daemon.

## Ownership

### Taugentic desktop owns

- window composition and navigation
- temporary form, focus, and panel state
- presentation of daemon snapshots and events
- the direct JSON-RPC client lifecycle
- starting the canonical Taugentic runtime-control command in development and
  from the packaged app

The desktop does not own sessions, runs, approvals, permissions, persistence,
harness selection, model metadata, replay order, or daemon recovery policy.

### GPUIX owns

- the React reconciler
- the retained native element tree
- GPUI window creation, rendering, focus, text input, scrolling, and events
- the JavaScript-to-Rust bridge
- native desktop test automation

GPUIX must not contain Taugentic product state or daemon policy.

### `packages/shared` owns

`packages/shared` is generated output from `ta-protocol`. It exposes transport
types to TypeScript and contains no handwritten policy, validation, defaults, or
runtime code.

Run these commands after a protocol change:

```sh
cargo xtask export-protocol
cargo xtask check-protocol
```

## Dependency direction

The dependency direction is fixed:

```text
Taugentic React app -> GPUIX -> GPUI
Taugentic React app -> generated ta-protocol types
Taugentic React app -> daemon JSON-RPC transport
```

Do not add a preload layer, desktop IPC bridge, browser renderer, or second
desktop state owner. Put product behavior in the Taugentic app, native rendering
behavior in GPUIX, and runtime behavior in the daemon.

## Runtime identity

`TAUGENTIC_DAEMON_SOCKET_NAME` selects the local daemon identity. The
`desktop-dev` recipe defaults to `ta-daemon-gpui`. Override the variable only
when you intentionally connect to another daemon.

The desktop must consume the socket path reported by the Rust runtime-control
owner. TypeScript must not reimplement platform socket-path rules.

## Dependency policy

Taugentic pins both `@gpuix/react` and `@gpuix/native` to exact versions. Keep
the direct native dependency even though React also depends on it. The direct
pin prevents the bridge and the native binary from moving independently.

Upgrade both packages in one change. Run the type check, the native live-window
test, and the manual milestone test before accepting the upgrade.

pnpm delays other new package releases for seven days and rejects downgraded
registry trust signals. The GPUIX packages and their direct event parser are
explicit exceptions because Taugentic pins and reviews each GPUIX release.
