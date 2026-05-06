# Windows Sandbox Discovery

`ta-host-platform` is the runtime probe SSOT for Windows sandbox capability
reporting. `ta-sandbox` owns the platform-neutral `SandboxProfile` contract, and
`ta-windows-sandbox` is the only Windows implementation boundary that turns that
profile into Win32 process, filesystem, and network controls.

## AppContainer Mode

The canonical Windows strategy is an AppContainer-backed helper process layered
with the existing restricted token and kill-on-close Job Object. The helper must
launch the requested child with `CreateProcessAsUserW` and `STARTUPINFOEXW`.
`STARTUPINFOEXW.lpAttributeList` carries
`PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`, whose
`SECURITY_CAPABILITIES.AppContainerSid` is the AppContainer SID and whose
capability list is derived from `SandboxProfile`.

The helper lifecycle is:

1. Create or open a deterministic per-invocation AppContainer profile with
   `CreateAppContainerProfile`.
2. Resolve the profile SID and create a restricted primary token with
   `CreateRestrictedToken`.
3. Build an extended startup attribute list containing both the allowed stdio
   handle list and AppContainer security capabilities.
4. Spawn suspended with `CreateProcessAsUserW`.
5. Assign the process to the Job Object before resume.
6. Wait for exit and always close handles; delete the AppContainer profile when
   the helper owns a temporary profile.

Profile names must be collision-resistant and not derived from untrusted command
text. Profile deletion is part of cleanup. If process launch fails after profile
creation, cleanup still runs. Errors are surfaced through the existing typed
host-platform/sandbox error path with Win32 operation names and error codes; no
normal failure path may panic.

## Network Restrictions

Job Objects do not provide destination-aware network restrictions, so they are
not a valid implementation for `NetworkPolicy::Off`, `Loopback`, or
`Allowlist`. The canonical strategy is:

- `Off`: AppContainer with no `internetClient`, `internetClientServer`, or
  `privateNetworkClientServer` capability.
- `Open`: grant `internetClient`.
- `Loopback`: keep internet capabilities omitted and add a scoped AppContainer
  loopback exemption with `NetworkIsolationSetAppContainerConfig`, restoring the
  previous exemption list on cleanup.
- `Allowlist`: use Windows Filtering Platform (WFP) rules owned by the helper
  for destination-aware policy; fail closed if WFP rule installation is
  unavailable.

AppContainer omission gives the default-deny baseline. WFP is the correct
extension point for fine-grained destination policy. The implementation must not
pretend a Job Object setting enforces address allowlists.

## Filesystem Allowlist

AppContainer filesystem access is granted by ACLs on specific paths. For each
`SandboxProfile.fs_read_paths` and `SandboxProfile.fs_write_paths` entry, the
helper resolves a canonical Windows path, grants the AppContainer SID an
explicit allow ACE, and records the original security descriptor needed to
restore the path.

The Win32 APIs are `SetEntriesInAclW` and `SetNamedSecurityInfoW`. Read paths map
to read/execute-style file access. Write paths map to read/write/delete-child
access appropriate for the path type. All added ACEs are tracked by path and
restored on cleanup. Leaving permanent ACL changes on user paths is a blocking
failure.

## Runtime Probe Matrix

`ta-host-platform` should expose sandbox capability facts next to the existing
`secrets_backend_capability` pattern. The probe should report:

- helper presence and safety
- restricted-token/job support
- AppContainer support
- filesystem allowlist support
- network default-deny support
- destination-aware WFP support

`HostCapabilities.sandbox` remains the coarse current backend kind. Dedicated
sandbox capability functions are the SSOT for feature-level decisions so UI,
daemon, and CLI callers do not duplicate Windows version or helper probing.

## Dependencies

Prefer Microsoft-maintained Win32 bindings. The workspace already pins
`windows-sys = "0.52"` and the Windows helper uses it today. Adding the higher
level `windows` crate should be justified separately if typed COM/HSTRING
support becomes necessary. Any new Win32 binding version is a supply-chain
surface and must remain workspace-pinned instead of duplicated per crate.

## Tests

Pure logic stays cross-platform and unit tested: profile-to-capability mapping,
network policy classification, path access classification, and cleanup plan
construction. Windows-only integration tests use `#[cfg(target_os = "windows")]`
and exercise process launch, AppContainer isolation, ACL grant/revert, and WFP
rule cleanup on a real Windows host.

Development on macOS requires both host checks and Windows cross-target checks:

```sh
cargo check --workspace
cargo check -p ta-windows-sandbox --target x86_64-pc-windows-msvc
cargo clippy -p ta-windows-sandbox --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```
