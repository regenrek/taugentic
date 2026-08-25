set shell := ["zsh", "-cu"]
set quiet

@setup:
  @command -v lefthook >/dev/null || { echo "error: install lefthook before running just setup" >&2; exit 1; }
  @command -v gitleaks >/dev/null || { echo "error: install gitleaks before running just setup" >&2; exit 1; }
  @lefthook install
  @cargo build
  @pnpm install --dir apps/desktop

@build-cli:
  @cargo build -p ta-cli -p ta-orchestrator

@ta *args:
  @target_root="${CARGO_TARGET_DIR:-./target}"; \
  binary="$target_root/debug/ta-cli"; \
  if [[ ! -x "$binary" ]]; then \
    cargo build -p ta-cli; \
  fi; \
  "$binary" {{args}}

@daemon:
  @target_root="${CARGO_TARGET_DIR:-./target}"; \
  binary="$target_root/debug/ta-daemon"; \
  if [[ ! -x "$binary" ]]; then \
    cargo build -p ta-orchestrator; \
  fi; \
  "$binary"

@desktop-dev:
  @pnpm --dir apps/desktop dev

@desktop-dev-agent:
  @TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT=8315 \
  TAUGENTIC_ELECTRON_INSPECT_PORT=9229 \
  TAUGENTIC_ELECTRON_ENABLE_LOGGING=1 \
  TAUGENTIC_ELECTRON_LOG_FILE="${TMPDIR:-/tmp}/my-electron.log" \
  pnpm --dir apps/desktop dev

@smoke:
  @cargo run -p xtask -- smoke-local-daemon

@security-check:
  @node ./scripts/security-check.mjs

@sync-model-catalog:
  @node ./scripts/sync-model-catalog.mjs

@release-check:
  @cargo build -p ta-cli -p ta-orchestrator
  @cargo test -p ta-cli --lib
  @cargo test -p ta-orchestrator --lib
  @cargo xtask check-daemon-foundation
  @cargo xtask smoke-local-daemon
  @pnpm --dir apps/desktop check
  @pnpm --dir apps/desktop test
  @node ./scripts/security-check.mjs

@daemon-cleanup:
  @node ./scripts/daemon-cleanup.mjs

@daemon-cleanup-apply:
  @node ./scripts/daemon-cleanup.mjs --apply
