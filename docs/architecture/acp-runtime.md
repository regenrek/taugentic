# ACP runtime architecture

This document is the canonical reference for ACP runtime ownership after the
typed harness dispatch hard cut. Its job is to prevent future work from
reintroducing runtime-profile string dispatch, provider/harness confusion,
legacy ACP flavor paths, or UI-owned runtime truth.

## Owner map

- `runtime_profile_id` is an identity only and must not be parsed as the
  source of dispatch, flavor, provider policy, model choice, auth state, or
  sandbox semantics.
- `AgentExecutionHarness` is the typed owner of which harness owns a run. It is
  distinct from `ProviderId`, selected model, auth profile, and policy mode.
- `ta-orchestrator` owns runtime selection. It derives the harness from
  `StrategyRegistry`, `StrategyKind`, and normalized runtime profile metadata,
  validates the explicit `AgentRuntimeSelection`, and records the typed decision
  in `RunExecutionRoute` before dispatch.
- `taugentic-agent` consumes `ExecutionRequest.execution_harness` with an
  exhaustive match. It must not parse `runtime_profile_id`, provider ids, or
  ACP-flavored strings to choose a strategy.
- `ta-protocol` owns transport-neutral wire shapes; `ta-store` owns durable run,
  approval, artifact, and agent-turn projections.
- `ta-provider-acp` owns ACP provider descriptors, ACP subprocess launch
  details, raw ACP JSON-RPC mapping, and provider-controlled error-data
  sanitization.
- `apps/desktop` presents daemon snapshots. It does not own runtime profile
  semantics, approval truth, or harness selection.

## Harness ownership boundary

`AgentExecutionHarness` has two ownership kinds:

- Native `NativeLoop`: Taugentic owns the daemon-backed turn loop, tools,
  approvals, sandbox policy, memory, telemetry, resume semantics, and future
  native subagent/workflow capabilities.
- External integration lanes `Acp` and `CodexAppServer`: Taugentic owns
  lifecycle, stream mapping, approval/permission bridging, MCP forwarding, and
  the process boundary. The external harness owns its internal turn loop, tools,
  subagents, and sandbox.

Harness selection must not be inferred from auth. Native + OpenAI API key and
future Native + OpenAI OAuth/subscription are both native harness executions;
OAuth or subscription auth must not imply Codex app-server routing.

## Runtime profile and harness selection

Runtime profiles describe user-selectable policy and provider identity. They do
not contain a model or an account. `AgentRuntimeSelection` carries the explicit
runtime profile, model, and optional auth profile for a run. The daemon derives
the harness and freezes all four choices in `RunExecutionRoute`.

The canonical request flow is:

1. `agent_runtime::providers` registers runtime strategies.
2. `StrategyRegistry` stores each strategy with a `StrategyKind`.
3. `AgentRuntimeService` validates the selected model and auth profile against
   the registered provider and durable auth-profile state.
4. `RunExecutionRuntime::build_execution_request` asks `StrategyRegistry` for
   the `AgentExecutionHarness` for the normalized profile.
5. `taugentic-agent::execution_strategy::dispatch` runs the selected harness.

Native/API-key style runs remain canonical in the native harness. OpenAI,
Anthropic, and declarative OpenAI-compatible providers resolve their clients in
`native_loop`; ACP provider adapters must not leak into `turn_loop`.

## ACP provider registration

ACP provider registration and execution are descriptor/spec owned:

- `AcpProviderDescriptor` describes stable provider identity, display metadata,
  launch kind, binary resolution, setup commands, and UI support metadata.
- `AcpProviderSpec` is the stable shareable handle carried by orchestrator
  strategy metadata and by `AgentExecutionHarness::Acp`.
- `AcpProviderRegistry` combines builtin descriptors with future
  user-configured descriptors and rejects duplicate provider ids.

There is no `BuiltinAcpFlavor` contract and no runtime-profile string fallback
lane. Adapter-local launch details may use provider-specific arguments or mode
mapping, but dispatch identity comes from the descriptor/spec path.

## Capability Ownership

Capability discovery belongs to the daemon runtime surface, not to renderer
state or provider dispatch. Provider adapters may probe provider-specific facts,
but `ta-orchestrator` decides how those facts are cached and projected through
runtime snapshots.

If an `AcpCapabilityCache` is present, it must be daemon scoped and invalidated
by provider-auth changes, model changes such as `session/set_model`, and
descriptor updates. Until that cache exists, snapshots should read through the
descriptor/registry path rather than growing a second cache in the UI or ACP
adapter.

## ACP Turn And Error Contract

ACP subprocesses speak raw JSON-RPC; Taugentic exposes daemon-owned stream and
run state. The ACP adapter layer is responsible for mapping provider events into
the canonical runtime stream contract before they reach the store or UI.

The current ACP stream contract is:

- emit one `AssistantTurnStarted` frame for each ACP prompt stream before raw
  provider update events are projected
- map assistant deltas and tool-call frames in provider order
- emit one `AssistantTurnCompleted` frame after a successful ACP prompt stream
- signal successful ACP turn closeout through `sink.complete(...)`
- on cancellation or provider error, fail or cancel the run through the
  daemon-owned run lifecycle

Provider JSON-RPC errors should preserve useful diagnostics: JSON-RPC code,
message, and bounded `data` details such as `reason`, `message`, `error`, or
`detail`. Provider-controlled data must be sanitized before surfacing:
sensitive keys and secret-like values are redacted, strings and summaries are
bounded, and large arrays or objects are truncated.

## Approval And UI Boundary

Approval truth is daemon/store owned. Provider strategies request approvals
through runtime approval handles; resolving an approval goes back to the live
daemon-owned run handle.

The desktop reads approval snapshots from the daemon and sends approval
decisions back through the daemon contract. Desktop code must not mirror
approvals into a second durable store or infer approval state from component
lifecycle.

## Future Sandbox And Capsules Boundary

Sandboxing is a later leaf consumer under `AgentExecutionHarness`. It may add
execution policy inputs to an existing harness, but it must not become a
parallel spawn or dispatch path.

Capsules and the context firewall must also build on the typed harness
foundation. Future child contexts should return daemon/store-owned receipts,
artifacts, and promotion decisions; they must not bypass `ExecutionRequest` or
create a second provider spawn path.

## Verification

For docs-only changes, `git diff --check` is sufficient.

Use these focused commands when changing the ACP runtime architecture:

```sh
cargo test -p ta-provider-acp --lib
cargo test -p ta-orchestrator acp
cargo test -p ta-orchestrator agent_runtime
cargo test -p ta-orchestrator provider_run_request
cargo test -p taugentic-agent --lib --tests
cd apps/desktop && pnpm test
git diff --check
```

What they prove:

- typed harness dispatch is derived by the daemon and consumed by
  `taugentic-agent`
- runtime profile normalization has one owner and preserves the implicit-clear
  versus explicit-reject split
- ACP descriptors/specs are the registration and launch path
- ACP provider errors keep useful diagnostics without leaking provider-controlled
  secrets
- Query-owned approval projections remain visible in Mission Control Steps

Live ACP smoke tests additionally prove the installed provider CLI and selected
model can complete a real turn. A live failure caused by a missing provider CLI,
auth state, or unsupported model version is not by itself an architecture
failure; routing, stream lifecycle, approval, and redaction invariants still
need to be checked separately.
