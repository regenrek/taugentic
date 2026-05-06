use std::{ffi::OsString, path::PathBuf};

use ta_host_platform::{SandboxKind, linux_sandbox_helper_path};

use crate::{PreparedSandboxCommand, SandboxBackend, SandboxCommand, SandboxError, SandboxProfile};

const PROFILE_ARG: &str = "--profile-json";
const ARG_SEPARATOR: &str = "--";

#[derive(Debug, Clone, Copy)]
pub struct LandlockBackend;

impl SandboxBackend for LandlockBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::LinuxLandlockBwrap
    }

    fn prepare(
        &self,
        profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        let helper = linux_sandbox_helper_path().ok_or(SandboxError::Unsupported {
            kind: self.kind(),
            reason: "Linux sandbox helper binary is missing next to the current executable",
        })?;
        prepare_with_helper_path(helper, profile, command)
    }
}

/// Wraps the command for the Linux helper; the helper owns absolute-program validation.
fn prepare_with_helper_path(
    helper: PathBuf,
    profile: &SandboxProfile,
    command: SandboxCommand,
) -> Result<PreparedSandboxCommand, SandboxError> {
    let profile_json = serde_json::to_string(profile).map_err(|error| {
        SandboxError::InvalidProfile(format!(
            "Linux sandbox profile must be JSON-serializable: {error}"
        ))
    })?;
    let mut args = vec![
        OsString::from(PROFILE_ARG),
        OsString::from(profile_json),
        OsString::from(ARG_SEPARATOR),
        command.program().clone(),
    ];
    args.extend(command.args().iter().cloned());

    Ok(PreparedSandboxCommand::new(
        SandboxKind::LinuxLandlockBwrap,
        helper.into_os_string(),
        args,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkPolicy, SandboxCommand, SandboxProfile};

    #[test]
    fn wraps_command_with_linux_helper_and_profile_json() {
        let helper = PathBuf::from("/opt/taugentic/ta-linux-sandbox");
        let profile = SandboxProfile::new()
            .read_path("/repo")
            .write_path("/repo/target")
            .network(NetworkPolicy::Off);
        let command = SandboxCommand::new("/bin/true", vec!["--ignored".into()]);

        let prepared =
            prepare_with_helper_path(helper.clone(), &profile, command).expect("prepared");
        let (program, args) = prepared.into_parts();

        assert_eq!(program, helper.into_os_string());
        assert_eq!(args[0], OsString::from(PROFILE_ARG));
        assert!(args[1].to_string_lossy().contains("/repo"));
        assert_eq!(args[2], OsString::from(ARG_SEPARATOR));
        assert_eq!(args[3], OsString::from("/bin/true"));
        assert_eq!(args[4], OsString::from("--ignored"));
    }

    #[test]
    fn passes_network_open_to_linux_helper() {
        let helper = PathBuf::from("/opt/taugentic/ta-linux-sandbox");
        let profile = SandboxProfile::new().network(NetworkPolicy::Open);
        let command = SandboxCommand::new("/bin/true", Vec::new());

        let prepared = prepare_with_helper_path(helper, &profile, command).expect("prepared");
        let (_program, args) = prepared.into_parts();
        let profile_json = args[1].to_string_lossy();
        let profile: SandboxProfile = serde_json::from_str(&profile_json).expect("profile json");

        assert_eq!(profile.network_policy(), &NetworkPolicy::Open);
    }

    #[test]
    fn passes_loopback_and_allowlist_to_linux_helper() {
        let helper = PathBuf::from("/opt/taugentic/ta-linux-sandbox");
        for profile in [
            SandboxProfile::new().network(NetworkPolicy::Loopback),
            SandboxProfile::new().network(NetworkPolicy::Allowlist(vec!["443".to_string()])),
        ] {
            let command = SandboxCommand::new("/bin/true", Vec::new());
            let prepared =
                prepare_with_helper_path(helper.clone(), &profile, command).expect("prepared");
            let (_program, args) = prepared.into_parts();
            let profile_json = args[1].to_string_lossy();
            let parsed: SandboxProfile = serde_json::from_str(&profile_json).expect("profile json");

            assert_eq!(parsed.network_policy(), profile.network_policy());
        }
    }

    #[test]
    fn preserves_program_path_for_helper_boundary_validation() {
        let helper = PathBuf::from("/opt/taugentic/ta-linux-sandbox");
        let profile = SandboxProfile::new().network(NetworkPolicy::Off);
        let command = SandboxCommand::new("true", Vec::new());

        let prepared = prepare_with_helper_path(helper, &profile, command).expect("prepared");
        let (_program, args) = prepared.into_parts();

        assert_eq!(args[3], OsString::from("true"));
    }
}
