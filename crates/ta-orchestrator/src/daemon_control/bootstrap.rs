use std::{
    ffi::OsString,
    fs, io,
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};

use crate::{
    BackgroundServiceControlError, BackgroundServiceState, DaemonActualRuntimeMode,
    RuntimeControlObservedState, RuntimeControlOwnershipRecord, acquire_runtime_control_lock,
    clear_runtime_control_ownership_if_matches, complete_runtime_control_transition,
    daemon_control_status, disable_background_service, enable_background_service,
    host::config::{DaemonControlLaunchConfig, mint_control_token},
    is_daemon_unavailable, process_is_running, read_background_service_state,
    read_persisted_runtime_control_state, read_runtime_control_ownership,
    request_background_disable_handoff, request_background_enable_handoff,
    request_reconcile_handoff, request_stop_handoff, resolve_daemon_binary, spawn_daemon_process,
    terminate_process, write_runtime_control_ownership,
};
use ta_jsonrpc::{ClientConfig, JsonRpcClient, JsonRpcClientError, SocketAddress};
use ta_protocol::local_control::RuntimeControlBootstrapCommand;
use ta_protocol::wire::{
    DaemonControlStatusResult, DaemonRuntimeMode, DaemonStatusParams, DaemonStatusResult,
    METHOD_DAEMON_STATUS,
};
use thiserror::Error;

/// Private bootstrap result. Local identity is converted into an opaque
/// runtime-control handle before it reaches the desktop bridge.
pub(crate) struct DesktopRuntimeBootstrap {
    pub status: DaemonControlStatusResult,
    pub(crate) local_daemon: Option<DesktopLocalDaemon>,
}

/// The one process lease created by desktop bootstrap. It stays entirely in
/// Rust so a desktop close can prove it is acting on the exact child it owns.
pub(crate) struct DesktopLocalDaemon {
    pub(crate) daemon_instance_id: String,
    pub(crate) child: Child,
}

pub const CONTROL_BOOTSTRAP_SUBCOMMAND: &str = RuntimeControlBootstrapCommand::SUBCOMMAND;
const BOOTSTRAP_CLIENT_NAME: &str = "ta-daemon-bootstrap";
const BOOTSTRAP_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const BOOTSTRAP_READY_TIMEOUT: Duration = Duration::from_secs(30);
const BOOTSTRAP_POLL_INTERVAL: Duration = Duration::from_millis(100);
const STALE_LOCAL_PROCESS_TERMINATE_TIMEOUT: Duration = Duration::from_secs(3);

trait BootstrapBackgroundServiceManager {
    fn read_state(&self) -> Result<BackgroundServiceState, BackgroundServiceControlError>;

    fn enable(
        &self,
        program: &Path,
        launch_config: &DaemonControlLaunchConfig,
    ) -> Result<(), BackgroundServiceControlError>;

    fn disable(&self) -> Result<(), BackgroundServiceControlError>;
}

struct SystemBootstrapBackgroundServiceManager;

impl BootstrapBackgroundServiceManager for SystemBootstrapBackgroundServiceManager {
    fn read_state(&self) -> Result<BackgroundServiceState, BackgroundServiceControlError> {
        read_background_service_state()
    }

    fn enable(
        &self,
        program: &Path,
        launch_config: &DaemonControlLaunchConfig,
    ) -> Result<(), BackgroundServiceControlError> {
        enable_background_service(program, launch_config)
    }

    fn disable(&self) -> Result<(), BackgroundServiceControlError> {
        disable_background_service()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeControlBootstrapAction {
    Start,
    Snapshot,
    Reconcile,
    ResetLocal,
    EnableBackground,
    DisableBackground,
    Stop,
}

#[derive(Debug, Clone)]
pub struct RuntimeControlBootstrapConfig {
    pub socket_address: SocketAddress,
    pub launch_config: DaemonControlLaunchConfig,
    pub runtime_mode: DaemonRuntimeMode,
}

#[derive(Debug, Error)]
pub enum RuntimeControlBootstrapError {
    #[error(transparent)]
    Control(#[from] BackgroundServiceControlError),
    #[error(transparent)]
    Handoff(#[from] crate::RuntimeControlHandoffError),
    #[error(transparent)]
    Rpc(#[from] JsonRpcClientError),
    #[error("failed to launch daemon process: {0}")]
    Spawn(#[source] crate::DaemonControlOperationError),
    #[error("failed to resolve daemon binary for background bootstrap: {0}")]
    ResolveBinary(#[source] crate::DaemonControlOperationError),
    #[error(
        "daemon did not become ready in expected {mode:?} mode before bootstrap timeout on {socket}"
    )]
    StartupTimeout {
        mode: DaemonRuntimeMode,
        socket: String,
    },
    #[error("missing runtime-control bootstrap command")]
    MissingCommand,
    #[error("unknown runtime-control bootstrap command: {0}")]
    UnknownCommand(String),
}

pub fn parse_runtime_control_bootstrap_action<I>(
    args: I,
) -> Result<Option<RuntimeControlBootstrapAction>, RuntimeControlBootstrapError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        return Ok(None);
    };
    if command != CONTROL_BOOTSTRAP_SUBCOMMAND {
        return Ok(None);
    }

    let Some(action) = args.next() else {
        return Err(RuntimeControlBootstrapError::MissingCommand);
    };
    match RuntimeControlBootstrapCommand::parse(action.to_string_lossy().as_ref()) {
        Some(RuntimeControlBootstrapCommand::Start) => {
            Ok(Some(RuntimeControlBootstrapAction::Start))
        }
        Some(RuntimeControlBootstrapCommand::Snapshot) => {
            Ok(Some(RuntimeControlBootstrapAction::Snapshot))
        }
        Some(RuntimeControlBootstrapCommand::Reconcile) => {
            Ok(Some(RuntimeControlBootstrapAction::Reconcile))
        }
        Some(RuntimeControlBootstrapCommand::ResetLocal) => {
            Ok(Some(RuntimeControlBootstrapAction::ResetLocal))
        }
        Some(RuntimeControlBootstrapCommand::EnableBackground) => {
            Ok(Some(RuntimeControlBootstrapAction::EnableBackground))
        }
        Some(RuntimeControlBootstrapCommand::DisableBackground) => {
            Ok(Some(RuntimeControlBootstrapAction::DisableBackground))
        }
        Some(RuntimeControlBootstrapCommand::Stop) => Ok(Some(RuntimeControlBootstrapAction::Stop)),
        None => Err(RuntimeControlBootstrapError::UnknownCommand(
            action.to_string_lossy().into_owned(),
        )),
    }
}

pub fn run_runtime_control_bootstrap_action(
    action: RuntimeControlBootstrapAction,
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    match action {
        RuntimeControlBootstrapAction::Start => bootstrap_start_runtime(config),
        RuntimeControlBootstrapAction::Snapshot => snapshot_runtime_control(config),
        RuntimeControlBootstrapAction::Reconcile => bootstrap_reconcile_runtime(config),
        RuntimeControlBootstrapAction::ResetLocal => bootstrap_reset_local_runtime(config),
        RuntimeControlBootstrapAction::EnableBackground => bootstrap_enable_background(config),
        RuntimeControlBootstrapAction::DisableBackground => bootstrap_disable_background(config),
        RuntimeControlBootstrapAction::Stop => bootstrap_stop_runtime(config),
    }
}

fn bootstrap_start_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    let _lock = acquire_runtime_control_lock()?;
    let client = daemon_client(config);

    match client.call::<_, DaemonStatusResult>(METHOD_DAEMON_STATUS, &DaemonStatusParams {}) {
        Ok(status) => return build_control_status(config, Some(status)),
        Err(error) if is_daemon_unavailable(&error) => {}
        Err(error) => return Err(error.into()),
    }

    if config.runtime_mode == DaemonRuntimeMode::Local {
        recover_stale_owned_local_runtime(config)?;
    }

    match config.runtime_mode {
        DaemonRuntimeMode::Local => {
            let _child = bootstrap_local_runtime(config)?;
        }
        DaemonRuntimeMode::Background => bootstrap_background_runtime(config)?,
    }

    let status = wait_for_ready_status(&client, config.runtime_mode)?;
    build_control_status(config, Some(status))
}

/// Start the daemon for one desktop lifecycle and atomically classify the
/// outcome while holding the runtime-control lock. The lease never leaves the
/// Rust desktop boundary; it exists solely to guard a later local stop.
pub(crate) fn bootstrap_desktop_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DesktopRuntimeBootstrap, RuntimeControlBootstrapError> {
    let _lock = acquire_runtime_control_lock()?;
    let client = daemon_client(config);

    match client.call::<_, DaemonStatusResult>(METHOD_DAEMON_STATUS, &DaemonStatusParams {}) {
        Ok(status) => {
            return Ok(DesktopRuntimeBootstrap {
                status: build_control_status(config, Some(status))?,
                local_daemon: None,
            });
        }
        Err(error) if is_daemon_unavailable(&error) => {}
        Err(error) => return Err(error.into()),
    }

    if config.runtime_mode == DaemonRuntimeMode::Local {
        recover_stale_owned_local_runtime(config)?;
        let child = bootstrap_local_runtime(config)?;
        let status = wait_for_ready_status(&client, DaemonRuntimeMode::Local)?;
        let daemon_instance_id = status.daemon_instance_id.clone();
        return Ok(DesktopRuntimeBootstrap {
            status: build_control_status(config, Some(status))?,
            local_daemon: Some(DesktopLocalDaemon {
                daemon_instance_id,
                child,
            }),
        });
    }

    bootstrap_background_runtime(config)?;
    let status = wait_for_ready_status(&client, DaemonRuntimeMode::Background)?;
    Ok(DesktopRuntimeBootstrap {
        status: build_control_status(config, Some(status))?,
        local_daemon: None,
    })
}

fn snapshot_runtime_control(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    let _lock = acquire_runtime_control_lock()?;
    let client = daemon_client(config);
    let status = current_status(&client).ok();
    build_control_status(config, status)
}

fn bootstrap_reconcile_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    request_reconcile_handoff(|| observe_runtime_control(config)).map_err(Into::into)
}

fn bootstrap_reset_local_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    bootstrap_reset_local_runtime_with(config, &SystemBootstrapBackgroundServiceManager)
}

fn bootstrap_reset_local_runtime_with(
    config: &RuntimeControlBootstrapConfig,
    service_manager: &impl BootstrapBackgroundServiceManager,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    let _lock = acquire_runtime_control_lock()?;
    if service_manager.read_state()?.available {
        service_manager.disable()?;
    }
    let _ = clear_runtime_control_ownership_if_matches(None)?;
    complete_runtime_control_transition(DaemonRuntimeMode::Local, false)?;
    let client = daemon_client(config);
    let status = current_status(&client).ok();
    build_control_status_with(config, status, service_manager)
}

fn bootstrap_enable_background(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    request_background_enable_handoff(|| observe_runtime_control(config)).map_err(Into::into)
}

fn bootstrap_disable_background(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    request_background_disable_handoff(|| observe_runtime_control(config)).map_err(Into::into)
}

fn bootstrap_stop_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    let status = build_control_status(config, current_status(&daemon_client(config)).ok())?;
    match status.actual_mode {
        DaemonActualRuntimeMode::Background => {
            request_stop_handoff(DaemonRuntimeMode::Background, || {
                observe_runtime_control(config)
            })
            .map_err(Into::into)
        }
        DaemonActualRuntimeMode::Local => {
            request_stop_handoff(DaemonRuntimeMode::Local, || observe_runtime_control(config))
                .map_err(Into::into)
        }
        DaemonActualRuntimeMode::Stopped => Ok(status),
        DaemonActualRuntimeMode::Foreign => Err(RuntimeControlBootstrapError::Control(
            BackgroundServiceControlError::CommandFailed {
                command: "runtime-control-bootstrap stop",
                detail: "refusing to stop a foreign runtime".to_string(),
            },
        )),
    }
}

fn bootstrap_local_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<Child, RuntimeControlBootstrapError> {
    let control_token = mint_control_token();
    let launch_config = config
        .launch_config
        .with_control_token(Some(control_token.clone()));
    let child = spawn_daemon_process(&launch_config.log_path, &launch_config.environment())
        .map_err(RuntimeControlBootstrapError::Spawn)?;
    let client = daemon_client(config);
    let status = wait_for_ready_status(&client, DaemonRuntimeMode::Local)?;
    write_runtime_control_ownership(&RuntimeControlOwnershipRecord {
        runtime_mode: DaemonRuntimeMode::Local,
        daemon_instance_id: status.daemon_instance_id,
        control_token,
        process_id: Some(child.id()),
    })?;
    Ok(child)
}

fn bootstrap_background_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<(), RuntimeControlBootstrapError> {
    bootstrap_background_runtime_with(config, &SystemBootstrapBackgroundServiceManager)
}

fn bootstrap_background_runtime_with(
    config: &RuntimeControlBootstrapConfig,
    service_manager: &impl BootstrapBackgroundServiceManager,
) -> Result<(), RuntimeControlBootstrapError> {
    let daemon_binary =
        resolve_daemon_binary().map_err(RuntimeControlBootstrapError::ResolveBinary)?;
    let control_token = mint_control_token();
    let launch_config = config
        .launch_config
        .with_control_token(Some(control_token.clone()));
    service_manager.enable(&daemon_binary, &launch_config)?;
    let client = daemon_client(config);
    let status = wait_for_ready_status(&client, DaemonRuntimeMode::Background)?;
    let service_state = service_manager.read_state()?;
    write_runtime_control_ownership(&RuntimeControlOwnershipRecord {
        runtime_mode: DaemonRuntimeMode::Background,
        daemon_instance_id: status.daemon_instance_id,
        control_token,
        process_id: service_state.process_id,
    })?;
    Ok(())
}

fn recover_stale_owned_local_runtime(
    config: &RuntimeControlBootstrapConfig,
) -> Result<(), RuntimeControlBootstrapError> {
    let Some(ownership) = read_runtime_control_ownership()? else {
        return Ok(());
    };
    if ownership.runtime_mode != DaemonRuntimeMode::Local {
        return Ok(());
    }

    if let Some(process_id) = ownership.process_id {
        match process_is_running(process_id) {
            Some(true) => terminate_process(process_id, STALE_LOCAL_PROCESS_TERMINATE_TIMEOUT)?,
            Some(false) => {}
            None => {
                return Err(RuntimeControlBootstrapError::Control(
                    BackgroundServiceControlError::CommandFailed {
                        command: "runtime-control-bootstrap recover-local",
                        detail: format!(
                            "failed to determine liveness for owned local daemon pid {process_id}"
                        ),
                    },
                ));
            }
        }
    }

    remove_owned_local_socket(config)?;
    let _ = clear_runtime_control_ownership_if_matches(Some((
        ownership.daemon_instance_id.as_str(),
        &ownership.control_token,
    )))?;
    Ok(())
}

fn remove_owned_local_socket(
    config: &RuntimeControlBootstrapConfig,
) -> Result<(), RuntimeControlBootstrapError> {
    match &config.socket_address {
        #[cfg(unix)]
        SocketAddress::Unix(path) => remove_owned_local_unix_socket(path),
        #[cfg(not(unix))]
        SocketAddress::Unix(_) => Ok(()),
        SocketAddress::NamedPipe(_) => Ok(()),
    }
}

#[cfg(unix)]
fn remove_owned_local_unix_socket(path: &Path) -> Result<(), RuntimeControlBootstrapError> {
    use std::os::unix::fs::FileTypeExt;

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(RuntimeControlBootstrapError::Control(
                BackgroundServiceControlError::ReadFile {
                    path: path.to_path_buf(),
                    source,
                },
            ));
        }
    };

    if !metadata.file_type().is_socket() {
        return Err(RuntimeControlBootstrapError::Control(
            BackgroundServiceControlError::CommandFailed {
                command: "runtime-control-bootstrap recover-local",
                detail: format!(
                    "refusing to remove non-socket path while recovering local runtime: {}",
                    path.display()
                ),
            },
        ));
    }

    fs::remove_file(path).map_err(|source| {
        RuntimeControlBootstrapError::Control(BackgroundServiceControlError::RemoveFile {
            path: path.to_path_buf(),
            source,
        })
    })
}

fn build_control_status(
    config: &RuntimeControlBootstrapConfig,
    status: Option<DaemonStatusResult>,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    build_control_status_with(config, status, &SystemBootstrapBackgroundServiceManager)
}

fn build_control_status_with(
    config: &RuntimeControlBootstrapConfig,
    status: Option<DaemonStatusResult>,
    service_manager: &impl BootstrapBackgroundServiceManager,
) -> Result<DaemonControlStatusResult, RuntimeControlBootstrapError> {
    let observed = observed_runtime_control_state_with(config, status, service_manager)?;
    let control_plane = read_persisted_runtime_control_state()?;
    Ok(daemon_control_status(&control_plane, &observed))
}

fn observed_runtime_control_state_with(
    config: &RuntimeControlBootstrapConfig,
    status: Option<DaemonStatusResult>,
    service_manager: &impl BootstrapBackgroundServiceManager,
) -> Result<RuntimeControlObservedState, RuntimeControlBootstrapError> {
    let ownership = read_runtime_control_ownership()?;
    let background_service = service_manager.read_state()?;
    let socket_path = status
        .as_ref()
        .map(|status| status.socket_path.clone())
        .unwrap_or_else(|| config.socket_address.to_string());
    Ok(RuntimeControlObservedState {
        daemon_status: status.clone(),
        background_service,
        ownership,
        socket_path,
        log_path: config.launch_config.log_path.display().to_string(),
        daemon_version: status.map(|status| status.version),
    })
}

pub(crate) fn observe_runtime_control(
    config: &RuntimeControlBootstrapConfig,
) -> Result<RuntimeControlObservedState, BackgroundServiceControlError> {
    let client = daemon_client(config);
    let status =
        match client.call::<_, DaemonStatusResult>(METHOD_DAEMON_STATUS, &DaemonStatusParams {}) {
            Ok(status) => Some(status),
            Err(error) if is_daemon_unavailable(&error) => None,
            Err(error) => {
                return Err(BackgroundServiceControlError::CommandFailed {
                    command: "daemon.status",
                    detail: error.to_string(),
                });
            }
        };
    let ownership = read_runtime_control_ownership()?;
    let background_service = read_background_service_state()?;
    let socket_path = status
        .as_ref()
        .map(|status| status.socket_path.clone())
        .unwrap_or_else(|| config.socket_address.to_string());
    Ok(RuntimeControlObservedState {
        daemon_status: status.clone(),
        background_service,
        ownership,
        socket_path,
        log_path: config.launch_config.log_path.display().to_string(),
        daemon_version: status.map(|status| status.version),
    })
}

fn daemon_client(config: &RuntimeControlBootstrapConfig) -> JsonRpcClient {
    JsonRpcClient::new(ClientConfig {
        service_name: BOOTSTRAP_CLIENT_NAME.to_string(),
        socket_address: config.socket_address.clone(),
        io_timeout: BOOTSTRAP_REQUEST_TIMEOUT,
    })
}

fn current_status(
    client: &JsonRpcClient,
) -> Result<DaemonStatusResult, RuntimeControlBootstrapError> {
    Ok(client.call(METHOD_DAEMON_STATUS, &DaemonStatusParams {})?)
}

fn wait_for_ready_status(
    client: &JsonRpcClient,
    expected_mode: DaemonRuntimeMode,
) -> Result<DaemonStatusResult, RuntimeControlBootstrapError> {
    let deadline = Instant::now() + BOOTSTRAP_READY_TIMEOUT;
    loop {
        match classify_ready_status_attempt(
            current_status(client),
            expected_mode,
            Instant::now() < deadline,
            &client.config().socket_address.to_string(),
        )? {
            Some(status) => return Ok(status),
            None => thread::sleep(BOOTSTRAP_POLL_INTERVAL),
        }
    }
}

fn classify_ready_status_attempt(
    attempt: Result<DaemonStatusResult, RuntimeControlBootstrapError>,
    expected_mode: DaemonRuntimeMode,
    before_deadline: bool,
    socket: &str,
) -> Result<Option<DaemonStatusResult>, RuntimeControlBootstrapError> {
    match attempt {
        Ok(status) if status.ready && status.runtime_mode == expected_mode => Ok(Some(status)),
        Ok(_) if before_deadline => Ok(None),
        Ok(_) => Err(RuntimeControlBootstrapError::StartupTimeout {
            mode: expected_mode,
            socket: socket.to_string(),
        }),
        Err(RuntimeControlBootstrapError::Rpc(error)) if is_daemon_unavailable(&error) => {
            if before_deadline {
                Ok(None)
            } else {
                Err(RuntimeControlBootstrapError::StartupTimeout {
                    mode: expected_mode,
                    socket: socket.to_string(),
                })
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, path::Path, process::Command, time::Duration};
    use ta_protocol::wire::DaemonTransitionStatus;

    struct FakeBootstrapBackgroundServiceManager {
        state: BackgroundServiceState,
        disable_calls: Cell<u32>,
    }

    impl BootstrapBackgroundServiceManager for FakeBootstrapBackgroundServiceManager {
        fn read_state(&self) -> Result<BackgroundServiceState, BackgroundServiceControlError> {
            Ok(self.state.clone())
        }

        fn enable(
            &self,
            _program: &Path,
            _launch_config: &DaemonControlLaunchConfig,
        ) -> Result<(), BackgroundServiceControlError> {
            panic!("reset-local bootstrap test must not enable background service");
        }

        fn disable(&self) -> Result<(), BackgroundServiceControlError> {
            self.disable_calls.set(self.disable_calls.get() + 1);
            Ok(())
        }
    }

    #[test]
    fn unavailable_socket_at_readiness_deadline_is_startup_timeout() {
        let error = classify_ready_status_attempt(
            Err(RuntimeControlBootstrapError::Rpc(
                JsonRpcClientError::ConnectionClosed,
            )),
            DaemonRuntimeMode::Local,
            false,
            "test-control-endpoint",
        )
        .expect_err("unavailable socket at the deadline should be a startup timeout");

        assert!(matches!(
            error,
            RuntimeControlBootstrapError::StartupTimeout {
                mode: DaemonRuntimeMode::Local,
                socket,
            } if socket == "test-control-endpoint"
        ));
    }

    #[test]
    fn reconcile_bootstrap_returns_snapshot_without_pending_transition_or_reentrant_lock() {
        crate::host::config::with_test_config_home("bootstrap-reconcile-no-reentrant-lock", || {
            let config = crate::host::config::test_config();
            let status = run_runtime_control_bootstrap_action(
                RuntimeControlBootstrapAction::Reconcile,
                &RuntimeControlBootstrapConfig {
                    socket_address: config.socket_address().clone(),
                    launch_config: config.daemon_control_launch_config(),
                    runtime_mode: config.runtime_mode,
                },
            )
            .expect("reconcile bootstrap should return a control snapshot");

            assert_eq!(status.transition_status, DaemonTransitionStatus::Idle);
            assert!(!status.reconcile_required);
            assert!(!status.allowed_actions.is_empty());
        });
    }

    #[test]
    fn reset_local_bootstrap_clears_background_desire_and_pending_transition() {
        crate::host::config::with_test_config_home("bootstrap-reset-local", || {
            let config = crate::host::config::test_config();
            crate::start_runtime_control_transition(
                crate::PendingTransitionKind::EnableBackground,
                DaemonRuntimeMode::Background,
                true,
                crate::TransitionStep::StopConflictingRuntime,
            )
            .expect("background transition should persist");
            crate::write_runtime_control_ownership(&crate::RuntimeControlOwnershipRecord {
                runtime_mode: DaemonRuntimeMode::Background,
                daemon_instance_id: "foreign-daemon".to_string(),
                control_token: crate::host::config::mint_control_token(),
                process_id: Some(123),
            })
            .expect("ownership should persist");

            let service_manager = FakeBootstrapBackgroundServiceManager {
                state: BackgroundServiceState {
                    available: true,
                    enabled: true,
                    loaded: false,
                    running: false,
                    process_id: None,
                    service_name: Some("taugentic-daemon.service".to_string()),
                },
                disable_calls: Cell::new(0),
            };
            let status = bootstrap_reset_local_runtime_with(
                &RuntimeControlBootstrapConfig {
                    socket_address: config.socket_address().clone(),
                    launch_config: config.daemon_control_launch_config(),
                    runtime_mode: config.runtime_mode,
                },
                &service_manager,
            )
            .expect("reset-local bootstrap should return a local control snapshot");

            assert_eq!(status.desired_mode, DaemonRuntimeMode::Local);
            assert_eq!(status.actual_mode, DaemonActualRuntimeMode::Stopped);
            assert_eq!(status.transition_status, DaemonTransitionStatus::Idle);
            assert!(!status.background_opt_in);
            assert!(!status.reconcile_required);
            assert!(status.pending_transition.is_none());
            assert_eq!(service_manager.disable_calls.get(), 1);
            assert_eq!(
                crate::read_persisted_runtime_control_state()
                    .expect("control plane should read")
                    .desired_mode,
                DaemonRuntimeMode::Local
            );
            assert!(
                crate::read_runtime_control_ownership()
                    .expect("ownership should read")
                    .is_none()
            );
        });
    }

    #[test]
    #[cfg(unix)]
    fn recover_stale_owned_local_runtime_terminates_pid_and_clears_socket_and_ownership() {
        crate::host::config::with_test_config_home("bootstrap-recover-stale-local", || {
            use std::os::unix::net::UnixListener;

            let config = crate::host::config::test_config();
            let socket_path = match config.socket_address() {
                SocketAddress::Unix(path) => path.clone(),
                SocketAddress::NamedPipe(_) => panic!("unix test expected unix socket"),
            };
            if let Some(parent) = socket_path.parent() {
                fs::create_dir_all(parent).expect("socket parent should exist");
            }
            let listener =
                UnixListener::bind(&socket_path).expect("test should create a unix socket file");
            drop(listener);

            let mut child = Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("sleep should spawn");
            let ownership = crate::RuntimeControlOwnershipRecord {
                runtime_mode: DaemonRuntimeMode::Local,
                daemon_instance_id: "stale-owned-local".to_string(),
                control_token: crate::host::config::mint_control_token(),
                process_id: Some(child.id()),
            };
            crate::write_runtime_control_ownership(&ownership).expect("ownership should persist");

            recover_stale_owned_local_runtime(&RuntimeControlBootstrapConfig {
                socket_address: config.socket_address().clone(),
                launch_config: config.daemon_control_launch_config(),
                runtime_mode: config.runtime_mode,
            })
            .expect("stale local runtime should recover");

            let _ = child.wait();
            let process_id = ownership.process_id.expect("test should have pid");
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while std::time::Instant::now() < deadline {
                if matches!(process_is_running(process_id), Some(false)) {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }

            assert_eq!(process_is_running(process_id), Some(false));
            assert!(
                crate::read_runtime_control_ownership()
                    .expect("ownership should read")
                    .is_none()
            );
            assert!(!socket_path.exists());
        });
    }
}
