# Model Catalog Ownership

Taugentic uses one catalog for native-harness model metadata and selection.
`ta-model-catalog` owns that catalog. Provider adapters, workflows, the daemon,
and clients do not maintain model lists or native model defaults.

## Sources and lifecycle

- The release snapshot in `crates/ta-model-catalog/generated/catalog.json`
  provides a validated catalog at process start.
- The daemon fetches `https://models.dev/api.json` at startup and every four
  hours. It replaces the active snapshot only after the complete response
  validates.
- `scripts/sync-model-catalog.mjs` regenerates the release snapshot. Run it
  intentionally when preparing a release; CI does not depend on the network.
- Provider authentication determines whether a configured provider is usable.

The active snapshot is memory-only. Taugentic does not persist or migrate model
catalog state.

## Harness boundaries

- Native harness providers use `ta-model-catalog` for enumeration, validation,
  and the metadata-derived default.
- Codex app-server models come from the authenticated Codex session.
- ACP models come from the connected ACP session.

Codex and ACP discovery are protocol boundaries, not alternate native catalog
owners. The desktop displays daemon projections and does not validate or
default model identifiers itself.

## Updating the snapshot

```sh
node scripts/sync-model-catalog.mjs
cargo test -p ta-model-catalog
```

Review the generated diff and confirm current providers and frontier models are
present before committing it.
