use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    HostCapabilities, LocalIpcKind, NetworkAllowlistSupport, NetworkPolicySupport,
    SandboxCapabilities, SandboxCapability, SandboxCapabilityReason, SandboxKind,
};

const WINDOWS_SANDBOX_HELPER: &str = "ta-windows-sandbox.exe";

pub fn current_capabilities() -> HostCapabilities {
    let sandbox_capabilities = sandbox_capabilities();
    HostCapabilities {
        local_ipc: LocalIpcKind::WindowsNamedPipe,
        sandbox: sandbox_kind_for_probe(sandbox_capabilities),
        supports_unix_peer_credentials: false,
        supports_launchd_user_services: false,
        supports_systemd_user_services: false,
        supports_windows_service_control: true,
    }
}

pub fn secrets_backend_capability() -> crate::SecretsBackend {
    crate::SecretsBackend::CredentialManager
}

pub fn sandbox_capabilities() -> SandboxCapabilities {
    let helper_available = windows_sandbox_helper_path().is_some();
    sandbox_capabilities_for_probe(helper_available)
}

pub fn windows_sandbox_helper_path() -> Option<PathBuf> {
    let current_exe = env::current_exe().ok()?;
    windows_sandbox_helper_path_from_exe(current_exe)
}

fn windows_sandbox_helper_path_from_exe(current_exe: PathBuf) -> Option<PathBuf> {
    let directory = current_exe.parent()?;
    let helper = directory.join(WINDOWS_SANDBOX_HELPER);
    if is_safe_windows_sandbox_helper(&helper) {
        return Some(helper);
    }

    #[cfg(test)]
    {
        windows_sandbox_helper_path_from_deps_dir(directory)
    }

    #[cfg(not(test))]
    {
        None
    }
}

pub fn is_safe_windows_sandbox_helper(path: &Path) -> bool {
    if path.file_name() != Some(OsStr::new(WINDOWS_SANDBOX_HELPER)) {
        return false;
    }
    let Ok(canonical) = fs::canonicalize(path) else {
        return false;
    };
    let Ok(target_metadata) = fs::metadata(&canonical) else {
        return false;
    };
    let Ok(path_metadata) = fs::symlink_metadata(path) else {
        return false;
    };

    !path_metadata.file_type().is_symlink()
        && path_metadata.file_type().is_file()
        && target_metadata.file_type().is_file()
}

#[cfg(test)]
fn windows_sandbox_helper_path_from_deps_dir(directory: &Path) -> Option<PathBuf> {
    if directory.file_name()? != "deps" {
        return None;
    }

    let helper = directory.parent()?.join(WINDOWS_SANDBOX_HELPER);
    is_safe_windows_sandbox_helper(&helper).then_some(helper)
}

fn sandbox_capabilities_for_probe(helper_available: bool) -> SandboxCapabilities {
    let network_policy = windows_network_policy_support(helper_available);
    SandboxCapabilities {
        helper_available,
        restricted_token_job: helper_available,
        appcontainer: helper_available,
        filesystem_allowlist: helper_available,
        network_default_deny: helper_available,
        network_destination_allowlist: network_policy.allowlist.ip_cidr.is_supported(),
        network_policy,
    }
}

fn windows_network_policy_support(helper_available: bool) -> NetworkPolicySupport {
    if !helper_available {
        return NetworkPolicySupport {
            off: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxHelperUnavailable,
            },
            loopback: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxHelperUnavailable,
            },
            allowlist: NetworkAllowlistSupport::unsupported(
                SandboxCapabilityReason::SandboxHelperUnavailable,
            ),
            open: SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxHelperUnavailable,
            },
        };
    }

    NetworkPolicySupport {
        off: SandboxCapability::Supported,
        loopback: SandboxCapability::Supported,
        allowlist: NetworkAllowlistSupport {
            tcp_port: SandboxCapability::Supported,
            ip_cidr: SandboxCapability::Supported,
            domain_name: SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::WindowsDomainAllowlistRequiresResolver,
            },
        },
        open: SandboxCapability::Supported,
    }
}

fn sandbox_kind_for_probe(capabilities: SandboxCapabilities) -> SandboxKind {
    if capabilities.appcontainer {
        SandboxKind::WindowsAppContainer
    } else if capabilities.restricted_token_job {
        SandboxKind::WindowsRestrictedTokenJob
    } else {
        SandboxKind::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_restricted_token_job_when_helper_is_available() {
        assert_eq!(
            sandbox_kind_for_probe(SandboxCapabilities {
                appcontainer: false,
                ..sandbox_capabilities_for_probe(true)
            }),
            SandboxKind::WindowsRestrictedTokenJob
        );
        assert_eq!(
            sandbox_kind_for_probe(sandbox_capabilities_for_probe(true)),
            SandboxKind::WindowsAppContainer
        );
    }

    #[test]
    fn reports_unsupported_when_helper_is_missing() {
        assert_eq!(
            sandbox_kind_for_probe(sandbox_capabilities_for_probe(false)),
            SandboxKind::Unsupported
        );
    }

    #[test]
    fn reports_appcontainer_capabilities_when_helper_is_available() {
        assert_eq!(
            sandbox_capabilities_for_probe(true),
            SandboxCapabilities {
                helper_available: true,
                restricted_token_job: true,
                appcontainer: true,
                filesystem_allowlist: true,
                network_default_deny: true,
                network_destination_allowlist: true,
                network_policy: NetworkPolicySupport {
                    off: SandboxCapability::Supported,
                    loopback: SandboxCapability::Supported,
                    allowlist: NetworkAllowlistSupport {
                        tcp_port: SandboxCapability::Supported,
                        ip_cidr: SandboxCapability::Supported,
                        domain_name: SandboxCapability::FailClosed {
                            reason: SandboxCapabilityReason::WindowsDomainAllowlistRequiresResolver,
                        },
                    },
                    open: SandboxCapability::Supported,
                },
            }
        );
    }

    #[test]
    fn reports_windows_wfp_allowlist_support() {
        assert_eq!(
            windows_network_policy_support(true).allowlist.ip_cidr,
            SandboxCapability::Supported
        );
        assert_eq!(
            windows_network_policy_support(true).allowlist.domain_name,
            SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::WindowsDomainAllowlistRequiresResolver,
            }
        );
        assert_eq!(
            windows_network_policy_support(false).open,
            SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::SandboxHelperUnavailable,
            }
        );
    }

    #[test]
    fn reports_credential_manager_for_windows_secrets() {
        assert_eq!(
            secrets_backend_capability(),
            crate::SecretsBackend::CredentialManager
        );
    }

    #[test]
    fn probes_helper_next_to_current_exe() {
        let root = unique_probe_root("sibling");
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("create bin dir");
        let helper = bin.join(WINDOWS_SANDBOX_HELPER);
        std::fs::write(&helper, "").expect("write helper");

        assert_eq!(
            windows_sandbox_helper_path_from_exe(bin.join("test-bin.exe")),
            Some(helper)
        );

        std::fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_probe_rejects_wrong_path() {
        let root = unique_probe_root("wrong-helper");
        std::fs::create_dir_all(&root).expect("create probe dir");
        let candidate = root.join("not-the-helper.exe");
        std::fs::write(&candidate, "").expect("write candidate");

        assert!(!is_safe_windows_sandbox_helper(&candidate));

        std::fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_probe_accepts_regular_helper_file() {
        let root = unique_probe_root("regular-helper");
        std::fs::create_dir_all(&root).expect("create probe dir");
        let helper = root.join(WINDOWS_SANDBOX_HELPER);
        std::fs::write(&helper, "").expect("write helper");

        assert!(is_safe_windows_sandbox_helper(&helper));

        std::fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_probe_rejects_symlink_candidate() {
        let root = unique_probe_root("symlink-helper");
        std::fs::create_dir_all(&root).expect("create probe dir");
        let target = root.join("real-helper.exe");
        let candidate = root.join(WINDOWS_SANDBOX_HELPER);
        std::fs::write(&target, "").expect("write target");
        match std::os::windows::fs::symlink_file(&target, &candidate) {
            Ok(()) => assert!(!is_safe_windows_sandbox_helper(&candidate)),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {}
            Err(error) => panic!("create symlink: {error}"),
        }

        std::fs::remove_dir_all(root).expect("remove probe dir");
    }

    fn unique_probe_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "ta-windows-helper-probe-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }
}
