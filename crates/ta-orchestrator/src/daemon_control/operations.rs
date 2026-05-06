use std::{
    env,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use ta_jsonrpc::JsonRpcClientError;
use thiserror::Error;

const DAEMON_BINARY_ENV_VAR: &str = "TAUGENTIC_DAEMON_BINARY";

#[derive(Debug, Error)]
pub enum DaemonControlOperationError {
    #[error("failed to access local filesystem: {0}")]
    Io(#[from] io::Error),
    #[error("failed to determine the current ta binary location: {0}")]
    CurrentExecutable(#[source] io::Error),
    #[error("unable to locate ta-daemon next to ta or in PATH")]
    DaemonBinaryNotFound,
    #[error("daemon log file not found at {path}")]
    DaemonLogMissing { path: PathBuf },
}

pub fn resolve_daemon_binary() -> Result<PathBuf, DaemonControlOperationError> {
    if let Some(override_path) = env::var_os(DAEMON_BINARY_ENV_VAR) {
        let override_path = PathBuf::from(override_path);
        if override_path.is_file() {
            return Ok(override_path);
        }
    }

    let current_exe = env::current_exe().map_err(DaemonControlOperationError::CurrentExecutable)?;
    let binary_name = daemon_binary_name();

    if let Some(parent) = current_exe.parent() {
        let sibling = parent.join(binary_name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }

    if let Some(path_var) = env::var_os("PATH") {
        for directory in env::split_paths(&path_var) {
            let candidate = directory.join(binary_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(DaemonControlOperationError::DaemonBinaryNotFound)
}

pub fn daemon_binary_name() -> &'static str {
    if cfg!(windows) {
        "ta-daemon.exe"
    } else {
        "ta-daemon"
    }
}

pub fn spawn_daemon_process(
    log_path: &Path,
    launch_environment: &[(String, String)],
) -> Result<Child, DaemonControlOperationError> {
    let binary_path = resolve_daemon_binary()?;
    spawn_daemon_process_with_binary(log_path, &binary_path, launch_environment)
}

pub fn is_daemon_unavailable(error: &JsonRpcClientError) -> bool {
    matches!(
        error,
        JsonRpcClientError::Socket(_)
            | JsonRpcClientError::Read(_)
            | JsonRpcClientError::Write(_)
            | JsonRpcClientError::Flush(_)
            | JsonRpcClientError::ConnectionClosed
            | JsonRpcClientError::ResponseTimeout { .. }
    )
}

fn spawn_daemon_process_with_binary(
    log_path: &Path,
    binary_path: &Path,
    launch_environment: &[(String, String)],
) -> Result<Child, DaemonControlOperationError> {
    let log_dir = log_path
        .parent()
        .ok_or_else(|| DaemonControlOperationError::DaemonLogMissing {
            path: log_path.to_path_buf(),
        })?
        .to_path_buf();
    fs::create_dir_all(&log_dir)?;

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let log_file_stderr = log_file.try_clone()?;

    let mut command = Command::new(binary_path);
    command
        .envs(launch_environment.iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_stderr));

    command.spawn().map_err(DaemonControlOperationError::Io)
}

#[cfg(test)]
mod tests {
    use super::daemon_binary_name;

    #[test]
    fn uses_platform_appropriate_daemon_binary_name() {
        if cfg!(windows) {
            assert_eq!(daemon_binary_name(), "ta-daemon.exe");
        } else {
            assert_eq!(daemon_binary_name(), "ta-daemon");
        }
    }
}
