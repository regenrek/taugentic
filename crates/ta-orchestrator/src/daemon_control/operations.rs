use std::{
    env,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
};

use ta_jsonrpc::JsonRpcClientError;
use ta_protocol::{local_control::RuntimeControlBootstrapCommand, wire::DaemonControlStatusResult};
use thiserror::Error;

const DAEMON_BINARY_ENV_VAR: &str = "TAUGENTIC_DAEMON_BINARY";

/// The internal operation at which desktop runtime startup stopped. This
/// classification is Rust-only; the desktop bridge intentionally reduces all
/// failures to its existing public generic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopRuntimeStartStage {
    Configuration,
    Bootstrap,
}

/// Typed Rust-only provenance for the desktop start path. It deliberately
/// retains no underlying error or values, and must not cross the desktop
/// bridge.
#[derive(Debug)]
pub struct DesktopRuntimeStartError {
    stage: DesktopRuntimeStartStage,
}

impl DesktopRuntimeStartError {
    pub fn stage(&self) -> DesktopRuntimeStartStage {
        self.stage
    }
}

impl std::fmt::Display for DesktopRuntimeStartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("desktop runtime start failed")
    }
}

impl std::error::Error for DesktopRuntimeStartError {}

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
    #[error("runtime-control bootstrap `{action}` failed: {detail}")]
    BootstrapFailed { action: String, detail: String },
    #[error("failed to decode runtime-control bootstrap response: {0}")]
    BootstrapResponse(#[source] serde_json::Error),
}

/// Opaque Rust-only ownership of a desktop-started local runtime. Its identity
/// and Child are intentionally uninspectable outside runtime control;
/// releasing it is the only supported operation.
pub struct DesktopRuntimeHandle {
    local_daemon: Option<super::bootstrap::DesktopLocalDaemon>,
}

impl DesktopRuntimeHandle {
    fn from_local_daemon(local_daemon: Option<super::bootstrap::DesktopLocalDaemon>) -> Self {
        Self { local_daemon }
    }

    pub fn release(&mut self) -> Result<(), DaemonControlOperationError> {
        let Some(local_daemon) = self.local_daemon.as_mut() else {
            return Ok(());
        };
        close_desktop_local_runtime_if_owned(local_daemon)?;
        self.local_daemon = None;
        Ok(())
    }
}

/// Rust-only startup result. The bridge can read the control status and retain
/// or release the opaque handle, but cannot inspect daemon identity or mode.
pub struct DesktopRuntimeStart {
    status: DaemonControlStatusResult,
    handle: DesktopRuntimeHandle,
}

impl DesktopRuntimeStart {
    pub fn control_status(&self) -> &DaemonControlStatusResult {
        &self.status
    }

    pub fn into_handle(self) -> DesktopRuntimeHandle {
        self.handle
    }

    pub fn release(mut self) -> Result<(), DaemonControlOperationError> {
        self.handle.release()
    }
}

pub fn start_desktop_runtime() -> Result<DesktopRuntimeStart, DesktopRuntimeStartError> {
    let config =
        crate::host::config::DaemonConfig::load().map_err(|_| DesktopRuntimeStartError {
            stage: DesktopRuntimeStartStage::Configuration,
        })?;
    let bootstrap = crate::bootstrap_desktop_runtime(&crate::RuntimeControlBootstrapConfig {
        socket_address: config.socket_address().clone(),
        launch_config: config.daemon_control_launch_config(),
        runtime_mode: config.runtime_mode,
    })
    .map_err(|_| DesktopRuntimeStartError {
        stage: DesktopRuntimeStartStage::Bootstrap,
    })?;
    Ok(DesktopRuntimeStart {
        status: bootstrap.status,
        handle: DesktopRuntimeHandle::from_local_daemon(bootstrap.local_daemon),
    })
}

fn close_desktop_local_runtime_if_owned(
    local_daemon: &mut super::bootstrap::DesktopLocalDaemon,
) -> Result<(), DaemonControlOperationError> {
    let config = crate::host::config::DaemonConfig::load().map_err(|error| {
        DaemonControlOperationError::BootstrapFailed {
            action: "desktop-close".to_string(),
            detail: error.to_string(),
        }
    })?;
    let runtime_config = crate::RuntimeControlBootstrapConfig {
        socket_address: config.socket_address().clone(),
        launch_config: config.daemon_control_launch_config(),
        runtime_mode: config.runtime_mode,
    };
    super::handoff::release_desktop_local_runtime_if_matches(
        &local_daemon.daemon_instance_id,
        &mut local_daemon.child,
        &super::handoff::RuntimeControlHandoffConfig {
            socket_address: runtime_config.socket_address.clone(),
            launch_config: runtime_config.launch_config.clone(),
            expected_transition_op_id: None,
            expected_daemon_instance_id: None,
            expected_control_token: None,
        },
        || crate::daemon_control::bootstrap::observe_runtime_control(&runtime_config),
    )
    .map_err(|error| DaemonControlOperationError::BootstrapFailed {
        action: "desktop-close".to_string(),
        detail: error.to_string(),
    })
}

/// Invoke the daemon's protocol-owned runtime-control bootstrap command.
///
/// This is the single client-side control invocation for native clients. It
/// deliberately owns no daemon state machine and does not accept secrets.
pub fn invoke_runtime_control_bootstrap(
    action: RuntimeControlBootstrapCommand,
) -> Result<DaemonControlStatusResult, DaemonControlOperationError> {
    let daemon_binary = resolve_daemon_binary()?;
    let output = Command::new(daemon_binary)
        .arg(RuntimeControlBootstrapCommand::SUBCOMMAND)
        .arg(action.as_str())
        .output()?;
    if !output.status.success() {
        return Err(DaemonControlOperationError::BootstrapFailed {
            action: action.as_str().to_string(),
            detail: command_detail(&output),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(DaemonControlOperationError::BootstrapResponse)
}

fn command_detail(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = format!("{} {}", stderr.trim(), stdout.trim())
        .trim()
        .to_string();
    if detail.is_empty() {
        output
            .status
            .code()
            .map(|code| format!("exit code {code}"))
            .unwrap_or_else(|| "terminated by signal".to_string())
    } else {
        detail
    }
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
    use super::{DesktopRuntimeHandle, daemon_binary_name};

    #[test]
    fn uses_platform_appropriate_daemon_binary_name() {
        if cfg!(windows) {
            assert_eq!(daemon_binary_name(), "ta-daemon.exe");
        } else {
            assert_eq!(daemon_binary_name(), "ta-daemon");
        }
    }

    #[test]
    fn opaque_runtime_handle_without_a_local_child_has_no_release_authority() {
        let mut attached = DesktopRuntimeHandle { local_daemon: None };
        let mut background = DesktopRuntimeHandle { local_daemon: None };

        attached.release().expect("attached runtime release");
        background.release().expect("background runtime release");
    }
}
