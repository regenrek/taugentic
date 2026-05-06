mod config;
mod errors;
mod permissions;
mod session;
mod spawn;
mod stream;

use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
};

use crate::error::AcpClientError;

pub use config::{AcpProcessConfig, DEFAULT_CANCEL_GRACE};
pub use permissions::{
    AcpPermissionDecision, AcpPermissionDecisionFuture, AcpPermissionOption,
    AcpPermissionOptionKind, AcpPermissionRequest,
};
pub use session::AcpSessionModelUpdate;
pub use stream::AcpClientEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpClientTrace {
    pub run_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct AcpProcessAdapter {
    config: AcpProcessConfig,
}

impl AcpProcessAdapter {
    pub fn new(config: AcpProcessConfig) -> Self {
        Self { config }
    }

    pub fn spawn(self, trace: AcpClientTrace) -> Result<AcpClient, AcpClientError> {
        let rpc = RpcState::new(&self.config.flavor_id, trace.run_id, trace.session_id);
        let mut child = spawn::spawn_acp_process(&self.config, &rpc.trace)?;
        let writer = child
            .stdin
            .take()
            .ok_or_else(|| AcpClientError::ProcessFailed("ACP stdin was not piped".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AcpClientError::ProcessFailed("ACP stdout was not piped".to_string()))?;
        Ok(AcpClient {
            child,
            writer,
            lines: BufReader::new(stdout).lines(),
            rpc,
            config: self.config,
        })
    }
}

pub struct AcpClient {
    child: Child,
    writer: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    rpc: RpcState,
    config: AcpProcessConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceContext {
    flavor_id: String,
    run_id: String,
    session_id: String,
}

#[derive(Debug)]
struct RpcState {
    next_id: u64,
    trace: TraceContext,
}

impl RpcState {
    fn new(flavor_id: &str, run_id: String, session_id: String) -> Self {
        Self {
            next_id: 1,
            trace: TraceContext {
                flavor_id: flavor_id.to_string(),
                run_id,
                session_id,
            },
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn string_field(update: &serde_json::Value, field: &str) -> Option<String> {
    update
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}
