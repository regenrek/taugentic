# Docs

This folder separates current-state source-of-truth documents from contracts,
testing policy, operational runbooks, architecture decisions, migration plans,
and archived material.

## Canonical document types

### `architecture/`

Current-state ownership, layering, and boundary rules.

### `contracts/`

Transport, protocol, config, and edge-facing interface contracts.

### `testing/`

Canonical testing policy, test-layer ownership, and CI expectations.

### `operations/`

Runbooks for packaging, release, local smoke, incident response, and other
procedural workflows.

### `decisions/`

Numbered ADRs for high-cost or irreversible architectural decisions.

### `migrations/`

Time-bound rollout, hard-cut, and migration plans. These are not the steady
state architecture source of truth.

### `archive/`

Superseded material kept only for historical context.

## Current canonical docs

- `architecture/overview.md`
- `architecture/runtime-ownership.md`
- `architecture/desktop-boundaries.md`
- `architecture/lanes.md`
- `decisions/0001-daemon-app-server-foundation.md`
- `decisions/0002-best-possible-2026-daemon-architecture.md`
- `contracts/ipc.md`
- `testing/strategy.md`
- `migrations/2026-04-orchestrator-workspace-hard-cut.md`

## Foundation gate

The canonical daemon foundation gate lives in `architecture/overview.md`.

Use it before adding new daemon app-surface work to confirm:

- protocol export is current
- daemon integration proofs are green
- CLI and desktop still use the canonical daemon path
- local smoke still passes

## Change rules

- Keep one rule in one canonical document.
- Put current-state rules in `architecture/`, `contracts/`, `testing/`, or
  `operations/`.
- Put time-bound rollout and hard-cut plans in `migrations/`.
- Put major architectural choices in numbered ADRs under `decisions/`.
- Move stale docs to `archive/` instead of leaving them beside current-state
  docs.
- Update root `README.md` and nearby references when canonical doc paths move.
