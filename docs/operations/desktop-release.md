# Desktop Release

Canonical owner: `.github/workflows/desktop-release.yml`

This runbook covers the packaged desktop release path only.
Do not invent parallel release steps in ad hoc shell notes or local CI jobs.

## Local Preflight

Run the full release gate before packaging:

```sh
just release-check
```

For tagged releases, the tag must match `apps/desktop/packages/main/package.json`:

```sh
pnpm --dir apps/desktop release:version -- --tag=vX.Y.Z
```

Publish mode is tag-owned too:

```sh
pnpm --dir apps/desktop release:publish-mode -- --ref=refs/tags/vX.Y.Z
```

Final tagged publishing is finalizer-owned too:

- matrix package jobs always run `package -- --publish never`
- only the final tagged release job may mutate GitHub Releases
- if the tag is not publishable, finalization fails before any release upload

Package the desktop app and generate the release manifest:

```sh
pnpm --dir apps/desktop package
pnpm --dir apps/desktop release:trust
pnpm --dir apps/desktop release:artifacts
```

Expected outputs land in `apps/desktop/release/`:

- packaged installer artifacts (`.dmg`, `.deb`, `.exe`, etc.)
- `release-manifest.json`
- `release-sha256.txt`
- `attestation-subjects.txt`

## CI Release Path

`desktop-release.yml` is the only canonical GitHub workflow for packaged desktop artifacts.

It does three things:

1. run `just release-check` as a preflight gate
2. package desktop artifacts on Linux, macOS, and Windows
3. aggregate one canonical release bundle and publish it only after the whole matrix is green

`just release-check` includes:

- Rust build + focused crate lib tests
- `cargo xtask check-daemon-foundation`
- `cargo xtask smoke-local-daemon`
- desktop `check`
- desktop `test`
- `scripts/security-check.mjs`

After packaging, the workflow also hard-fails if produced artifacts fail the
platform trust gate:

- macOS: codesign plus stapled notarization ticket
- Windows: Authenticode-valid signature

Non-tag runs are build-only. They package with `--publish never` and cannot
mutate GitHub Releases.

## Release Profiles

Release profile is owned by `TAUGENTIC_DESKTOP_RELEASE_PROFILE`.

Allowed values:

- `stable`
- `nightly`
- `mission-control`

The profile is the SSOT for packaged desktop identity:

- `appId`
- `productName`
- staged app package name
- artifact name stem
- profile channel identity

Do not hardcode those values anywhere else.

Current durable publish truth is narrower than the profile set:

- tagged `stable` releases publish durably through GitHub Releases
- `nightly` and `mission-control` are currently packaged identities and CI
  artifact shapes only; they are not durable published release channels and do
  not model their own publisher config

## Signing Secrets

Tagged macOS releases require:

- `CSC_LINK`
- `CSC_KEY_PASSWORD`
- `APPLE_ID`
- `APPLE_APP_SPECIFIC_PASSWORD`
- `APPLE_TEAM_ID`

Tagged Windows releases require:

- `CSC_LINK`
- `CSC_KEY_PASSWORD`

If these are missing, the workflow fails fast before packaging.

## Attestations

Public repositories can generate artifact attestations directly.

Private or internal repositories need GitHub Enterprise Cloud.
If that is available, set repository variable `TAUGENTIC_ENABLE_ATTESTATIONS=1`
to turn the attestation step on for non-public repos too.

## Durable Publisher

Tagged `stable` releases publish through GitHub Releases.

- the workflow creates or reuses a draft release for the tag
- matrix packaging uploads platform artifacts only into CI artifact storage for aggregation
- the final tagged release job is the only GitHub Release publisher
- that final job aggregates the downloaded matrix artifacts into one canonical release bundle and hard-fails on duplicate final asset names
- that final job rebuilds `release-manifest.json`, `release-sha256.txt`, and `attestation-subjects.txt` from the aggregated artifact set
- that final job uploads the packaged artifacts plus the canonical metadata bundle into the draft release
- a final job flips the draft to a real release only after the whole matrix is green
- non-tag dispatches never publish, even for `stable`

Do not add a second publisher path beside this GitHub release flow.
