use std::{
    fs::{self},
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use ta_daemon_client::DaemonClient;
use ta_jsonrpc::{JsonRpcClientError, SocketAddress};
use ta_orchestrator::{daemon_log_path_for_current_env, invoke_runtime_control_bootstrap};
use ta_protocol::local_control::RuntimeControlBootstrapCommand;
use ta_protocol::wire::{
    DaemonActualRuntimeMode, DaemonControlStatusResult, DaemonStatusResult, DaemonStopResult,
};

use crate::error::CliError;
use crate::output::DaemonStatusPoll;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonLogsResult {
    pub path: String,
    pub contents: String,
    pub lines: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonWaitResult {
    pub ready: bool,
    pub socket_path: String,
    pub log_path: String,
    pub version: String,
    pub waited_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStartResult {
    pub started: bool,
    pub already_running: bool,
    pub pid: Option<u32>,
    pub socket_path: String,
    pub log_path: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonRestartResult {
    pub restarted: bool,
    pub was_running: bool,
    pub pid: Option<u32>,
    pub socket_path: String,
    pub log_path: String,
    pub version: String,
}

pub fn start_daemon(
    daemon_client: &DaemonClient,
    timeout: Duration,
    interval: Duration,
) -> Result<DaemonStartResult, CliError> {
    start_daemon_inner(daemon_client, timeout, interval)
}

fn start_daemon_inner(
    daemon_client: &DaemonClient,
    timeout: Duration,
    interval: Duration,
) -> Result<DaemonStartResult, CliError> {
    match daemon_client.status() {
        Ok(status) if status.ready => {
            return Ok(DaemonStartResult {
                started: false,
                already_running: true,
                pid: None,
                socket_path: status.socket_path,
                log_path: status.log_path,
                version: status.version,
            });
        }
        Ok(status) => {
            let log_path = PathBuf::from(status.log_path.clone());
            let status = wait_for_daemon_status(
                || daemon_client.status(),
                &daemon_client.socket_display_string(),
                timeout,
                interval,
                &log_path,
            )?;
            return Ok(DaemonStartResult {
                started: false,
                already_running: true,
                pid: None,
                socket_path: status.socket_path,
                log_path: status.log_path,
                version: status.version,
            });
        }
        Err(error) if is_daemon_unavailable(&error) => {}
        Err(error) => return Err(CliError::Daemon(error)),
    }

    let status = run_bootstrap_subcommand(RuntimeControlBootstrapCommand::Start)?;
    let version = status.daemon_version.clone().ok_or_else(|| {
        CliError::ControlProtocol("bootstrap start returned no daemon version".to_string())
    })?;

    Ok(DaemonStartResult {
        started: true,
        already_running: false,
        pid: None,
        socket_path: status.socket_path,
        log_path: status.log_path,
        version,
    })
}

pub fn wait_for_daemon(
    daemon_client: &DaemonClient,
    timeout: Duration,
    interval: Duration,
) -> Result<DaemonWaitResult, CliError> {
    let log_path = daemon_log_path(daemon_client)?;
    let started_at = Instant::now();
    let status = wait_for_daemon_status(
        || daemon_client.status(),
        &daemon_client.socket_display_string(),
        timeout,
        interval,
        &log_path,
    )?;

    Ok(DaemonWaitResult {
        ready: status.ready,
        socket_path: status.socket_path,
        log_path: status.log_path,
        version: status.version,
        waited_ms: elapsed_ms(started_at.elapsed()),
    })
}

pub fn read_daemon_status(daemon_client: &DaemonClient) -> Result<DaemonStatusResult, CliError> {
    daemon_client.status().map_err(CliError::Daemon)
}

pub fn poll_daemon_status(daemon_client: &DaemonClient) -> DaemonStatusPoll {
    match daemon_client.status() {
        Ok(status) => DaemonStatusPoll::Reachable { status },
        Err(error) => {
            let error_message = error.to_string();
            if is_daemon_unavailable(&error) {
                DaemonStatusPoll::Unavailable {
                    socket_path: daemon_client.socket_display_string(),
                    log_path: daemon_log_path_from_status(
                        daemon_client.socket_address(),
                        Err(error),
                    )
                    .ok()
                    .map(|path| path.display().to_string()),
                    error: error_message,
                }
            } else {
                DaemonStatusPoll::Error {
                    socket_path: daemon_client.socket_display_string(),
                    error: error_message,
                }
            }
        }
    }
}

pub fn watch_daemon_status<F>(
    daemon_client: &DaemonClient,
    interval: Duration,
    count: Option<u64>,
    mut emit: F,
) -> Result<(), CliError>
where
    F: FnMut(DaemonStatusPoll) -> Result<(), CliError>,
{
    let mut emitted = 0_u64;

    loop {
        emit(poll_daemon_status(daemon_client))?;
        emitted += 1;

        if count.is_some_and(|target| emitted >= target) {
            return Ok(());
        }

        thread::sleep(interval);
    }
}

pub fn restart_daemon(
    daemon_client: &DaemonClient,
    timeout: Duration,
    interval: Duration,
) -> Result<DaemonRestartResult, CliError> {
    // Restart is intentionally reachable-only. Offline recovery stays an explicit stop-only path.
    let was_running = !matches!(
        read_background_status(daemon_client)?.actual_mode,
        DaemonActualRuntimeMode::Stopped
    );
    if was_running {
        stop_configured_daemon(daemon_client, timeout, interval)?;
    }

    let started = start_daemon_inner(daemon_client, timeout, interval)?;
    Ok(DaemonRestartResult {
        restarted: true,
        was_running,
        pid: started.pid,
        socket_path: started.socket_path,
        log_path: started.log_path,
        version: started.version,
    })
}

pub fn read_logs(daemon_client: &DaemonClient, tail: usize) -> Result<DaemonLogsResult, CliError> {
    let log_path = daemon_log_path(daemon_client)?;
    read_daemon_log_tail(&log_path, tail)
}

pub fn read_background_status(
    daemon_client: &DaemonClient,
) -> Result<DaemonControlStatusResult, CliError> {
    daemon_client.control_status().map_err(CliError::Daemon)
}

pub fn reconcile_runtime_control(
    daemon_client: &DaemonClient,
    _timeout: Duration,
    _interval: Duration,
) -> Result<DaemonControlStatusResult, CliError> {
    let _ = daemon_client;
    run_bootstrap_subcommand(RuntimeControlBootstrapCommand::Reconcile)
}

pub fn enable_background_mode(
    daemon_client: &DaemonClient,
    _timeout: Duration,
    _interval: Duration,
) -> Result<DaemonControlStatusResult, CliError> {
    let _ = daemon_client;
    run_bootstrap_subcommand(RuntimeControlBootstrapCommand::EnableBackground)
}

pub fn disable_background_mode(
    daemon_client: &DaemonClient,
    _timeout: Duration,
    _interval: Duration,
) -> Result<DaemonControlStatusResult, CliError> {
    let _ = daemon_client;
    run_bootstrap_subcommand(RuntimeControlBootstrapCommand::DisableBackground)
}

pub fn stop_configured_daemon(
    daemon_client: &DaemonClient,
    timeout: Duration,
    interval: Duration,
) -> Result<DaemonStopResult, CliError> {
    match run_bootstrap_subcommand(RuntimeControlBootstrapCommand::Stop) {
        Ok(status) => {
            if status.actual_mode == DaemonActualRuntimeMode::Stopped {
                return Ok(DaemonStopResult { stopping: false });
            }
            wait_for_daemon_shutdown(
                || daemon_client.status(),
                &daemon_client.socket_display_string(),
                timeout,
                interval,
            )?;
            Ok(DaemonStopResult { stopping: true })
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn is_daemon_unavailable(error: &JsonRpcClientError) -> bool {
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

pub(crate) fn daemon_log_path_from_status(
    socket_address: &SocketAddress,
    status: Result<DaemonStatusResult, JsonRpcClientError>,
) -> Result<PathBuf, CliError> {
    match status {
        Ok(status) => Ok(PathBuf::from(status.log_path)),
        Err(error) if is_daemon_unavailable(&error) => {
            Ok(daemon_log_path_for_current_env(socket_address))
        }
        Err(error) => Err(CliError::Daemon(error)),
    }
}

fn daemon_log_path(daemon_client: &DaemonClient) -> Result<PathBuf, CliError> {
    daemon_log_path_from_status(daemon_client.socket_address(), daemon_client.status())
}

fn run_bootstrap_subcommand(
    action: RuntimeControlBootstrapCommand,
) -> Result<DaemonControlStatusResult, CliError> {
    invoke_runtime_control_bootstrap(action).map_err(map_daemon_binary_error)
}

fn read_daemon_log_tail(log_path: &Path, tail: usize) -> Result<DaemonLogsResult, CliError> {
    let contents = match fs::read_to_string(log_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CliError::DaemonLogMissing {
                path: log_path.to_path_buf(),
            });
        }
        Err(error) => return Err(CliError::Io(error)),
    };
    let (contents, lines, truncated) = tail_lines(&contents, tail);

    Ok(DaemonLogsResult {
        path: log_path.display().to_string(),
        contents,
        lines,
        truncated,
    })
}

fn wait_for_daemon_status<F>(
    mut read_status: F,
    socket_display: &str,
    timeout: Duration,
    interval: Duration,
    log_path: &Path,
) -> Result<DaemonStatusResult, CliError>
where
    F: FnMut() -> Result<DaemonStatusResult, JsonRpcClientError>,
{
    let deadline = Instant::now() + timeout;
    loop {
        match read_status() {
            Ok(status) if status.ready => return Ok(status),
            Ok(_) if Instant::now() < deadline => thread::sleep(interval),
            Ok(_) => {
                return Err(CliError::DaemonStartupTimeout {
                    timeout,
                    socket: socket_display.to_string(),
                    log_path: log_path.to_path_buf(),
                });
            }
            Err(error) if is_daemon_unavailable(&error) && Instant::now() < deadline => {
                thread::sleep(interval);
            }
            Err(error) if is_daemon_unavailable(&error) => {
                return Err(CliError::DaemonStartupTimeout {
                    timeout,
                    socket: socket_display.to_string(),
                    log_path: log_path.to_path_buf(),
                });
            }
            Err(error) => return Err(CliError::Daemon(error)),
        }
    }
}

fn wait_for_daemon_shutdown<F>(
    mut read_status: F,
    socket_display: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<(), CliError>
where
    F: FnMut() -> Result<DaemonStatusResult, JsonRpcClientError>,
{
    let deadline = Instant::now() + timeout;
    loop {
        match read_status() {
            Ok(_) if Instant::now() < deadline => thread::sleep(interval),
            Ok(_) => {
                return Err(CliError::DaemonShutdownTimeout {
                    timeout,
                    socket: socket_display.to_string(),
                });
            }
            Err(error) if is_daemon_unavailable(&error) => return Ok(()),
            Err(error) => return Err(CliError::Daemon(error)),
        }
    }
}

fn tail_lines(contents: &str, tail: usize) -> (String, usize, bool) {
    let lines: Vec<&str> = contents.lines().collect();
    if lines.is_empty() {
        return (String::new(), 0, false);
    }

    if tail == 0 || lines.len() <= tail {
        return (lines.join("\n"), lines.len(), false);
    }

    let start = lines.len() - tail;
    (lines[start..].join("\n"), tail, true)
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn map_daemon_binary_error(error: ta_orchestrator::DaemonControlOperationError) -> CliError {
    match error {
        ta_orchestrator::DaemonControlOperationError::Io(source) => CliError::Io(source),
        ta_orchestrator::DaemonControlOperationError::CurrentExecutable(source) => {
            CliError::CurrentExecutable(source)
        }
        ta_orchestrator::DaemonControlOperationError::DaemonBinaryNotFound => {
            CliError::DaemonBinaryNotFound
        }
        ta_orchestrator::DaemonControlOperationError::DaemonLogMissing { path } => {
            CliError::DaemonLogMissing { path }
        }
        ta_orchestrator::DaemonControlOperationError::BootstrapFailed { action, detail } => {
            CliError::ControlProtocol(format!("bootstrap subcommand `{action}` failed: {detail}"))
        }
        ta_orchestrator::DaemonControlOperationError::BootstrapResponse(source) => {
            CliError::DeserializeControlRequest(source)
        }
    }
}
