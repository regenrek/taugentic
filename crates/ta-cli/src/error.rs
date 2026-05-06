use std::{path::PathBuf, process::ExitStatus, time::Duration};

use ta_jsonrpc::JsonRpcClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Parse(#[from] clap::Error),
    #[error(transparent)]
    Daemon(#[from] JsonRpcClientError),
    #[error("failed to access local filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to serialize command output: {0}")]
    SerializeOutput(#[source] serde_json::Error),
    #[error("failed to serialize control response: {0}")]
    SerializeControlResponse(#[source] serde_json::Error),
    #[error("failed to deserialize control request: {0}")]
    DeserializeControlRequest(#[source] serde_json::Error),
    #[error("invalid control protocol message: {0}")]
    ControlProtocol(String),
    #[error("failed to determine the current ta binary location: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("unable to locate ta-daemon next to ta or in PATH")]
    DaemonBinaryNotFound,
    #[error(
        "daemon did not become ready within {timeout:?} on socket {socket}; expected logs at {log_path}"
    )]
    DaemonStartupTimeout {
        timeout: Duration,
        socket: String,
        log_path: PathBuf,
    },
    #[error("daemon did not stop within {timeout:?} on socket {socket}")]
    DaemonShutdownTimeout { timeout: Duration, socket: String },
    #[error("daemon exited early with status {status}; {details}")]
    DaemonExitedEarly { status: ExitStatus, details: String },
    #[error("daemon log file not found at {path}")]
    DaemonLogMissing { path: PathBuf },
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Parse(error) => error.exit_code(),
            Self::Daemon(_)
            | Self::Io(_)
            | Self::SerializeOutput(_)
            | Self::SerializeControlResponse(_)
            | Self::DeserializeControlRequest(_)
            | Self::ControlProtocol(_)
            | Self::CurrentExecutable(_)
            | Self::DaemonBinaryNotFound
            | Self::DaemonStartupTimeout { .. }
            | Self::DaemonShutdownTimeout { .. }
            | Self::DaemonExitedEarly { .. }
            | Self::DaemonLogMissing { .. }
            | Self::InvalidInput(_) => 1,
        }
    }

    pub fn report(&self) {
        match self {
            Self::Parse(error) => {
                let _ = error.print();
            }
            Self::Daemon(_)
            | Self::Io(_)
            | Self::SerializeOutput(_)
            | Self::SerializeControlResponse(_)
            | Self::DeserializeControlRequest(_)
            | Self::ControlProtocol(_)
            | Self::CurrentExecutable(_)
            | Self::DaemonBinaryNotFound
            | Self::DaemonStartupTimeout { .. }
            | Self::DaemonShutdownTimeout { .. }
            | Self::DaemonExitedEarly { .. }
            | Self::DaemonLogMissing { .. }
            | Self::InvalidInput(_) => {
                eprintln!("{self}");
            }
        }
    }
}
