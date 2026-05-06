# Sandbox Network Policy Discovery

`ta-sandbox` is the SSOT for the platform-neutral sandbox contract. The
canonical network policy type is `crates/ta-sandbox/src/profile.rs::NetworkPolicy`
and the exact variant set is:

- `Off`: deny external networking in backends that own network controls.
- `Loopback`: allow loopback destinations only.
- `Allowlist(Vec<String>)`: backend-owned destination allowlist. Current
  production sub-shapes are TCP ports, IP/CIDR ranges, and domain names.
- `Open`: unrestricted egress; filesystem sandboxing still applies.

No backend owns a separate policy enum. Backend-local types may classify the
canonical policy for implementation, but they must not introduce a second public
policy contract.

## Capability Reporting

`crates/ta-host-platform` exposes sandbox capability reporting next to the
existing `secrets_backend_capability` pattern. `SandboxCapabilities` keeps the
coarse existing booleans and adds `network_policy: NetworkPolicySupport`.
`NetworkPolicySupport` reports `off`, `loopback`, `open`, and nested
`allowlist` sub-capabilities for `tcp_port`, `ip_cidr`, and `domain_name`.
Each entry is `Supported`, `FailClosed { reason }`, or
`Unsupported { reason }`.

`FailClosed` means the backend recognizes the canonical policy and rejects the
profile before launch when it cannot enforce it. `Unsupported` means the runtime
probe lacks a required OS/helper capability, so callers should not offer that
policy on this host. Diagnostics must surface the typed reason instead of
inferring support from OS names or booleans.

## Support Matrix

| Policy shape | Linux Landlock/bwrap | macOS Seatbelt | Windows AppContainer |
| --- | --- | --- | --- |
| `Off` | Implemented via Landlock TCP denial on kernel >= 6.7 or bwrap network namespace fallback. Unsupported when neither path is available. | Implemented by `(deny network*)`. | Implemented by AppContainer without internet capabilities. |
| `Open` | Implemented by filesystem Landlock with no network rules on kernel >= 5.13. Unsupported when filesystem Landlock is unavailable; bwrap fallback is `Off` only. | Implemented by `(allow network*)`. | Implemented with `internetClient`. |
| `Loopback` | FailClosed: Landlock TCP rules are port-only and cannot constrain destination address; canonical future path is backend-owned network namespace plus nftables/eBPF. | FailClosed: Seatbelt cannot express loopback-only egress for arbitrary processes; NetworkExtension is out of scope for the helper. | Implemented with AppContainer loopback exemption and no internet capabilities. |
| `Allowlist` TCP ports | Implemented via Landlock ABI v4 `ConnectTcp` port rules on kernel >= 6.7. Unsupported otherwise. | FailClosed: Seatbelt cannot express destination-aware allowlists. | Implemented with helper-owned WFP permit filters plus lower-priority default-deny filters. |
| `Allowlist` IP/CIDR | FailClosed: Landlock has no destination address predicate. | FailClosed: Seatbelt cannot express destination-aware allowlists. | Implemented with helper-owned WFP address/prefix filters plus lower-priority default-deny filters. |
| `Allowlist` domain names | FailClosed: DNS names require a managed resolver/proxy and are not an OS primitive. | FailClosed: Seatbelt cannot express destination-aware allowlists. | FailClosed: WFP can enforce resolved IPs, but domain-to-IP lifecycle is a separate managed resolver/proxy problem. |

## Linux Strategy

The primary Linux path is the existing helper:

- Landlock filesystem rules are the baseline for `Open` and filesystem
  allowlists when kernel support is available.
- Landlock ABI v4 TCP rules are canonical for `Off` and TCP-port allowlists on
  kernel >= 6.7.
- bwrap remains a production-grade fallback for `Off` only because its network
  namespace can fully disconnect the child. It must not be used for `Open`,
  `Loopback`, or `Allowlist` because that would silently change semantics.

Linux `Loopback` and IP/CIDR allowlists need address-aware enforcement. The
canonical future path is a helper-owned network namespace with nftables or
cgroup eBPF rules. Landlock-network is not sufficient because the kernel API is
port-only as of ABI v4.

## macOS Strategy

The production path remains `/usr/bin/sandbox-exec` with Seatbelt profiles.
Seatbelt honestly implements `Off` and `Open`. Loopback and destination
allowlists fail closed because Seatbelt does not provide process-local
destination predicates. NetworkExtension can express richer policy at the
system-extension layer, but it is not an in-process helper fallback and would
require product packaging, entitlement, and lifecycle work before it could be a
production backend.

## Windows Strategy

The production path is AppContainer plus restricted token, Job Object, scoped
filesystem ACL grants, and network controls:

- `Off`: omit `internetClient`, `internetClientServer`, and
  `privateNetworkClientServer`.
- `Open`: grant `internetClient`.
- `Loopback`: omit internet capabilities and apply an AppContainer loopback
  exemption during the helper lifetime.
- `Allowlist`: grant `internetClient`, then install helper-owned Windows
  Filtering Platform permit filters for configured TCP ports and IP/CIDR
  entries plus lower-priority default-deny filters for the AppContainer package.

WFP is required for IP/CIDR and port allowlists. The implementation uses the
existing workspace-pinned `windows-sys` binding, installs dynamic-session filters
in a transaction, scopes them to the sandboxed AppContainer package SID, and
cleans them up when the helper closes the WFP engine. Domain-name allowlists
remain fail-closed until a managed resolver/proxy owns DNS-to-address churn.

## External Patterns

The Codex Windows sandbox keeps WFP provider and sublayer GUIDs stable and wraps
filter installation in explicit engine/transaction RAII. That pattern is useful
for Taugentic's WFP slice, but Taugentic should use ephemeral helper-owned
filters scoped to the current sandbox instead of copying Codex's persistent
account setup.

Codex's Linux sandbox demonstrates a hard fail pattern: bwrap is the primary
namespace boundary and the inner helper applies seccomp without falling back to
weaker paths. Taugentic should keep the same fail-closed rule for network policy
selection: if the chosen kernel or namespace primitive cannot express the
requested canonical policy, reject before launch.

## Tests And Gates

Pure logic stays cross-platform: capability matrix construction, policy
classification, allowlist parsing, and fail-closed error mapping. OS-gated tests
use `#[cfg(target_os = "...")]` and run only on matching dev hosts or CI.

Required local gates per touched slice:

```sh
cargo fmt --all
cargo clippy -p <touched crate> --all-targets -- -D warnings
cargo test -p <touched crate>
cargo check --workspace
```

Windows-touching changes additionally require:

```sh
cargo check -p ta-windows-sandbox --target x86_64-pc-windows-msvc
cargo clippy -p ta-windows-sandbox --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

Linux-touching changes should run:

```sh
cargo check -p ta-linux-sandbox --target x86_64-unknown-linux-gnu
```

If a target is missing, install it with `rustup target add ...`; if installation
is blocked, document the skipped gate and the manual smoke required on that OS.
