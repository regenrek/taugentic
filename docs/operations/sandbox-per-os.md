# Sandbox Per OS

Use this runbook to interpret sandbox capability and network-policy behavior on an operator host. The canonical engineering matrix is `crates/ta-host-platform/docs/sandbox-network-policy-discovery.md`; this page is an operator entry point.

## Canonical Policy Owner

`crates/ta-sandbox/src/profile.rs::NetworkPolicy` owns the platform-neutral policy:

- `Off`
- `Loopback`
- `Allowlist(Vec<String>)`
- `Open`

Backends must either enforce the requested canonical policy or fail closed before launch. Do not infer capability from OS name alone; read the runtime capability snapshot.

## macOS

Backend: Seatbelt through `/usr/bin/sandbox-exec`.

Implemented:

- Filesystem sandboxing.
- `NetworkPolicy::Off` through `(deny network*)`.
- `NetworkPolicy::Open` through `(allow network*)`.

Fail-closed:

- `Loopback`: Seatbelt cannot express loopback-only egress for arbitrary processes.
- Destination allowlists: Seatbelt cannot express destination-aware TCP port, IP/CIDR, or domain allowlists.

Operator check:

```sh
just ta daemon status --json
just ta daemon logs --tail 200
```

Then inspect Mission Control **Sandbox** capability fields.

## Linux

Backend: Landlock plus helper fallback, with bubblewrap only for fully disconnected networking.

Implemented:

- Filesystem Landlock on kernels with required filesystem ABI.
- `NetworkPolicy::Off` through Landlock TCP denial on kernel >= 6.7, or bubblewrap network namespace fallback.
- TCP-port allowlists through Landlock ABI v4 `ConnectTcp` on kernel >= 6.7.
- `NetworkPolicy::Open` when filesystem Landlock is available and no network rules are requested.

Fail-closed:

- `Loopback`: Landlock TCP rules are port-only and cannot constrain destination address.
- IP/CIDR allowlists: Landlock has no destination address predicate.
- Domain-name allowlists: DNS names require a managed resolver/proxy and are not an OS primitive.
- Bubblewrap is not used for `Open`, `Loopback`, or `Allowlist`; doing so would silently change policy semantics.

Known future path for loopback/IP policies: backend-owned network namespace with nftables or cgroup eBPF.

Operator check:

```sh
uname -r
just ta daemon logs --tail 200
```

Full TCP network policy requires Linux kernel >= 6.7.

## Windows

Backend: AppContainer plus restricted token, Job Object, scoped filesystem ACL grants, and Windows Filtering Platform.

Implemented:

- `NetworkPolicy::Off`: AppContainer without internet capabilities.
- `NetworkPolicy::Open`: AppContainer with `internetClient`.
- `Loopback`: AppContainer loopback exemption without internet capabilities.
- TCP-port allowlists: helper-owned WFP permit filters plus default-deny filters.
- IP/CIDR allowlists: helper-owned WFP address/prefix filters plus default-deny filters.

Fail-closed:

- Domain-name allowlists: WFP can enforce resolved IPs, but DNS-to-address lifecycle needs a managed resolver/proxy owner.

Operator check:

```powershell
just ta daemon status --json
just ta daemon logs --tail 200
```

If WFP setup fails, capture the daemon log tail and the Mission Control sandbox snapshot.

## Read Capability Snapshot

Preferred UI path:

1. Open **Mission Control**.
2. Read the daemon diagnostics panel.
3. Inspect the sandbox fields: OS, sandbox kind, helper availability, filesystem allowlist, default-deny networking, and destination allowlist support.

Raw JSON-RPC path:

```sh
SOCKET="$(just ta daemon status --json | jq -r '.socketPath')"
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"daemon.initialize","params":{"clientName":"ops-runbook","clientVersion":"0","protocolVersion":"2026-04-stage3","capabilities":{"notifications":false,"eventSubscriptions":false}}}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"daemon.diagnostics.snapshot","params":{}}'
} | nc -U "$SOCKET"
```

Read `result.sandbox`.

## Escalation

File a bug with:

- OS and version.
- Kernel version on Linux.
- Requested `NetworkPolicy`.
- Mission Control sandbox snapshot or `daemon.diagnostics.snapshot.result.sandbox`.
- `just ta daemon logs --tail 300`.
- The exact run id and recipe id.

Do not weaken a sandbox profile to make a run pass unless the operator explicitly accepts unrestricted networking for that run.
