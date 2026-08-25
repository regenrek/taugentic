# Taugentic

![Taugentic banner](public/taugentic_banner.webp)

**Plan, Chat, and Ship Code in One AI Workspace**

## Requirements

- Rust `1.90`
- Node `24`
- `pnpm`
- `just`
- Lefthook
- Gitleaks

## Install

```bash
just setup
```

`just setup` installs the local pre-commit and pre-push hooks. The hooks reject
secrets, credentials, runtime logs, and dumps before Git can publish them.

## Start

Run the desktop app:

```bash
just desktop-dev
```

Run the daemon standalone only if you want to debug it directly:

```bash
just daemon
```

## Operations

Production operator runbooks live in `docs/operations/README.md`.
