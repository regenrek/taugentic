use serde::{Deserialize, Serialize};
use ta_host_platform::{HostPlatform, LocalIpcKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneCapabilities {
    pub supports_network: bool,
    pub supports_shell: bool,
    pub supports_file_edits: bool,
    pub supports_subagents: bool,
}

impl LaneCapabilities {
    pub fn from_host_platform(host_platform: &HostPlatform) -> Self {
        let supports_local_process_ops = matches!(
            host_platform.capabilities.local_ipc,
            LocalIpcKind::UnixDomainSocket { .. } | LocalIpcKind::WindowsNamedPipe
        );

        Self {
            supports_network: false,
            supports_shell: supports_local_process_ops,
            supports_file_edits: supports_local_process_ops,
            supports_subagents: supports_local_process_ops,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.supports_shell || self.supports_file_edits || self.supports_subagents
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ta_host_platform::{
        HostCapabilities, HostOs, HostPlatform, LocalIpcKind, OsVersion, SandboxKind,
    };

    use super::*;

    #[test]
    fn derives_native_lane_defaults_from_host_platform() {
        let host_platform = HostPlatform {
            os: HostOs::Macos,
            version: OsVersion::parse("15.4.1"),
            edition: None,
            linux_distribution: None,
            capabilities: HostCapabilities {
                local_ipc: LocalIpcKind::UnixDomainSocket {
                    runtime_dir: PathBuf::from("/tmp/taugentic"),
                },
                sandbox: SandboxKind::MacosSeatbelt,
                supports_unix_peer_credentials: true,
                supports_launchd_user_services: true,
                supports_systemd_user_services: false,
                supports_windows_service_control: false,
            },
        };

        assert_eq!(
            LaneCapabilities::from_host_platform(&host_platform),
            LaneCapabilities {
                supports_network: false,
                supports_shell: true,
                supports_file_edits: true,
                supports_subagents: true,
            }
        );
    }
}
