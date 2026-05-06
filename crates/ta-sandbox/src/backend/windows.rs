use std::{ffi::OsString, path::PathBuf};

use ta_host_platform::{SandboxKind, windows_sandbox_helper_path};

use crate::{
    PreparedSandboxCommand, SandboxBackend, SandboxCommand, SandboxError, SandboxProfile,
    WINDOWS_SANDBOX_PROFILE_ARG, windows::validate_windows_appcontainer_profile,
};

const ARG_SEPARATOR: &str = "--";

#[derive(Debug, Clone, Copy)]
pub struct AppContainerBackend;

impl SandboxBackend for AppContainerBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::WindowsAppContainer
    }

    fn prepare(
        &self,
        profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        validate_windows_appcontainer_profile(profile)
            .map_err(|error| SandboxError::InvalidProfile(error.to_string()))?;
        let helper = windows_sandbox_helper_path().ok_or(SandboxError::Unsupported {
            kind: self.kind(),
            reason: "Windows sandbox helper binary is missing next to the current executable",
        })?;
        prepare_with_helper_path(helper, profile, command)
    }
}

fn prepare_with_helper_path(
    helper: PathBuf,
    profile: &SandboxProfile,
    command: SandboxCommand,
) -> Result<PreparedSandboxCommand, SandboxError> {
    let profile_json = serde_json::to_string(profile).map_err(|error| {
        SandboxError::InvalidProfile(format!(
            "Windows sandbox profile must be JSON-serializable: {error}"
        ))
    })?;
    let mut args = vec![
        OsString::from(WINDOWS_SANDBOX_PROFILE_ARG),
        OsString::from(profile_json),
        OsString::from(ARG_SEPARATOR),
        command.program().clone(),
    ];
    args.extend(command.args().iter().cloned());

    Ok(PreparedSandboxCommand::new(
        SandboxKind::WindowsAppContainer,
        helper.into_os_string(),
        args,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkPolicy, SandboxCommand, SandboxProfile};

    #[test]
    fn wraps_command_with_windows_helper_and_profile_json() {
        let helper = PathBuf::from(r"C:\Program Files\Taugentic\ta-windows-sandbox.exe");
        let profile = SandboxProfile::new().network(NetworkPolicy::Off);
        let command = SandboxCommand::new(
            r"C:\Windows\System32\cmd.exe",
            vec![OsString::from("/c"), OsString::from("echo ok")],
        );

        let prepared =
            prepare_with_helper_path(helper.clone(), &profile, command).expect("prepared");
        let (program, args) = prepared.into_parts();

        assert_eq!(program, helper.into_os_string());
        assert_eq!(args[0], OsString::from(WINDOWS_SANDBOX_PROFILE_ARG));
        assert_eq!(args[2], OsString::from(ARG_SEPARATOR));
        assert_eq!(args[3], OsString::from(r"C:\Windows\System32\cmd.exe"));
        assert_eq!(args[4], OsString::from("/c"));
        assert_eq!(args[5], OsString::from("echo ok"));
    }

    #[test]
    fn accepts_filesystem_paths_for_acl_backend() {
        let with_paths = SandboxProfile::new().read_path(r"C:\repo");

        assert!(validate_windows_appcontainer_profile(&with_paths).is_ok());
    }

    #[test]
    fn accepts_appcontainer_network_modes() {
        for policy in [
            NetworkPolicy::Off,
            NetworkPolicy::Loopback,
            NetworkPolicy::Allowlist(vec!["443".to_string()]),
            NetworkPolicy::Open,
        ] {
            let profile = SandboxProfile::new().network(policy);

            assert!(validate_windows_appcontainer_profile(&profile).is_ok());
        }
    }
}
