use std::{fmt, path::PathBuf};

use crate::{NetworkPolicy, SandboxProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsNetworkCapability {
    DefaultDeny,
    Loopback,
    DestinationAllowlist,
    InternetClient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsSandboxProfileError {
    DestinationAllowlistUnsupported,
}

impl fmt::Display for WindowsSandboxProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationAllowlistUnsupported => formatter.write_str(
                "Windows AppContainer network allowlists require destination-aware WFP enforcement",
            ),
        }
    }
}

pub fn validate_windows_appcontainer_profile(
    profile: &SandboxProfile,
) -> Result<(), WindowsSandboxProfileError> {
    windows_network_capability(profile.network_policy()).map(|_| ())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsFilesystemGrant {
    path: PathBuf,
    access: WindowsFilesystemAccess,
}

impl WindowsFilesystemGrant {
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn access(&self) -> WindowsFilesystemAccess {
        self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFilesystemAccess {
    Read,
    Write,
}

pub fn filesystem_grants(profile: &SandboxProfile) -> Vec<WindowsFilesystemGrant> {
    let mut grants = Vec::new();
    grants.extend(
        profile
            .fs_read_paths()
            .iter()
            .cloned()
            .map(|path| WindowsFilesystemGrant {
                path,
                access: WindowsFilesystemAccess::Read,
            }),
    );
    grants.extend(
        profile
            .fs_write_paths()
            .iter()
            .cloned()
            .map(|path| WindowsFilesystemGrant {
                path,
                access: WindowsFilesystemAccess::Write,
            }),
    );
    grants
}

pub fn windows_network_capability(
    policy: &NetworkPolicy,
) -> Result<WindowsNetworkCapability, WindowsSandboxProfileError> {
    match policy {
        NetworkPolicy::Off => Ok(WindowsNetworkCapability::DefaultDeny),
        NetworkPolicy::Open => Ok(WindowsNetworkCapability::InternetClient),
        NetworkPolicy::Loopback => Ok(WindowsNetworkCapability::Loopback),
        NetworkPolicy::Allowlist(entries) if entries.is_empty() => {
            Ok(WindowsNetworkCapability::DefaultDeny)
        }
        NetworkPolicy::Allowlist(_) => Ok(WindowsNetworkCapability::DestinationAllowlist),
    }
}

pub fn appcontainer_capability_names(
    policy: &NetworkPolicy,
) -> Result<&'static [&'static str], WindowsSandboxProfileError> {
    match windows_network_capability(policy)? {
        WindowsNetworkCapability::DefaultDeny => Ok(&[]),
        WindowsNetworkCapability::Loopback => Ok(&[]),
        WindowsNetworkCapability::DestinationAllowlist => Ok(&["internetClient"]),
        WindowsNetworkCapability::InternetClient => Ok(&["internetClient"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_appcontainer_default_deny_and_open_networking() {
        assert_eq!(
            windows_network_capability(&NetworkPolicy::Off),
            Ok(WindowsNetworkCapability::DefaultDeny)
        );
        assert_eq!(
            windows_network_capability(&NetworkPolicy::Loopback),
            Ok(WindowsNetworkCapability::Loopback)
        );
        assert_eq!(
            appcontainer_capability_names(&NetworkPolicy::Open),
            Ok(&["internetClient"][..])
        );
    }

    #[test]
    fn classifies_destination_allowlists_for_wfp_enforcement() {
        assert_eq!(
            windows_network_capability(&NetworkPolicy::Allowlist(vec!["127.0.0.1".to_string()])),
            Ok(WindowsNetworkCapability::DestinationAllowlist)
        );
        assert_eq!(
            appcontainer_capability_names(&NetworkPolicy::Allowlist(vec!["127.0.0.1".to_string()])),
            Ok(&["internetClient"][..])
        );
        assert_eq!(
            windows_network_capability(&NetworkPolicy::Allowlist(Vec::new())),
            Ok(WindowsNetworkCapability::DefaultDeny)
        );
    }

    #[test]
    fn validates_filesystem_paths_for_acl_backend() {
        let profile = SandboxProfile::new()
            .read_path(r"C:\repo")
            .write_path(r"C:\repo\target");

        assert!(validate_windows_appcontainer_profile(&profile).is_ok());
        assert_eq!(
            filesystem_grants(&profile),
            vec![
                WindowsFilesystemGrant {
                    path: PathBuf::from(r"C:\repo"),
                    access: WindowsFilesystemAccess::Read,
                },
                WindowsFilesystemGrant {
                    path: PathBuf::from(r"C:\repo\target"),
                    access: WindowsFilesystemAccess::Write,
                },
            ]
        );
    }
}
