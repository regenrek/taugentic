use std::path::Path;

use ta_host_platform::SandboxKind;

use crate::{
    NetworkPolicy, PreparedSandboxCommand, SandboxBackend, SandboxCommand, SandboxError,
    SandboxProfile,
};

const SANDBOX_EXEC_PATH: &str = "/usr/bin/sandbox-exec";
const MACOS_LOOPBACK_UNSUPPORTED: &str =
    "macOS Seatbelt cannot enforce loopback-only network policy";
const MACOS_ALLOWLIST_UNSUPPORTED: &str =
    "macOS Seatbelt cannot enforce destination-aware network allowlists";

#[derive(Debug, Clone, Copy)]
pub struct SeatbeltBackend;

impl SandboxBackend for SeatbeltBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::MacosSeatbelt
    }

    fn prepare(
        &self,
        profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        let sandbox_exec = sandbox_exec_path()?;
        let policy = seatbelt_profile(profile)?;
        let mut args = vec!["-p".into(), policy.into(), command.program().clone()];
        args.extend(command.args().iter().cloned());

        Ok(PreparedSandboxCommand::new(
            SandboxKind::MacosSeatbelt,
            sandbox_exec.as_os_str(),
            args,
        ))
    }
}

fn sandbox_exec_path() -> Result<&'static Path, SandboxError> {
    let path = Path::new(SANDBOX_EXEC_PATH);
    if path.is_file() {
        Ok(path)
    } else {
        Err(SandboxError::Unsupported {
            kind: SandboxKind::MacosSeatbelt,
            reason: "macOS sandbox wrapper is missing at /usr/bin/sandbox-exec",
        })
    }
}

fn seatbelt_profile(profile: &SandboxProfile) -> Result<String, SandboxError> {
    let mut rules = vec![
        "(version 1)".to_string(),
        "(deny default)".to_string(),
        "(allow process*)".to_string(),
        "(allow sysctl-read)".to_string(),
        "(allow file-read-metadata)".to_string(),
        platform_runtime_reads(),
        network_rule(profile.network_policy())?,
    ];

    if profile.child_inherits_tty_enabled() {
        rules.push(r#"(allow file-read* file-write* (literal "/dev/tty"))"#.to_string());
    }

    for path in profile.fs_read_paths() {
        rules.push(format!(
            r#"(allow file-read* (subpath "{}"))"#,
            escape_path(path)
        ));
    }
    for path in profile.fs_write_paths() {
        let path = escape_path(path);
        rules.push(format!(
            r#"(allow file-read* file-write* (subpath "{path}"))"#
        ));
    }

    Ok(rules.join("\n"))
}

fn platform_runtime_reads() -> String {
    let mut paths = vec![r#"(literal "/")"#.to_string()];
    paths.extend(
        [
            "/bin",
            "/sbin",
            "/usr",
            "/System",
            "/Library/Apple",
            "/private/var/db",
        ]
        .into_iter()
        .map(|path| format!(r#"(subpath "{path}")"#)),
    );
    paths
        .join(" ")
        .pipe(|paths| format!("(allow file-read* {paths})"))
}

fn network_rule(policy: &NetworkPolicy) -> Result<String, SandboxError> {
    match policy {
        NetworkPolicy::Off => Ok("(deny network*)".to_string()),
        NetworkPolicy::Open => Ok("(allow network*)".to_string()),
        NetworkPolicy::Loopback => Err(SandboxError::InvalidProfile(
            MACOS_LOOPBACK_UNSUPPORTED.to_string(),
        )),
        NetworkPolicy::Allowlist(_) => Err(SandboxError::InvalidProfile(
            MACOS_ALLOWLIST_UNSUPPORTED.to_string(),
        )),
    }
}

fn escape_path(path: &Path) -> String {
    let mut escaped = String::new();
    for character in path.to_string_lossy().chars() {
        match character {
            '\\' => escaped.push_str(r"\\"),
            '"' => escaped.push_str("\\\""),
            other => escaped.push(other),
        }
    }
    escaped
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn wraps_command_with_sandbox_exec_policy() {
        let profile = SandboxProfile::new()
            .read_path("/repo")
            .network(NetworkPolicy::Off);
        let command = SandboxCommand::new("sh", vec!["-c".into(), "echo ok".into()]);

        let prepared = SeatbeltBackend
            .prepare(&profile, command)
            .expect("prepared");
        let (program, args) = prepared.into_parts();

        assert_eq!(program, OsString::from(SANDBOX_EXEC_PATH));
        assert_eq!(args[0], OsString::from("-p"));
        assert!(args[1].to_string_lossy().contains(r#"(literal "/")"#));
        assert!(args[1].to_string_lossy().contains(r#"(subpath "/repo")"#));
        assert_eq!(args[2], OsString::from("sh"));
    }

    #[test]
    fn rejects_network_allowlist_until_backend_supports_it() {
        let profile =
            SandboxProfile::new().network(NetworkPolicy::Allowlist(vec!["localhost".to_string()]));
        let command = SandboxCommand::new("sh", Vec::new());

        let error = SeatbeltBackend
            .prepare(&profile, command)
            .expect_err("invalid");

        assert!(matches!(error, SandboxError::InvalidProfile(_)));
    }

    #[test]
    fn rejects_loopback_when_seatbelt_cannot_enforce_destination() {
        let profile = SandboxProfile::new().network(NetworkPolicy::Loopback);
        let error = network_rule(profile.network_policy()).expect_err("loopback rejected");

        assert!(matches!(
            error,
            SandboxError::InvalidProfile(message)
                if message.contains("loopback-only") && message.contains("Seatbelt")
        ));
    }
}
