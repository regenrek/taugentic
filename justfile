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
  @cargo build -p ta-orchestrator --bin ta-daemon
  @daemon_binary="$(cargo xtask print-daemon-binary)"; \
  TAUGENTIC_DAEMON_SOCKET_NAME="${TAUGENTIC_DAEMON_SOCKET_NAME:-ta-daemon-gpui}" \
  TAUGENTIC_DAEMON_BINARY="$daemon_binary" \
  pnpm --dir apps/desktop dev

@desktop-native-check:
  @cargo test -p ta-desktop-native
  @cargo build -p ta-desktop-native
  @pnpm --dir apps/desktop --filter @taugentic/desktop-daemon-native test

@desktop-check:
  @cargo build -p ta-desktop-native
  @pnpm --dir apps/desktop --filter @taugentic/desktop-daemon-native stage-native
  @pnpm --dir apps/desktop check

@smoke:
  @cargo run -p xtask -- smoke-local-daemon

@measure-m1:
  @cargo build -p ta-orchestrator --release
  @pnpm --dir apps/desktop run build
  @TAUGENTIC_M1_DAEMON_PROFILE=release pnpm --dir apps/desktop run measure:m1

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
