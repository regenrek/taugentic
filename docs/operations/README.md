# Operations Docs

Procedural runbooks for operators. Keep canonical architecture and contract policy in `docs/architecture/`, `docs/contracts/`, or crate-local docs; link to those sources instead of duplicating them here.

## Runbooks

- `run-daemon.md`: start, attach to, stop, health-check, and recover the local daemon.
- `openai-chatgpt-login.md`: operate native OpenAI ChatGPT subscription login/logout and credential storage.
- `workflow-file.md`: load, validate, reload, and troubleshoot `taugentic.workflow/v1`.
- `github-work-source.md`: operate GitHub issue polling and the Work Inbox through the loaded workflow.
- `restart-reconciliation.md`: inspect daemon startup reconciliation for active runs left by a previous process.
- `token-telemetry.md`: verify real provider token-usage telemetry in Run Detail and Mission Control.
- `sandbox-per-os.md`: interpret per-OS sandbox capability and fail-closed behavior.
- `debug-ui-guide.md`: choose the right Mission Control, Run Detail, approval, token, work, and replay surfaces.
- `tracing-and-diagnostics.md`: collect logs, diagnostics snapshots, panic evidence, and bug-report artifacts.
- `desktop-release.md`: package and release the desktop app.

## Minimum Host Requirements

| OS | Required baseline |
| --- | --- |
| macOS | Rust `1.90`, Node `24`, `pnpm`, `just`, Keychain, launchd user services, Unix sockets, `/usr/bin/sandbox-exec` for Seatbelt sandboxing. |
| Linux | Rust `1.90`, Node `24`, `pnpm`, `just`, Unix sockets, systemd user services for background mode, Secret Service for durable secrets, Landlock-capable kernel; kernel >= 6.7 for TCP network policy enforcement. |
| Windows | Rust `1.90`, Node `24`, `pnpm`, `just`, Credential Manager, Windows service control, named pipes, AppContainer, Job Object, and Windows Filtering Platform for network allowlists. |

## Rules

- Keep these documents operational and step-oriented.
- Do not restate canonical ownership or contract rules here; link back to
  `docs/architecture/` or `docs/contracts/` when needed.
- If a runbook becomes timeless architecture policy, move that rule into the
  canonical architecture or contract owner.
