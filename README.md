# Taugentic

![Taugentic banner](public/taugentic_banner.webp)

**Plan, Chat, and Ship Code in One AI Workspace**

## Requirements

- Rust `1.90`
- Node `24`
- `pnpm`
- `just`

## Install

```bash
cargo build
pnpm install --dir apps/desktop
```

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
