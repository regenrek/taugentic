# Workflow File

`taugentic.workflow/v1` is the daemon-owned policy file for background orchestration. It replaces env-driven work-source defaults: without a loaded workflow, the background orchestrator stays idle and reports `workflow not loaded; background orchestrator is idle`.

## Location

Default path:

```text
~/.taugentic/workflow.yaml
```

The daemon loads this file on boot when it exists. Operators can also load or change the active file through JSON-RPC:

```text
workflow.load      { "path": "/absolute/path/to/workflow.yaml" }
workflow.status    {}
workflow.reload    {}
workflow.validate  { "path": "/absolute/path/to/workflow.yaml" }
workflow.validate  { "contents": "kind: taugentic.workflow/v1\n..." }
```

## Schema

```yaml
kind: taugentic.workflow/v1
name: default-github-implementation
source:
  kind: github_issues
  repo: regenrek/taugentic
  active_states: ["ready"]
  terminal_states: ["done", "cancelled"]
orchestrator:
  max_concurrent_missions: 8
  max_capsules_per_mission: 6
  retry:
    initial_ms: 10000
    max_ms: 300000
policy:
  approvals:
    file_write: ask
    process: ask_for_sensitive
    network: allowlist
  network_allowlist:
    - github.com
    - api.github.com
runtime_profiles:
  implementer:
    provider: codex
    model: gpt-5.6-sol
outputs:
  required:
    - tests
    - patch_or_blocker
budgets:
  per_capsule:
    max_tokens: 100000
  per_orchestrator:
    max_wall_time_ms: 3600000
  per_workflow:
    max_cost_usd: 25.0
```

Provider IDs and model IDs must match the daemon runtime catalog (`codex`, `openai`, `anthropic`, or a declarative provider ID). `github_issues` currently binds `source.repo` to `owner/name`; `active_states` is used as the issue label filter for the existing GitHub source adapter.

## Reload Semantics

`workflow.reload` reparses and validates the configured file path.

- Valid reload: atomically swaps the active workflow and reports `reloaded`.
- Invalid reload: keeps the last known good workflow active and reports all validation errors collected after parsing.
- Missing workflow: leaves the orchestrator idle; work items can be listed/dismissed, but triggering background work is rejected.

## Troubleshooting

Use `workflow.validate` before reload to check edits without changing the active policy. In the desktop app, Mission Control shows the loaded workflow name, source kind, capsule profile count, and last reload outcome. A failed reload badge means the daemon is still using the previous valid workflow.
