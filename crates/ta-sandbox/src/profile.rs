use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxProfile {
    fs_read_paths: Vec<PathBuf>,
    fs_write_paths: Vec<PathBuf>,
    network: NetworkPolicy,
    env_allowlist: Vec<String>,
    child_inherits_tty: bool,
}

impl SandboxProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.fs_read_paths.push(path.into());
        self
    }

    pub fn write_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.fs_write_paths.push(path.into());
        self
    }

    pub fn network(mut self, policy: NetworkPolicy) -> Self {
        self.network = policy;
        self
    }

    /// Allows a parent environment variable to be copied into the child.
    ///
    /// When this profile is active, `ta-exec` clears inherited environment first.
    /// Only names recorded here are reloaded from the parent environment, and
    /// caller-provided overrides are accepted only for these same names.
    pub fn env(mut self, name: impl Into<String>) -> Self {
        self.env_allowlist.push(name.into());
        self
    }

    pub fn child_inherits_tty(mut self, inherits_tty: bool) -> Self {
        self.child_inherits_tty = inherits_tty;
        self
    }

    pub fn fs_read_paths(&self) -> &[PathBuf] {
        &self.fs_read_paths
    }

    pub fn fs_write_paths(&self) -> &[PathBuf] {
        &self.fs_write_paths
    }

    pub fn network_policy(&self) -> &NetworkPolicy {
        &self.network
    }

    pub fn env_allowlist(&self) -> &[String] {
        &self.env_allowlist
    }

    pub fn allows_env(&self, name: &str) -> bool {
        self.env_allowlist.iter().any(|allowed| allowed == name)
    }

    pub fn child_inherits_tty_enabled(&self) -> bool {
        self.child_inherits_tty
    }

    pub fn reads_path(&self, path: &Path) -> bool {
        self.fs_read_paths
            .iter()
            .any(|allowed| path.starts_with(allowed))
    }

    pub fn writes_path(&self, path: &Path) -> bool {
        self.fs_write_paths
            .iter()
            .any(|allowed| path.starts_with(allowed))
    }
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self {
            fs_read_paths: Vec::new(),
            fs_write_paths: Vec::new(),
            network: NetworkPolicy::Off,
            env_allowlist: Vec::new(),
            child_inherits_tty: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkPolicy {
    /// Deny external networking in platform backends that own network controls.
    Off,
    /// Allow only loopback destinations (`127.0.0.0/8` and `::1`) in backends
    /// with address-aware network controls.
    ///
    /// The current Linux helper intentionally rejects this policy fail-closed:
    /// Landlock ABI v4 can restrict TCP connects by port but not by destination
    /// address. Linux support requires dedicated address-aware infrastructure
    /// such as cgroup eBPF enforcement or a backend-owned userspace proxy.
    Loopback,
    /// Backend-owned allowlist. Linux currently accepts TCP port entries such as
    /// `443` or `tcp:443`; this is not a substitute for loopback-only semantics
    /// because those rules apply to any destination address on the allowed port.
    Allowlist(Vec<String>),
    /// Allow unrestricted network egress; filesystem sandboxing still applies.
    ///
    /// `Open` does not enforce URL, host, or content policy. Callers that need
    /// destination-level control must use a stricter backend-owned policy.
    Open,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_deny_network_and_no_tty() {
        let profile = SandboxProfile::default();

        assert_eq!(profile.network_policy(), &NetworkPolicy::Off);
        assert!(!profile.child_inherits_tty_enabled());
        assert!(profile.fs_read_paths().is_empty());
        assert!(profile.fs_write_paths().is_empty());
    }

    #[test]
    fn builder_records_paths_network_env_and_tty() {
        let profile = SandboxProfile::new()
            .read_path("/repo")
            .write_path("/repo/target")
            .network(NetworkPolicy::Loopback)
            .env("HOME")
            .child_inherits_tty(true);

        assert!(profile.reads_path(Path::new("/repo/src/lib.rs")));
        assert!(profile.writes_path(Path::new("/repo/target/debug/app")));
        assert!(!profile.writes_path(Path::new("/repo/src/lib.rs")));
        assert_eq!(profile.network_policy(), &NetworkPolicy::Loopback);
        assert!(profile.allows_env("HOME"));
        assert!(profile.child_inherits_tty_enabled());
    }
}
