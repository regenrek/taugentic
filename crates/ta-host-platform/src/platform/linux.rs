use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::{
    HostCapabilities, LocalIpcKind, NetworkAllowlistSupport, NetworkPolicySupport,
    SandboxCapabilities, SandboxCapability, SandboxCapabilityReason, SandboxKind,
};
use nix::unistd::Uid;
use secret_service::{EncryptionType, blocking::SecretService};

const LINUX_SANDBOX_HELPER: &str = "ta-linux-sandbox";
const LINUX_SANDBOX_HELPER_ENV: &str = "TA_LINUX_SANDBOX_HELPER";
const BWRAP_PROGRAM: &str = "/usr/bin/bwrap";

pub fn current_capabilities() -> HostCapabilities {
    HostCapabilities {
        local_ipc: LocalIpcKind::UnixDomainSocket {
            runtime_dir: runtime_dir(),
        },
        sandbox: sandbox_kind_for_probe(
            linux_sandbox_helper_path().is_some(),
            landlock_filesystem_rules_available(),
            landlock_tcp_rules_available(),
            linux_bwrap_path().is_some(),
        ),
        supports_unix_peer_credentials: true,
        supports_launchd_user_services: false,
        supports_systemd_user_services: true,
        supports_windows_service_control: false,
    }
}

pub fn secrets_backend_capability() -> crate::SecretsBackend {
    if secret_service_available() {
        crate::SecretsBackend::SecretService
    } else {
        crate::SecretsBackend::None
    }
}

pub fn sandbox_capabilities() -> SandboxCapabilities {
    let helper_available = linux_sandbox_helper_path().is_some();
    let landlock_fs_available = landlock_filesystem_rules_available();
    let landlock_tcp_available = landlock_tcp_rules_available();
    let bwrap_available = linux_bwrap_path().is_some();
    let supported = sandbox_kind_for_probe(
        helper_available,
        landlock_fs_available,
        landlock_tcp_available,
        bwrap_available,
    ) != SandboxKind::Unsupported;
    let network_policy = linux_network_policy_support(
        helper_available,
        landlock_fs_available,
        landlock_tcp_available,
        bwrap_available,
    );

    SandboxCapabilities {
        helper_available,
        restricted_token_job: false,
        appcontainer: false,
        filesystem_allowlist: supported,
        network_default_deny: network_policy.off.is_supported(),
        network_destination_allowlist: network_policy.allowlist.tcp_port.is_supported(),
        network_policy,
    }
}

fn secret_service_available() -> bool {
    std::thread::spawn(|| {
        let Ok(secret_service) = SecretService::connect(EncryptionType::Dh) else {
            return false;
        };
        let Ok(collection) = secret_service
            .get_default_collection()
            .or_else(|_| secret_service.get_any_collection())
        else {
            return false;
        };
        collection.is_locked().is_ok_and(|locked| !locked)
    })
    .join()
    .unwrap_or(false)
}

pub fn linux_sandbox_helper_path() -> Option<PathBuf> {
    if let Some(helper) = env::var_os(LINUX_SANDBOX_HELPER_ENV) {
        return linux_sandbox_helper_path_from_override(helper);
    }

    let current_exe = env::current_exe().ok()?;
    linux_sandbox_helper_path_from_exe(current_exe)
}

pub fn linux_bwrap_path() -> Option<PathBuf> {
    let path = Path::new(BWRAP_PROGRAM);
    is_safe_bwrap_binary(path).then(|| path.to_path_buf())
}

pub fn is_safe_bwrap_binary(path: &Path) -> bool {
    is_safe_bwrap_binary_at(path, Path::new(BWRAP_PROGRAM))
}

fn is_safe_bwrap_binary_at(path: &Path, expected: &Path) -> bool {
    if path != expected {
        return false;
    }

    let Ok(target_metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(path_metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if path_metadata.file_type().is_symlink() || !target_metadata.file_type().is_file() {
        return false;
    }

    same_file_metadata(&target_metadata, &path_metadata) && target_metadata.uid() == 0
}

fn same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
}

/// Resolve `TA_LINUX_SANDBOX_HELPER` for CI and integration tests where Cargo
/// keeps test binaries under `target/*/deps` but builds helper binaries in
/// `target/*`; the override is accepted only after helper safety validation.
fn linux_sandbox_helper_path_from_override(helper: OsString) -> Option<PathBuf> {
    let helper = PathBuf::from(helper);
    is_safe_linux_sandbox_helper(&helper).then_some(helper)
}

fn is_safe_linux_sandbox_helper(path: &Path) -> bool {
    if path.file_name() != Some(OsStr::new(LINUX_SANDBOX_HELPER)) {
        return false;
    }

    let Ok(target_metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(path_metadata) = fs::symlink_metadata(path) else {
        return false;
    };

    !path_metadata.file_type().is_symlink()
        && target_metadata.file_type().is_file()
        && same_file_metadata(&target_metadata, &path_metadata)
        && target_metadata.mode() & 0o111 != 0
}

fn linux_sandbox_helper_path_from_exe(current_exe: PathBuf) -> Option<PathBuf> {
    let directory = current_exe.parent()?;
    let helper = directory.join(LINUX_SANDBOX_HELPER);
    if is_safe_linux_sandbox_helper(&helper) {
        return Some(helper);
    }

    linux_sandbox_helper_path_from_deps_dir(directory)
}

fn linux_sandbox_helper_path_from_deps_dir(directory: &Path) -> Option<PathBuf> {
    if directory.file_name()? != "deps" {
        return None;
    }

    let helper = directory.parent()?.join(LINUX_SANDBOX_HELPER);
    is_safe_linux_sandbox_helper(&helper).then_some(helper)
}

fn sandbox_kind_for_probe(
    helper_available: bool,
    landlock_fs_available: bool,
    landlock_tcp_available: bool,
    bwrap_available: bool,
) -> SandboxKind {
    if !helper_available {
        return SandboxKind::Unsupported;
    }
    if landlock_fs_available || landlock_tcp_available {
        SandboxKind::LinuxLandlockBwrap
    } else if bwrap_available {
        SandboxKind::LinuxBwrap
    } else {
        SandboxKind::Unsupported
    }
}

fn linux_network_policy_support(
    helper_available: bool,
    landlock_fs_available: bool,
    landlock_tcp_available: bool,
    bwrap_available: bool,
) -> NetworkPolicySupport {
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

    let off = if landlock_tcp_available || bwrap_available {
        SandboxCapability::Supported
    } else {
        SandboxCapability::Unsupported {
            reason: SandboxCapabilityReason::LinuxLandlockTcpUnavailable,
        }
    };
    let open = if landlock_fs_available {
        SandboxCapability::Supported
    } else {
        SandboxCapability::Unsupported {
            reason: SandboxCapabilityReason::LinuxLandlockFilesystemUnavailable,
        }
    };
    let allowlist_tcp_port = if landlock_tcp_available {
        SandboxCapability::Supported
    } else {
        SandboxCapability::Unsupported {
            reason: SandboxCapabilityReason::LinuxLandlockTcpUnavailable,
        }
    };

    NetworkPolicySupport {
        off,
        loopback: SandboxCapability::FailClosed {
            reason: SandboxCapabilityReason::LinuxLoopbackNeedsAddressAwareBackend,
        },
        allowlist: NetworkAllowlistSupport {
            tcp_port: allowlist_tcp_port,
            ip_cidr: SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::LinuxAllowlistRequiresTcpPorts,
            },
            domain_name: SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::LinuxAllowlistRequiresTcpPorts,
            },
        },
        open,
    }
}

fn landlock_filesystem_rules_available() -> bool {
    kernel_release()
        .and_then(|release| kernel_major_minor(&release))
        .is_some_and(|(major, minor)| major > 5 || (major == 5 && minor >= 13))
}

fn landlock_tcp_rules_available() -> bool {
    kernel_release()
        .and_then(|release| kernel_major_minor(&release))
        .is_some_and(|(major, minor)| major > 6 || (major == 6 && minor >= 7))
}

fn kernel_release() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease").ok()
}

fn kernel_major_minor(release: &str) -> Option<(u64, u64)> {
    let mut segments = release.split(['.', '-']);
    let major = segments.next()?.parse().ok()?;
    let minor = segments.next()?.parse().ok()?;
    Some((major, minor))
}

fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(fallback_runtime_dir)
}

fn fallback_runtime_dir() -> PathBuf {
    env::temp_dir().join(format!("taugentic-uid-{}", Uid::current().as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn fallback_runtime_dir_is_user_scoped() {
        let scoped = env::temp_dir().join(format!("taugentic-uid-{}", Uid::current().as_raw()));

        assert_eq!(fallback_runtime_dir(), scoped);
    }

    #[test]
    fn reports_landlock_sandbox_when_helper_and_landlock_tcp_are_available() {
        assert_eq!(
            sandbox_kind_for_probe(true, true, true, false),
            SandboxKind::LinuxLandlockBwrap
        );
        assert_eq!(
            sandbox_kind_for_probe(false, true, true, true),
            SandboxKind::Unsupported
        );
    }

    #[test]
    fn reports_bwrap_sandbox_when_landlock_tcp_probe_fails() {
        assert_eq!(
            sandbox_kind_for_probe(true, false, false, true),
            SandboxKind::LinuxBwrap
        );
        assert_eq!(
            sandbox_kind_for_probe(true, false, false, false),
            SandboxKind::Unsupported
        );
    }

    #[test]
    fn reports_landlock_for_filesystem_only_kernel_support() {
        assert_eq!(
            sandbox_kind_for_probe(true, true, false, false),
            SandboxKind::LinuxLandlockBwrap
        );
    }

    #[test]
    fn derives_linux_sandbox_capabilities_from_backend_support() {
        let network_policy = linux_network_policy_support(true, true, true, false);
        let capabilities = SandboxCapabilities {
            helper_available: true,
            restricted_token_job: false,
            appcontainer: false,
            filesystem_allowlist: true,
            network_default_deny: true,
            network_destination_allowlist: true,
            network_policy,
        };

        assert!(capabilities.filesystem_allowlist);
        assert!(capabilities.network_destination_allowlist);
        assert_eq!(
            capabilities.network_policy.allowlist.tcp_port,
            SandboxCapability::Supported
        );
        assert_eq!(
            capabilities.network_policy.loopback,
            SandboxCapability::FailClosed {
                reason: SandboxCapabilityReason::LinuxLoopbackNeedsAddressAwareBackend,
            }
        );
    }

    #[test]
    fn reports_linux_allowlist_unsupported_without_landlock_tcp() {
        let support = linux_network_policy_support(true, true, false, true);

        assert_eq!(
            support.allowlist.tcp_port,
            SandboxCapability::Unsupported {
                reason: SandboxCapabilityReason::LinuxLandlockTcpUnavailable,
            }
        );
        assert_eq!(support.off, SandboxCapability::Supported);
        assert_eq!(support.open, SandboxCapability::Supported);
    }

    #[test]
    fn reports_secret_service_or_none_for_linux_secrets() {
        assert!(matches!(
            secrets_backend_capability(),
            crate::SecretsBackend::SecretService | crate::SecretsBackend::None
        ));
    }

    #[test]
    fn parses_kernel_release_for_landlock_tcp_support() {
        assert_eq!(kernel_major_minor("6.7.0-31-generic"), Some((6, 7)));
        assert_eq!(kernel_major_minor("5.15.0"), Some((5, 15)));
        assert_eq!(kernel_major_minor("unknown"), None);
    }

    #[test]
    fn bwrap_probe_rejects_wrong_path() {
        let root = unique_probe_root("wrong-bwrap");
        fs::create_dir_all(&root).expect("create probe dir");
        let candidate = root.join("bwrap");
        fs::write(&candidate, "").expect("write candidate");

        assert!(!is_safe_bwrap_binary(&candidate));

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn bwrap_probe_rejects_symlink_candidate() {
        let root = unique_probe_root("symlink-bwrap");
        fs::create_dir_all(&root).expect("create probe dir");
        let target = root.join("real-bwrap");
        let candidate = root.join("bwrap");
        fs::write(&target, "").expect("write target");
        std::os::unix::fs::symlink(&target, &candidate).expect("create symlink");

        assert!(!is_safe_bwrap_binary_at(&candidate, &candidate));

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn probes_helper_next_to_current_exe() {
        let root = unique_probe_root("sibling");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("create bin dir");
        let helper = bin.join(LINUX_SANDBOX_HELPER);
        write_helper(&helper);

        assert_eq!(
            linux_sandbox_helper_path_from_exe(bin.join("test-bin")),
            Some(helper)
        );

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn ignores_parent_directory_helper_for_production_layouts() {
        let root = unique_probe_root("parent-only");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("create bin dir");
        write_helper(&root.join(LINUX_SANDBOX_HELPER));

        assert_eq!(
            linux_sandbox_helper_path_from_exe(bin.join("test-bin")),
            None
        );

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn allows_cargo_deps_parent_fallback_only_for_tests() {
        let root = unique_probe_root("deps");
        let deps = root.join("deps");
        fs::create_dir_all(&deps).expect("create deps dir");
        let helper = root.join(LINUX_SANDBOX_HELPER);
        write_helper(&helper);

        assert_eq!(
            linux_sandbox_helper_path_from_exe(deps.join("test-bin")),
            Some(helper)
        );

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_override_accepts_safe_helper_path() {
        let root = unique_probe_root("override");
        fs::create_dir_all(&root).expect("create probe dir");
        let helper = root.join(LINUX_SANDBOX_HELPER);
        write_helper(&helper);

        assert_eq!(
            linux_sandbox_helper_path_from_override(helper.clone().into_os_string()),
            Some(helper)
        );

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_override_rejects_wrong_helper_name() {
        let root = unique_probe_root("override-wrong-name");
        fs::create_dir_all(&root).expect("create probe dir");
        let candidate = root.join("not-the-helper");
        fs::write(&candidate, "").expect("write candidate");
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755)).expect("chmod");

        assert_eq!(
            linux_sandbox_helper_path_from_override(candidate.into_os_string()),
            None
        );

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_probe_rejects_non_executable_helper() {
        let root = unique_probe_root("non-executable");
        fs::create_dir_all(&root).expect("create probe dir");
        let helper = root.join(LINUX_SANDBOX_HELPER);
        fs::write(&helper, "").expect("write helper");
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o644)).expect("chmod");

        assert!(!is_safe_linux_sandbox_helper(&helper));

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    #[test]
    fn helper_probe_rejects_symlink_candidate() {
        let root = unique_probe_root("symlink-helper");
        fs::create_dir_all(&root).expect("create probe dir");
        let target = root.join("real-helper");
        let candidate = root.join(LINUX_SANDBOX_HELPER);
        fs::write(&target, "").expect("write target");
        std::os::unix::fs::symlink(&target, &candidate).expect("create symlink");

        assert!(!is_safe_linux_sandbox_helper(&candidate));

        fs::remove_dir_all(root).expect("remove probe dir");
    }

    fn write_helper(path: &Path) {
        fs::write(path, "").expect("write helper");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn unique_probe_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "ta-linux-helper-probe-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        root
    }
}
