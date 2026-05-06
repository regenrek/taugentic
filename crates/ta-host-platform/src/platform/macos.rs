use std::{env, path::PathBuf};

use crate::{
    HostCapabilities, LocalIpcKind, NetworkAllowlistSupport, NetworkPolicySupport,
    SandboxCapabilities, SandboxCapability, SandboxCapabilityReason, SandboxKind,
};

const MACOS_RUNTIME_DIR_FALLBACK: &str = "/tmp/taugentic/runtime";

pub fn current_capabilities() -> HostCapabilities {
    HostCapabilities {
        local_ipc: LocalIpcKind::UnixDomainSocket {
            runtime_dir: runtime_dir(),
        },
        sandbox: SandboxKind::MacosSeatbelt,
        supports_unix_peer_credentials: true,
        supports_launchd_user_services: true,
        supports_systemd_user_services: false,
        supports_windows_service_control: false,
    }
}

pub fn secrets_backend_capability() -> crate::SecretsBackend {
    crate::SecretsBackend::Keychain
}

pub fn sandbox_capabilities() -> SandboxCapabilities {
    SandboxCapabilities {
        helper_available: true,
        restricted_token_job: false,
        appcontainer: false,
        filesystem_allowlist: true,
        network_default_deny: true,
        network_destination_allowlist: false,
        network_policy: NetworkPolicySupport {
            off: SandboxCapability::Supported,
            loopback: SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::MacosSeatbeltDestinationPolicyUnsupported,
            },
            allowlist: NetworkAllowlistSupport::fail_closed(
                SandboxCapabilityReason::MacosSeatbeltDestinationPolicyUnsupported,
            ),
            open: SandboxCapability::Supported,
        },
    }
}

fn runtime_dir() -> PathBuf {
    runtime_dir_from_env(env::var_os("XDG_RUNTIME_DIR"), env::var_os("HOME"))
}

fn runtime_dir_from_env(
    xdg_runtime_dir: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    normalize_env_path(xdg_runtime_dir)
        .map(PathBuf::from)
        .unwrap_or_else(|| stable_user_runtime_dir(normalize_env_path(home)))
}

fn stable_user_runtime_dir(home: Option<String>) -> PathBuf {
    match home {
        Some(home) => PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("taugentic")
            .join("runtime"),
        _ => PathBuf::from(MACOS_RUNTIME_DIR_FALLBACK),
    }
}

fn normalize_env_path(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_xdg_runtime_dir_when_present() {
        let runtime_dir =
            runtime_dir_from_env(Some("/run/user/501".into()), Some("/Users/alice".into()));

        assert_eq!(runtime_dir, PathBuf::from("/run/user/501"));
    }

    #[test]
    fn falls_back_to_stable_user_runtime_dir() {
        let runtime_dir = stable_user_runtime_dir(Some("/Users/alice".to_string()));

        assert_eq!(
            runtime_dir,
            PathBuf::from("/Users/alice/Library/Application Support/taugentic/runtime")
        );
    }

    #[test]
    fn falls_back_to_fixed_tmp_runtime_dir_without_home() {
        let runtime_dir = stable_user_runtime_dir(None);

        assert_eq!(runtime_dir, PathBuf::from(MACOS_RUNTIME_DIR_FALLBACK));
    }

    #[test]
    fn treats_whitespace_runtime_dir_as_missing() {
        let runtime_dir = runtime_dir_from_env(Some("   ".into()), Some("/Users/alice".into()));

        assert_eq!(
            runtime_dir,
            PathBuf::from("/Users/alice/Library/Application Support/taugentic/runtime")
        );
    }

    #[test]
    fn treats_whitespace_home_as_missing() {
        let runtime_dir = runtime_dir_from_env(None, Some("   ".into()));

        assert_eq!(runtime_dir, PathBuf::from(MACOS_RUNTIME_DIR_FALLBACK));
    }

    #[test]
    fn reports_seatbelt_sandbox_kind() {
        assert_eq!(current_capabilities().sandbox, SandboxKind::MacosSeatbelt);
    }

    #[test]
    fn reports_seatbelt_sandbox_capabilities() {
        assert_eq!(
            sandbox_capabilities(),
            SandboxCapabilities {
                helper_available: true,
                restricted_token_job: false,
                appcontainer: false,
                filesystem_allowlist: true,
                network_default_deny: true,
                network_destination_allowlist: false,
                network_policy: NetworkPolicySupport {
                    off: SandboxCapability::Supported,
                    loopback: SandboxCapability::FailClosed {
                        reason: SandboxCapabilityReason::MacosSeatbeltDestinationPolicyUnsupported,
                    },
                    allowlist: NetworkAllowlistSupport::fail_closed(
                        SandboxCapabilityReason::MacosSeatbeltDestinationPolicyUnsupported,
                    ),
                    open: SandboxCapability::Supported,
                },
            }
        );
    }

    #[test]
    fn reports_keychain_for_macos_secrets() {
        assert_eq!(
            secrets_backend_capability(),
            crate::SecretsBackend::Keychain
        );
    }

    #[test]
    fn trims_runtime_dir_and_home_values() {
        let runtime_dir = runtime_dir_from_env(
            Some(" /run/user/501 ".into()),
            Some(" /Users/alice ".into()),
        );

        assert_eq!(runtime_dir, PathBuf::from("/run/user/501"));
    }
}
