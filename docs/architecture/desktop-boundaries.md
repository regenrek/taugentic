# Desktop Boundaries

This document is the canonical source of truth for the `apps/desktop/packages`
layer split.

The split is not about arbitrary package count. It is about keeping four
different trust and responsibility boundaries clean:

- `main`: privileged Electron shell and native orchestration
- `preload`: capability membrane between renderer and main
- `renderer`: UI and feature presentation
- `shared`: transport-facing contracts and validation SSOT

## Package ownership

### `packages/main`

Owns:

- `BrowserWindow` creation
- Electron app lifecycle
- daemon bootstrap or reuse
- `ipcMain` handlers
- native stream plumbing with `MessagePort`
- packaging-aware process ownership decisions

Must not own:

- renderer presentation state
- React components
- preload bridge surface design beyond the IPC it serves
- duplicated contract types that already live in `shared`

### `packages/preload`

Owns:

- `contextBridge.exposeInMainWorld(...)`
- narrow `ipcRenderer.invoke(...)` and stream-open wrappers
- no more power than the renderer actually needs

Must not own:

- UI state
- retry or cache policy
- daemon orchestration logic
- independent business validation or policy

### `packages/renderer`

Owns:

- routes, panels, view state, and feature-local orchestration
- calls into the desktop bridge through renderer-owned adapters
- mapping daemon results into presentation state

Must not own:

- direct `electron` imports
- `ipcRenderer` or `contextBridge`
- Node.js APIs
- daemon lifecycle policy beyond invoking desktop intents

### `packages/shared`

Owns:

- desktop IPC channel names
- renderer-visible desktop TypeScript contracts
- generated protocol bindings consumed by desktop TypeScript
- validation surfaces shared across desktop packages

Must not own:

- Electron runtime APIs
- Node.js runtime APIs
- React or renderer-specific UI logic
- main-process orchestration logic

## Dependency direction

Keep the dependency graph one-way:

- `main` -> `shared`
- `preload` -> `shared`
- `renderer` -> `shared`
- `renderer` -> renderer-local `lib/ipc/*` facades

Do not import in the opposite direction:

- `renderer` must not import `main`, `preload`, `electron`, or `node:*`
- `preload` must not import `renderer`
- `main` must not import `renderer` or `preload`
- `shared` must not import `electron`, `node:*`, React, or other desktop
  runtime packages

## Canonical renderer bridge entrypoints

Renderer code must use these canonical facades:

- `apps/desktop/packages/renderer/src/lib/ipc/api.ts`
- `apps/desktop/packages/renderer/src/lib/ipc/stream.ts`
- `apps/desktop/packages/renderer/src/features/window/state.ts`

Direct `window.desktopApi` access is only allowed inside the canonical desktop
IPC boundary modules. Direct `window.desktopWindow` access is only allowed
inside the renderer-local window boundary modules that own window chrome and
window-state bridging.

## Enforcement

These rules are enforced in code, not just prose:

- ESLint enforces package import boundaries and bans direct `window.desktopApi`
  access outside the canonical renderer bridge modules
- Oxlint provides fast baseline correctness and suspicious-code checks
- Knip keeps config and boundary-owner entrypoints visible so drift does not hide
  behind unused private modules

## Change rule

If a new desktop capability needs documentation:

- update this document when package ownership or import direction changes
- update `docs/contracts/ipc.md` when the actual IPC surface changes
- update `docs/architecture/runtime-ownership.md` when runtime ownership shifts
