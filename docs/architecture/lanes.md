# Lanes

Stage 1 ships one canonical daemon core and leaves lane-specific execution
behind explicit adapters.

## Planned lanes

- native lane: first-party execution path
- ACP lane: descriptor-owned subprocess providers selected by typed harness
  metadata, not by runtime-profile string patterns

## Rule

Lanes may influence capability shape and event mapping, but they do not become
the source of truth for runtime semantics.

`runtime_profile_id` is runtime profile identity only. Lane and harness
selection come from `StrategyRegistry` and `AgentExecutionHarness`; lane
adapters consume that decision and must not create fallback dispatch paths.

## Cross-reference

- Use `docs/architecture/acp-runtime.md` for ACP provider registration, typed
  harness dispatch, and sandbox/capsule boundaries.
- Use `docs/architecture/runtime-ownership.md` for who owns capability and
  runtime behavior.
- Use `docs/contracts/ipc.md` when lane-specific transport details affect the
  client contract.
