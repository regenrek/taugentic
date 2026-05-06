# Changelog

All notable user-facing changes to `taugentic` should be recorded here.

When creating a GitHub release, copy the matching release entry into the release body and keep the summary paragraph at the top. Include only the sections that have real content for that release:

- `### Breaking`
- `### Added`
- `### Changed`
- `### Fixed`

Keep bullets short, concrete, and impact-focused. Prefer describing what a user, operator, or contributor can now do, what changed in behavior, and what needs attention when something is breaking.

## Unreleased

### Added

- `ta daemon` now includes lifecycle commands for `start`, `wait`, `restart`, `stop`, `logs`, and watchable `status`, giving operators one canonical CLI surface for local daemon control.
- The desktop app now has a fuller shell with overview, activity, approvals, runs, sessions, artifacts, and shared workspace state instead of only the earlier first-pass panels.
- Desktop packaging and release automation now include Electron builder configuration, staged packaging scripts, and a dedicated GitHub desktop release workflow.
- The docs tree now has canonical sections for architecture, contracts, testing, operations, decisions, and archived material.
- Agent runtime management now includes provider health, profile-backed auth state, and a first Codex provider adapter so daemon clients can inspect and manage runtime connectivity through one typed surface.
- Agent runtime providers now have a provider-neutral adapter layer, including a built-in local provider and a shared provider contract that future runtime families can implement without re-owning daemon core behavior.
- The desktop shell now includes an agent-runtime management panel for selecting runtime profiles, changing policy and model settings, toggling extensions, and driving provider auth from the renderer.
- Desktop now consumes live agent stream events and paged agent turn history so mission control surfaces can render in-progress assistant output, tool-call progress, and pending lane states in one coherent session view.

### Changed

- The daemon and protocol surface are being hard-cut around a more explicit runtime contract, with split wire modules, refreshed generated desktop bindings, and new client-facing crates for daemon access and JSON-RPC transport.
- Electron main and preload transport ownership are being split into bootstrap, session authority, RPC connection, credential, and stream-focused modules instead of the earlier monolithic daemon session path.
- Root documentation is moving from flat top-level files to a structured `docs/` hierarchy with clearer ownership and migration boundaries.
- Runtime policy and daemon RPC handling now fold agent-runtime profile selection and provider auth flows directly into the daemon-owned control surface instead of leaving provider state as an external concern.
- Agent runtime selection is now persisted and treated as required state, so daemon clients reconnect against a stable selected profile instead of falling back to optional or implicit profile selection.
- Provider-owned model catalogs now pin runtime profiles to explicit model ids instead of embedding full model objects inside profile state, keeping provider defaults deterministic and reducing protocol drift as more providers are added.
- Desktop session bootstrap now hydrates the agent-runtime snapshot through the shared query layer instead of leaving runtime management outside the main mission-control shell state.
- Agent-stream transport now separates durable turn-history rows from transient live lane events, allowing the daemon/store to replay stable turn records while the renderer subscribes to bounded live stream fan-out for the focused session.

### Breaking

- Legacy generated contract files and earlier flat documentation paths are being removed in favor of the new protocol export shape and canonical `docs/` locations.

## 0.0.1 - 2026-04-09

Initial Stage 1 release of the Taugentic local workspace: a Rust daemon, a local CLI, a Rust-owned protocol export pipeline, and an Electron desktop shell now land together as one reviewable baseline.

### Added

- A Rust workspace with canonical crates for orchestration, protocol export, host platform detection, policy evaluation, observability, storage, and local automation.
- A local daemon with JSON-RPC transport, domain models for sessions, runs, approvals, artifacts, checkpoints, and runtime lifecycle management.
- A `ta` CLI with daemon status inspection for local tooling, smoke checks, and machine-readable automation output.
- An Electron desktop workspace with main, preload, renderer, generated shared bindings, and first-pass views for daemon status, sessions, runs, approvals, and artifacts.
- Protocol code generation through `xtask`, including exported TypeScript bindings and JSON Schema derived from Rust-owned contracts.
- Cross-platform validation and release hygiene, including CI, `cargo nextest`, desktop tests, protocol freshness checks, dependency hygiene, secret scanning, and daemon smoke coverage.

### Fixed

- Daemon smoke validation now verifies socket readiness and restart behavior, reducing the risk of shipping a desktop shell that cannot reconnect cleanly to the local runtime on supported hosts.
