use ta_host_platform::SandboxKind;

use crate::{PreparedSandboxCommand, SandboxBackend, SandboxCommand, SandboxError, SandboxProfile};

#[derive(Debug, Clone, Copy)]
pub struct UnsupportedSandboxBackend {
    kind: SandboxKind,
    reason: &'static str,
}

impl UnsupportedSandboxBackend {
    pub fn new(kind: SandboxKind, reason: &'static str) -> Self {
        Self { kind, reason }
    }
}

impl SandboxBackend for UnsupportedSandboxBackend {
    fn kind(&self) -> SandboxKind {
        self.kind
    }

    fn prepare(
        &self,
        _profile: &SandboxProfile,
        _command: SandboxCommand,
    ) -> Result<PreparedSandboxCommand, SandboxError> {
        Err(SandboxError::Unsupported {
            kind: self.kind,
            reason: self.reason,
        })
    }
}
