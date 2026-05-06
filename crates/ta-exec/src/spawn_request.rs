use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use ta_sandbox::SandboxProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioPolicy {
    Null,
    Inherit,
    Piped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessGroupPolicy {
    Inherit,
    New,
}

#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) env_remove: Vec<OsString>,
    pub(crate) env_clear: bool,
    pub(crate) stdin: StdioPolicy,
    pub(crate) stdout: StdioPolicy,
    pub(crate) stderr: StdioPolicy,
    pub(crate) kill_on_drop: bool,
    pub(crate) process_group: ProcessGroupPolicy,
    pub(crate) sandbox_profile: Option<SandboxProfile>,
}

impl SpawnRequest {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            env_remove: Vec::new(),
            env_clear: false,
            stdin: StdioPolicy::Inherit,
            stdout: StdioPolicy::Inherit,
            stderr: StdioPolicy::Inherit,
            kill_on_drop: true,
            process_group: ProcessGroupPolicy::Inherit,
            sandbox_profile: None,
        }
    }

    pub fn shell(command: impl Into<OsString>, cwd: impl Into<PathBuf>) -> Self {
        let (program, args) = platform_shell_command(command.into());
        Self::new(program)
            .args(args)
            .cwd(cwd)
            .stdin(StdioPolicy::Null)
            .stdout(StdioPolicy::Piped)
            .stderr(StdioPolicy::Piped)
            .process_group(platform_shell_process_group())
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Adds an explicit child environment entry.
    ///
    /// With a sandbox profile, parent env starts cleared and only the profile
    /// allowlist is rehydrated; explicit entries are the sole caller-controlled
    /// secret bridge into the child.
    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    /// Removes environment entries after inherited/allowlisted and explicit env
    /// have been applied.
    pub fn env_remove<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.env_remove.extend(names.into_iter().map(Into::into));
        self
    }

    pub fn env_clear(mut self, env_clear: bool) -> Self {
        self.env_clear = env_clear;
        self
    }

    pub fn stdin(mut self, policy: StdioPolicy) -> Self {
        self.stdin = policy;
        self
    }

    pub fn stdout(mut self, policy: StdioPolicy) -> Self {
        self.stdout = policy;
        self
    }

    pub fn stderr(mut self, policy: StdioPolicy) -> Self {
        self.stderr = policy;
        self
    }

    pub fn kill_on_drop(mut self, kill_on_drop: bool) -> Self {
        self.kill_on_drop = kill_on_drop;
        self
    }

    pub fn process_group(mut self, policy: ProcessGroupPolicy) -> Self {
        self.process_group = policy;
        self
    }

    pub fn sandbox_profile(mut self, profile: SandboxProfile) -> Self {
        self.sandbox_profile = Some(profile);
        self
    }

    pub fn program(&self) -> &OsStr {
        &self.program
    }

    pub fn sandbox_profile_ref(&self) -> Option<&SandboxProfile> {
        self.sandbox_profile.as_ref()
    }
}

#[cfg(unix)]
fn platform_shell_command(command: OsString) -> (OsString, Vec<OsString>) {
    ("/bin/sh".into(), vec!["-c".into(), command])
}

#[cfg(not(unix))]
fn platform_shell_command(command: OsString) -> (OsString, Vec<OsString>) {
    // Windows sandbox backend support is a later slice; keep the native shell default unchanged.
    ("cmd".into(), vec!["/C".into(), command])
}

#[cfg(unix)]
fn platform_shell_process_group() -> ProcessGroupPolicy {
    ProcessGroupPolicy::New
}

#[cfg(not(unix))]
fn platform_shell_process_group() -> ProcessGroupPolicy {
    ProcessGroupPolicy::Inherit
}
