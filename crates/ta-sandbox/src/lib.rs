//! Canonical sandbox profiles and backend preparation for Taugentic execution.
//!
//! This crate owns platform-neutral sandbox policy and platform backend selection.
//! Exec callers pass a `SandboxProfile`; backend modules decide how that profile
//! becomes a platform command wrapper.

mod profile;

pub mod backend;
pub mod windows;

use std::ffi::OsString;

use thiserror::Error;

pub use profile::{NetworkPolicy, SandboxProfile};
pub use ta_host_platform::SandboxKind;

pub const LINUX_SANDBOX_CALLER_ENV_PRESENT_ARG: &str = "--caller-env-present";
pub const WINDOWS_SANDBOX_PROFILE_ARG: &str = "--profile-json";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
    #[error("sandbox backend {kind} is unsupported: {reason}")]
    Unsupported {
        kind: SandboxKind,
        reason: &'static str,
    },
    #[error("invalid sandbox profile: {0}")]
    InvalidProfile(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCommand {
    program: OsString,
    args: Vec<OsString>,
}

impl SandboxCommand {
    pub fn new(program: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
        }
    }

    pub fn program(&self) -> &OsString {
        &self.program
    }

    pub fn args(&self) -> &[OsString] {
        &self.args
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSandboxCommand {
    program: OsString,
    args: Vec<OsString>,
    kind: SandboxKind,
}

impl PreparedSandboxCommand {
    pub fn new(kind: SandboxKind, program: impl Into<OsString>, args: Vec<OsString>) -> Self {
        Self {
            program: program.into(),
            args,
            kind,
        }
    }

    pub fn into_parts(self) -> (OsString, Vec<OsString>) {
        (self.program, self.args)
    }

    pub fn kind(&self) -> SandboxKind {
        self.kind
    }
}

pub trait SandboxBackend: Send + Sync {
    fn kind(&self) -> SandboxKind;

    fn prepare(
        &self,
        profile: &SandboxProfile,
        command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError>;
}

pub fn current_backend() -> Box<dyn SandboxBackend> {
    backend::current_backend()
}

pub fn prepare_current(
    profile: &SandboxProfile,
    command: SandboxCommand,
) -> Result<PreparedSandboxCommand, SandboxError> {
    current_backend().prepare(profile, command)
}
