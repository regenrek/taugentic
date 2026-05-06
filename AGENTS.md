# Taugentic Agent Notes

## Build Rules

- Production-grade only. No MVP shortcuts.
- One canonical implementation in primary path.
- Delete dead/legacy/duplicate paths as part of the change.
- No wrappers/shims/compat layers unless explicitly justified in diff.
- Single source of truth for policy, validation, enums, flags, config.
- Platform code: `cfg` only at module/manifest boundary; distro/OS version/capabilities via runtime probe SSOT.
- Cross-OS/distro work: use `crates/ta-host-platform` as SSOT.
- Validate inputs up front. Fail fast.
- Keep files under ~500 LOC; hard cap 750.
- Domain-folder structure only. Canonical imports only.
- For bugs, add regression test when it fits.
- use ssot to avoid drift
- ACP runtime changes must follow `docs/architecture/acp-runtime.md`.

## File-Size Cap

Production-grade file-size cap is 750 LOC per file (~500 ideal).

Exempted: machine-generated files. Currently:
- `apps/desktop/packages/shared/generated/**/*`
- Any `generated/` subtree under crate output paths.

Rationale: generated files are not human-edited, splitting them adds no review value. The cap targets hand-written source.

## Safety

- Assume parallel edits. Keep diff scoped. Stop only on direct conflict/breakage.
- No delete/move/overwrite without explicit user request.
- Prefer `trash` over destructive delete.
- Don’t leak secrets.
- Validate untrusted input. Preserve auth/tenant boundaries.
- Be cautious with new deps; flag supply-chain/CVE risk.

## Git / CI

- Conventional Commits only: `feat|fix|refactor|build|ci|chore|docs|style|perf|test`.
- Ask before `git push`.
- Use `gh` for GitHub ops and CI.
- Prefer deterministic, non-interactive commands with bounded output.

## Search / Verify

- Use web search early if unsure. Prefer latest stable docs/sources 2026.
- Quote exact errors.
- Prefer end-to-end verification; if blocked, state what is missing.

## Dev Commands

- Prefer the repo `justfile` for local developer command entry points.
- Prefer `just ta ...`, `just daemon`, `just desktop-dev`, and `just smoke` over shell aliases or adding convenience wrappers to `package.json`.
