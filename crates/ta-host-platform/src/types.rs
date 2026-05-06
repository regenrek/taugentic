use std::{fmt, path::PathBuf};

use os_info::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostOs {
    Linux,
    Macos,
    Windows,
}

impl HostOs {
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }

        #[cfg(target_os = "macos")]
        {
            Self::Macos
        }

        #[cfg(windows)]
        {
            Self::Windows
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPlatform {
    pub os: HostOs,
    pub version: OsVersion,
    pub edition: Option<String>,
    pub linux_distribution: Option<LinuxDistribution>,
    pub capabilities: HostCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsVersion {
    pub raw: String,
    pub major: Option<u64>,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

impl OsVersion {
    pub fn from_os_info(version: &Version) -> Self {
        match version {
            Version::Unknown => Self {
                raw: "unknown".to_string(),
                major: None,
                minor: None,
                patch: None,
            },
            Version::Semantic(major, minor, patch) => Self {
                raw: version.to_string(),
                major: Some(*major),
                minor: Some(*minor),
                patch: Some(*patch),
            },
            other => Self::parse(other.to_string()),
        }
    }

    pub fn parse(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let mut segments = raw.split('.');
        let major = segments.next().and_then(|segment| segment.parse().ok());
        let minor = segments.next().and_then(|segment| segment.parse().ok());
        let patch = segments.next().and_then(|segment| segment.parse().ok());

        Self {
            raw,
            major,
            minor,
            patch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxDistribution {
    pub id: String,
    pub name: String,
    pub version: String,
    pub edition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    pub local_ipc: LocalIpcKind,
    pub sandbox: SandboxKind,
    pub supports_unix_peer_credentials: bool,
    pub supports_launchd_user_services: bool,
    pub supports_systemd_user_services: bool,
    pub supports_windows_service_control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretsBackend {
    Keychain,
    SecretService,
    CredentialManager,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxKind {
    MacosSeatbelt,
    LinuxLandlockBwrap,
    LinuxBwrap,
    WindowsAppContainer,
    WindowsRestrictedTokenJob,
    Unsupported,
}

impl fmt::Display for SandboxKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::MacosSeatbelt => "macos-seatbelt",
            Self::LinuxLandlockBwrap => "linux-landlock-bwrap",
            Self::LinuxBwrap => "linux-bwrap",
            Self::WindowsAppContainer => "windows-appcontainer",
            Self::WindowsRestrictedTokenJob => "windows-restricted-token-job",
            Self::Unsupported => "unsupported",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxCapabilities {
    pub helper_available: bool,
    pub restricted_token_job: bool,
    pub appcontainer: bool,
    pub filesystem_allowlist: bool,
    pub network_default_deny: bool,
    pub network_destination_allowlist: bool,
    pub network_policy: NetworkPolicySupport,
}

impl SandboxCapabilities {
    pub const fn unsupported() -> Self {
        Self {
            helper_available: false,
            restricted_token_job: false,
            appcontainer: false,
            filesystem_allowlist: false,
            network_default_deny: false,
            network_destination_allowlist: false,
            network_policy: NetworkPolicySupport::unsupported(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicySupport {
    pub off: SandboxCapability,
    pub loopback: SandboxCapability,
    pub allowlist: NetworkAllowlistSupport,
    pub open: SandboxCapability,
}

impl NetworkPolicySupport {
    pub const fn unsupported() -> Self {
        Self {
            off: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxBackendUnavailable,
            },
            loopback: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxBackendUnavailable,
            },
            allowlist: NetworkAllowlistSupport::unsupported(
                SandboxCapabilityReason::SandboxBackendUnavailable,
            ),
            open: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxBackendUnavailable,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkAllowlistSupport {
    pub tcp_port: SandboxCapability,
    pub ip_cidr: SandboxCapability,
    pub domain_name: SandboxCapability,
}

impl NetworkAllowlistSupport {
    pub const fn unsupported(reason: SandboxCapabilityReason) -> Self {
        Self {
            tcp_port: SandboxCapability::Unsupported { reason },
            ip_cidr: SandboxCapability::Unsupported { reason },
            domain_name: SandboxCapability::Unsupported { reason },
        }
    }

    pub const fn fail_closed(reason: SandboxCapabilityReason) -> Self {
        Self {
            tcp_port: SandboxCapability::FailClosed { reason },
            ip_cidr: SandboxCapability::FailClosed { reason },
            domain_name: SandboxCapability::FailClosed { reason },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxCapability {
    Supported,
    FailClosed { reason: SandboxCapabilityReason },
    Unsupported { reason: SandboxCapabilityReason },
}

impl SandboxCapability {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxCapabilityReason {
    SandboxBackendUnavailable,
    SandboxHelperUnavailable,
    LinuxLandlockFilesystemUnavailable,
    LinuxLandlockTcpUnavailable,
    LinuxLoopbackNeedsAddressAwareBackend,
    LinuxAllowlistRequiresTcpPorts,
    MacosSeatbeltDestinationPolicyUnsupported,
    WindowsWfpAllowlistUnavailable,
    WindowsDomainAllowlistRequiresResolver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalIpcKind {
    UnixDomainSocket { runtime_dir: PathBuf },
    WindowsNamedPipe,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_semantic_versions() {
        let version = OsVersion::from_os_info(&Version::Semantic(15, 4, 1));
        assert_eq!(version.raw, "15.4.1");
        assert_eq!(version.major, Some(15));
        assert_eq!(version.minor, Some(4));
        assert_eq!(version.patch, Some(1));
    }

    #[test]
    fn preserves_custom_versions() {
        let version = OsVersion::parse("24H2");
        assert_eq!(version.raw, "24H2");
        assert_eq!(version.major, None);
        assert_eq!(version.minor, None);
        assert_eq!(version.patch, None);
    }

    #[test]
    fn sandbox_kind_has_stable_wire_name() {
        assert_eq!(
            SandboxKind::LinuxLandlockBwrap.to_string(),
            "linux-landlock-bwrap"
        );
        assert_eq!(SandboxKind::LinuxBwrap.to_string(), "linux-bwrap");
        assert_eq!(
            SandboxKind::WindowsAppContainer.to_string(),
            "windows-appcontainer"
        );
    }

    #[test]
    fn unsupported_sandbox_capabilities_are_typed() {
        let capabilities = SandboxCapabilities::unsupported();

        assert_eq!(
            capabilities.network_policy.allowlist.tcp_port,
            SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxBackendUnavailable,
            }
        );
        assert!(!capabilities.network_policy.off.is_supported());
    }
}
