//! Canonical process spawning primitives for Taugentic-owned execution.
//!
//! This first slice is intentionally consumed by the native shell tool only.
//! Sandbox backends plus ACP, Codex App Server, and MCP stdio process migration
//! are follow-up slices on the same PlanDB parent.

mod error;
mod local_engine;
mod pty;
mod spawn_request;

use tokio::process::Child;

pub use error::ExecError;
pub use local_engine::{LocalExecEngine, terminate_child, terminate_child_tree};
pub use pty::{LocalPtyEngine, PtyRequest, PtySession, PtySize};
pub use spawn_request::{ProcessGroupPolicy, SpawnRequest, StdioPolicy};
pub use ta_sandbox::{NetworkPolicy, SandboxError, SandboxKind, SandboxProfile};

pub trait ExecEngine: Send + Sync {
    fn spawn(&self, request: SpawnRequest) -> Result<Child, ExecError>;
}

#[cfg(test)]
mod tests;
