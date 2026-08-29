use thiserror::Error;

use crate::spawn_request::ProcessGroupPolicy;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("failed to spawn process: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("failed to wait for process: {0}")]
    Wait(#[source] std::io::Error),
    #[error("PTY operation failed: {0}")]
    Pty(String),
    #[error("failed to signal process: {0}")]
    Signal(String),
    #[error("process group policy {0:?} is unsupported on this platform")]
    UnsupportedProcessGroup(ProcessGroupPolicy),
    #[error("sandbox preparation failed: {0}")]
    Sandbox(#[from] ta_sandbox::SandboxError),
}
