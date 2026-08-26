use std::{
    ffi::OsString,
    io,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use crate::host::config::{
    ControlToken, DaemonControlLaunchConfig, RuntimeControlHandoffExpectation, mint_control_token,
};
use crate::{
    BackgroundServiceControlError, PendingTransitionKind, PersistedRuntimeControlState,
    RuntimeControlObservedState, RuntimeControlOwnershipRecord, TransitionStep,
    acquire_runtime_control_lock, advance_runtime_control_transition, clear_runtime_control_error,
    clear_runtime_control_ownership_if_matches, complete_runtime_control_transition,
    daemon_control_status, disable_background_service, enable_background_service,
    fail_runtime_control_transition,
    host::internal_stop::{
        InternalDaemonStopParams, InternalDaemonStopResult, METHOD_DAEMON_INTERNAL_STOP,
    },
    is_daemon_unavailable, read_background_service_state, read_persisted_runtime_control_state,
    resolve_daemon_binary, start_runtime_control_transition, stop_background_service,
    write_runtime_control_ownership,
};
use ta_jsonrpc::{
    ClientConfig, JsonRpcClient, JsonRpcClientError, PersistentJsonRpcClient, SocketAddress,
};
use ta_protocol::local_control::RuntimeControlHandoffCommand;
use ta_protocol::wire::{
    DaemonClientCapabilities, DaemonControlErrorCode, DaemonControlStatusResult,
    DaemonInitializeParams, DaemonInitializeResult, DaemonRuntimeMode, DaemonStatusParams,
    DaemonStatusResult, METHOD_DAEMON_INITIALIZE, METHOD_DAEMON_STATUS,
};
use thiserror::Error;

pub const CONTROL_HANDOFF_SUBCOMMAND: &str = RuntimeControlHandoffCommand::SUBCOMMAND;
pub(crate) const HANDOFF_CLIENT_NAME: &str = "ta-daemon-handoff";
const HANDOFF_FLUSH_GRACE: Duration = Duration::from_millis(200);
const HANDOFF_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const HANDOFF_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const HANDOFF_POLL_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const HANDOFF_EXPECTED_OP_ID_ENV_VAR: &str = "TAUGENTIC_RUNTIME_CONTROL_EXPECTED_OP_ID";
pub(crate) const HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR: &str =
    "TAUGENTIC_RUNTIME_CONTROL_EXPECTED_DAEMON_INSTANCE_ID";
pub(crate) const HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR: &str =
    "TAUGENTIC_RUNTIME_CONTROL_EXPECTED_CONTROL_TOKEN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeControlHandoffAction {
    EnableBackground,
    DisableBackground,
    StopLocalRuntime,
    StopBackgroundRuntime,
}

#[derive(Debug, Clone)]
pub struct RuntimeControlHandoffConfig {
    pub socket_address: SocketAddress,
    pub launch_config: DaemonControlLaunchConfig,
    pub expected_transition_op_id: Option<u64>,
    pub expected_daemon_instance_id: Option<String>,
    pub expected_control_token: Option<ControlToken>,
}

#[derive(Debug, Error)]
pub enum RuntimeControlHandoffError {
    #[error(transparent)]
    Control(#[from] BackgroundServiceControlError),
    #[error(transparent)]
    Rpc(#[from] JsonRpcClientError),
    #[error("daemon control action is not allowed in current state: {0}")]
    ActionNotAllowed(&'static str),
    #[error("failed to resolve current daemon executable: {0}")]
    CurrentExecutable(#[source] io::Error),
    #[error("failed to spawn daemon runtime-control handoff helper: {0}")]
    Spawn(#[source] io::Error),
    #[error("failed to resolve daemon binary for background handoff: {0}")]
    ResolveBinary(#[source] crate::DaemonControlOperationError),
    #[error("missing runtime-control handoff command")]
    MissingCommand,
    #[error("unknown runtime-control handoff command: {0}")]
    UnknownCommand(String),
    #[error("background daemon did not stop before handoff timeout on {socket}")]
    ShutdownTimeout { socket: String },
    #[error(
        "background daemon did not become ready in background mode before handoff timeout on {socket}"
    )]
    BackgroundStartTimeout { socket: String },
    #[error("runtime-control handoff is missing expected transition op id")]
    MissingExpectedTransitionOpId,
    #[error("runtime-control handoff is missing expected ownership identity")]
    MissingExpectedOwnership,
    #[error("failed to wait for the desktop-owned local daemon")]
    ChildWait(#[source] io::Error),
}

pub fn parse_runtime_control_handoff_action<I>(
    args: I,
) -> Result<Option<RuntimeControlHandoffAction>, RuntimeControlHandoffError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        return Ok(None);
    };
    if command != CONTROL_HANDOFF_SUBCOMMAND {
        return Ok(None);
    }

    let Some(action) = args.next() else {
        return Err(RuntimeControlHandoffError::MissingCommand);
    };
    match RuntimeControlHandoffCommand::parse(action.to_string_lossy().as_ref()) {
        Some(RuntimeControlHandoffCommand::EnableBackground) => {
            Ok(Some(RuntimeControlHandoffAction::EnableBackground))
        }
        Some(RuntimeControlHandoffCommand::DisableBackground) => {
            Ok(Some(RuntimeControlHandoffAction::DisableBackground))
        }
        Some(RuntimeControlHandoffCommand::StopLocalRuntime) => {
            Ok(Some(RuntimeControlHandoffAction::StopLocalRuntime))
        }
        Some(RuntimeControlHandoffCommand::StopBackgroundRuntime) => {
            Ok(Some(RuntimeControlHandoffAction::StopBackgroundRuntime))
        }
        None => Err(RuntimeControlHandoffError::UnknownCommand(
            action.to_string_lossy().into_owned(),
        )),
    }
}

pub fn run_runtime_control_handoff_action(
    action: RuntimeControlHandoffAction,
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    match action {
        RuntimeControlHandoffAction::EnableBackground => run_enable_background_handoff(config),
        RuntimeControlHandoffAction::DisableBackground => run_disable_background_handoff(config),
        RuntimeControlHandoffAction::StopLocalRuntime => run_stop_local_runtime_handoff(config),
        RuntimeControlHandoffAction::StopBackgroundRuntime => {
            run_stop_background_runtime_handoff(config)
        }
    }
}

pub fn request_background_disable_handoff<F>(
    observe_state: F,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    F: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
{
    request_background_disable_handoff_with(observe_state, spawn_disable_background_handoff)
}

pub fn request_background_enable_handoff<F>(
    observe_state: F,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    F: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
{
    request_background_enable_handoff_with(observe_state, spawn_enable_background_handoff)
}

pub fn request_reconcile_handoff<F>(
    mut observe_state: F,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    F: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
{
    let _lock = acquire_runtime_control_lock()?;
    let mut control_plane = read_persisted_runtime_control_state()?;
    if control_plane.last_error.is_some() {
        clear_runtime_control_error()?;
        control_plane = read_persisted_runtime_control_state()?;
    }
    let observed = observe_state()?;
    let Some(pending) = control_plane.pending_transition.clone() else {
        return Ok(daemon_control_status(&control_plane, &observed));
    };

    match pending.kind {
        PendingTransitionKind::EnableBackground => {
            reconcile_enable_handoff(&observed)?;
        }
        PendingTransitionKind::DisableBackground => {
            reconcile_disable_handoff(&observed)?;
        }
        PendingTransitionKind::RecoverToLocal => {
            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)?;
        }
    }

    let control_plane = read_persisted_runtime_control_state()?;
    let observed = observe_state()?;
    Ok(daemon_control_status(&control_plane, &observed))
}

pub fn request_stop_handoff<F>(
    runtime_mode: DaemonRuntimeMode,
    observe_state: F,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    F: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
{
    request_stop_handoff_with(
        runtime_mode,
        observe_state,
        spawn_stop_local_runtime_handoff,
        spawn_stop_background_runtime_handoff,
    )
}

/// Release the exact daemon process created by desktop bootstrap. This is not
/// a handoff: the desktop still owns the Child, so it can synchronously stop
/// and reap that one process while the runtime-control lock keeps replacement
/// ownership from racing the decision.
pub(super) fn release_desktop_local_runtime_if_matches<F>(
    expected_daemon_instance_id: &str,
    child: &mut Child,
    config: &RuntimeControlHandoffConfig,
    mut observe_state: F,
) -> Result<(), RuntimeControlHandoffError>
where
    F: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
{
    let _lock = acquire_runtime_control_lock()?;
    let observed = observe_state()?;
    let Some(ownership) =
        desktop_local_stop_ownership(&observed, expected_daemon_instance_id, child.id())
    else {
        return Ok(());
    };
    let authenticated_config = RuntimeControlHandoffConfig {
        socket_address: config.socket_address.clone(),
        launch_config: config
            .launch_config
            .with_control_token(Some(ownership.control_token.clone())),
        expected_transition_op_id: None,
        expected_daemon_instance_id: Some(ownership.daemon_instance_id.clone()),
        expected_control_token: Some(ownership.control_token.clone()),
    };
    stop_current_daemon_via_control_token(&authenticated_config)?;
    child
        .wait()
        .map_err(RuntimeControlHandoffError::ChildWait)?;
    clear_expected_runtime_control_ownership(&authenticated_config)?;
    Ok(())
}

fn desktop_local_stop_ownership<'a>(
    observed: &'a RuntimeControlObservedState,
    expected_daemon_instance_id: &str,
    child_process_id: u32,
) -> Option<&'a RuntimeControlOwnershipRecord> {
    let status = observed.daemon_status.as_ref()?;
    let ownership = observed.ownership.as_ref()?;
    (ownership.runtime_mode == DaemonRuntimeMode::Local
        && status.daemon_instance_id == expected_daemon_instance_id
        && ownership.daemon_instance_id == expected_daemon_instance_id
        && ownership.process_id == Some(child_process_id))
    .then_some(ownership)
}

fn request_stop_handoff_with<FObserve, FLocal, FBackground>(
    runtime_mode: DaemonRuntimeMode,
    mut observe_state: FObserve,
    spawn_local_handoff: FLocal,
    spawn_background_handoff: FBackground,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    FObserve: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
    FLocal: FnOnce((&str, &str)) -> Result<(), RuntimeControlHandoffError>,
    FBackground: FnOnce((&str, &str)) -> Result<(), RuntimeControlHandoffError>,
{
    let _lock = acquire_runtime_control_lock()?;
    let observed = observe_state()?;

    match runtime_mode {
        DaemonRuntimeMode::Local => {
            ensure_local_stop_request_allowed(&observed)?;
            let ownership = observed
                .ownership
                .as_ref()
                .expect("validated local stop should have ownership");
            spawn_local_handoff((
                ownership.daemon_instance_id.as_str(),
                ownership.control_token.as_str(),
            ))?;
        }
        DaemonRuntimeMode::Background => {
            ensure_background_stop_request_allowed(&observed)?;
            let ownership = observed
                .ownership
                .as_ref()
                .expect("validated background stop should have ownership");
            spawn_background_handoff((
                ownership.daemon_instance_id.as_str(),
                ownership.control_token.as_str(),
            ))?;
        }
    }

    let control_plane = read_persisted_runtime_control_state()?;
    let observed = observe_state()?;
    Ok(daemon_control_status(&control_plane, &observed))
}

fn request_background_disable_handoff_with<FObserve, FSpawn>(
    mut observe_state: FObserve,
    spawn_handoff: FSpawn,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    FObserve: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
    FSpawn: FnOnce(u64, Option<(&str, &str)>) -> Result<(), RuntimeControlHandoffError>,
{
    let _lock = acquire_runtime_control_lock()?;
    let control_plane = read_persisted_runtime_control_state()?;
    ensure_disable_request_allowed(&control_plane)?;
    let observed = observe_state()?;

    let transition = start_runtime_control_transition(
        PendingTransitionKind::DisableBackground,
        DaemonRuntimeMode::Local,
        false,
        TransitionStep::StopBackgroundService,
    )?;
    let expected_op_id = transition
        .pending_transition
        .as_ref()
        .map(|pending| pending.op_id)
        .expect("started transition should have pending op id");
    let expected_ownership = observed.ownership.as_ref().map(|ownership| {
        (
            ownership.daemon_instance_id.as_str(),
            ownership.control_token.as_str(),
        )
    });
    spawn_handoff(expected_op_id, expected_ownership).inspect_err(|error| {
        record_transition_failure(error);
    })?;

    let control_plane = read_persisted_runtime_control_state()?;
    let observed = observe_state()?;
    Ok(daemon_control_status(&control_plane, &observed))
}

fn request_background_enable_handoff_with<FObserve, FSpawn>(
    mut observe_state: FObserve,
    spawn_handoff: FSpawn,
) -> Result<DaemonControlStatusResult, RuntimeControlHandoffError>
where
    FObserve: FnMut() -> Result<RuntimeControlObservedState, BackgroundServiceControlError>,
    FSpawn: FnOnce(u64) -> Result<(), RuntimeControlHandoffError>,
{
    let _lock = acquire_runtime_control_lock()?;
    let control_plane = read_persisted_runtime_control_state()?;
    let observed = observe_state()?;
    ensure_enable_request_allowed(&control_plane, &observed)?;

    let transition = start_runtime_control_transition(
        PendingTransitionKind::EnableBackground,
        DaemonRuntimeMode::Background,
        true,
        TransitionStep::Prepare,
    )?;
    let expected_op_id = transition
        .pending_transition
        .as_ref()
        .map(|pending| pending.op_id)
        .expect("started transition should have pending op id");
    spawn_handoff(expected_op_id).inspect_err(|error| {
        record_transition_failure(error);
    })?;

    let control_plane = read_persisted_runtime_control_state()?;
    let observed = observe_state()?;
    Ok(daemon_control_status(&control_plane, &observed))
}

fn spawn_disable_background_handoff(
    expected_op_id: u64,
    expected_ownership: Option<(&str, &str)>,
) -> Result<(), RuntimeControlHandoffError> {
    spawn_handoff_subcommand(
        RuntimeControlHandoffCommand::DisableBackground.as_str(),
        RuntimeControlHandoffExpectation::from_values(
            Some(expected_op_id.to_string()),
            expected_ownership.map(|ownership| ownership.0.to_string()),
            expected_ownership.map(|ownership| ownership.1.to_string()),
        ),
    )
}

fn spawn_enable_background_handoff(expected_op_id: u64) -> Result<(), RuntimeControlHandoffError> {
    spawn_handoff_subcommand(
        RuntimeControlHandoffCommand::EnableBackground.as_str(),
        RuntimeControlHandoffExpectation::from_values(Some(expected_op_id.to_string()), None, None),
    )
}

fn spawn_stop_local_runtime_handoff(
    expected_ownership: (&str, &str),
) -> Result<(), RuntimeControlHandoffError> {
    spawn_handoff_subcommand(
        RuntimeControlHandoffCommand::StopLocalRuntime.as_str(),
        RuntimeControlHandoffExpectation::from_values(
            None,
            Some(expected_ownership.0.to_string()),
            Some(expected_ownership.1.to_string()),
        ),
    )
}

fn spawn_stop_background_runtime_handoff(
    expected_ownership: (&str, &str),
) -> Result<(), RuntimeControlHandoffError> {
    spawn_handoff_subcommand(
        RuntimeControlHandoffCommand::StopBackgroundRuntime.as_str(),
        RuntimeControlHandoffExpectation::from_values(
            None,
            Some(expected_ownership.0.to_string()),
            Some(expected_ownership.1.to_string()),
        ),
    )
}

fn spawn_handoff_subcommand(
    command: &str,
    expectation: RuntimeControlHandoffExpectation,
) -> Result<(), RuntimeControlHandoffError> {
    let current_exe =
        std::env::current_exe().map_err(RuntimeControlHandoffError::CurrentExecutable)?;
    let mut process = Command::new(current_exe);
    process.arg(CONTROL_HANDOFF_SUBCOMMAND).arg(command);
    for (key, value) in expectation.environment() {
        process.env(key, value);
    }
    process
        .spawn()
        .map(|_| ())
        .map_err(RuntimeControlHandoffError::Spawn)
}

fn ensure_disable_request_allowed(
    control_plane: &PersistedRuntimeControlState,
) -> Result<(), RuntimeControlHandoffError> {
    if control_plane.pending_transition.is_some() || control_plane.last_error.is_some() {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "disable-background while reconcile is required",
        ));
    }
    if !control_plane.background_opt_in {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "disable-background",
        ));
    }
    Ok(())
}

fn ensure_enable_request_allowed(
    control_plane: &PersistedRuntimeControlState,
    observed: &RuntimeControlObservedState,
) -> Result<(), RuntimeControlHandoffError> {
    if control_plane.pending_transition.is_some() || control_plane.last_error.is_some() {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "enable-background while reconcile is required",
        ));
    }
    if control_plane.background_opt_in {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "enable-background",
        ));
    }
    if !observed.background_service.available {
        return Err(RuntimeControlHandoffError::Control(
            BackgroundServiceControlError::UnsupportedPlatform,
        ));
    }
    Ok(())
}

fn ensure_local_stop_request_allowed(
    observed: &RuntimeControlObservedState,
) -> Result<(), RuntimeControlHandoffError> {
    let Some(status) = observed.daemon_status.as_ref() else {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-local without daemon status",
        ));
    };
    let Some(ownership) = observed.ownership.as_ref() else {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-local without ownership",
        ));
    };
    if ownership.runtime_mode != DaemonRuntimeMode::Local
        || ownership.daemon_instance_id != status.daemon_instance_id
    {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-local with mismatched ownership",
        ));
    }
    Ok(())
}

fn ensure_background_stop_request_allowed(
    observed: &RuntimeControlObservedState,
) -> Result<(), RuntimeControlHandoffError> {
    let Some(status) = observed.daemon_status.as_ref() else {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-background without daemon status",
        ));
    };
    let Some(ownership) = observed.ownership.as_ref() else {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-background without ownership",
        ));
    };
    if ownership.runtime_mode != DaemonRuntimeMode::Background
        || ownership.daemon_instance_id != status.daemon_instance_id
    {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-background with mismatched ownership",
        ));
    }
    Ok(())
}

fn run_disable_background_handoff(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    thread::sleep(HANDOFF_FLUSH_GRACE);

    let expected_op_id = required_expected_transition_op_id(config)?;
    let _lock = acquire_runtime_control_lock()?;
    if !transition_matches(PendingTransitionKind::DisableBackground, expected_op_id)? {
        return Ok(());
    }

    disable_background_service().inspect_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::DisableBackground,
            expected_op_id,
            error,
        );
    })?;

    wait_for_daemon_shutdown(&config.socket_address).inspect_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::DisableBackground,
            expected_op_id,
            error,
        );
    })?;

    clear_expected_runtime_control_ownership(config).inspect_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::DisableBackground,
            expected_op_id,
            error,
        );
    })?;

    if !complete_transition_if_matches(
        PendingTransitionKind::DisableBackground,
        expected_op_id,
        DaemonRuntimeMode::Local,
        false,
    )? {
        return Ok(());
    }

    Ok(())
}

fn run_enable_background_handoff(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    thread::sleep(HANDOFF_FLUSH_GRACE);

    let expected_op_id = required_expected_transition_op_id(config)?;
    let _lock = acquire_runtime_control_lock()?;
    if !transition_matches(PendingTransitionKind::EnableBackground, expected_op_id)? {
        return Ok(());
    }

    let daemon_binary =
        resolve_daemon_binary().map_err(RuntimeControlHandoffError::ResolveBinary)?;
    let service_control_token = mint_control_token();

    if !advance_step_if_matches(
        PendingTransitionKind::EnableBackground,
        expected_op_id,
        TransitionStep::StopConflictingRuntime,
    )? {
        return Ok(());
    }
    stop_current_daemon_via_control_token(config)?;
    wait_for_daemon_shutdown(&config.socket_address).inspect_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::EnableBackground,
            expected_op_id,
            error,
        );
    })?;

    if !advance_step_if_matches(
        PendingTransitionKind::EnableBackground,
        expected_op_id,
        TransitionStep::EnsureBackgroundService,
    )? {
        return Ok(());
    }
    enable_background_service(
        &daemon_binary,
        &config
            .launch_config
            .with_control_token(Some(service_control_token.clone())),
    )
    .map_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::EnableBackground,
            expected_op_id,
            &error,
        );
        RuntimeControlHandoffError::from(error)
    })?;

    if !advance_step_if_matches(
        PendingTransitionKind::EnableBackground,
        expected_op_id,
        TransitionStep::WaitForBackgroundRuntime,
    )? {
        return Ok(());
    }
    let status = wait_for_background_daemon_ready(&config.socket_address).inspect_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::EnableBackground,
            expected_op_id,
            error,
        );
    })?;

    let service_state = read_background_service_state().map_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::EnableBackground,
            expected_op_id,
            &error,
        );
        RuntimeControlHandoffError::from(error)
    })?;
    write_runtime_control_ownership(&RuntimeControlOwnershipRecord {
        runtime_mode: DaemonRuntimeMode::Background,
        daemon_instance_id: status.daemon_instance_id,
        control_token: service_control_token,
        process_id: service_state.process_id,
    })
    .map_err(|error| {
        record_transition_failure_for_transition(
            PendingTransitionKind::EnableBackground,
            expected_op_id,
            &error,
        );
        RuntimeControlHandoffError::from(error)
    })?;

    if !complete_transition_if_matches(
        PendingTransitionKind::EnableBackground,
        expected_op_id,
        DaemonRuntimeMode::Background,
        true,
    )? {
        return Ok(());
    }

    Ok(())
}

fn run_stop_local_runtime_handoff(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    thread::sleep(HANDOFF_FLUSH_GRACE);
    wait_for_daemon_shutdown(&config.socket_address)?;
    let _lock = acquire_runtime_control_lock()?;
    clear_expected_runtime_control_ownership(config)?;
    Ok(())
}

fn run_stop_background_runtime_handoff(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    thread::sleep(HANDOFF_FLUSH_GRACE);

    match wait_for_daemon_shutdown(&config.socket_address) {
        Ok(()) => {}
        Err(RuntimeControlHandoffError::ShutdownTimeout { .. }) => {}
        Err(error) => return Err(error),
    }

    stop_background_service().map_err(|error| {
        record_transition_failure(&error);
        RuntimeControlHandoffError::from(error)
    })?;
    wait_for_daemon_shutdown(&config.socket_address)?;

    let _lock = acquire_runtime_control_lock()?;
    clear_expected_runtime_control_ownership(config)?;
    Ok(())
}

fn reconcile_enable_handoff(
    observed: &RuntimeControlObservedState,
) -> Result<(), RuntimeControlHandoffError> {
    let control_plane = read_persisted_runtime_control_state()?;
    let expected_op_id = control_plane
        .pending_transition
        .as_ref()
        .map(|pending| pending.op_id)
        .ok_or(RuntimeControlHandoffError::MissingExpectedTransitionOpId)?;
    if is_current_owned_daemon_in_mode(observed, DaemonRuntimeMode::Background) {
        complete_runtime_control_transition(DaemonRuntimeMode::Background, true)?;
        return Ok(());
    }
    if is_current_owned_daemon_in_mode(observed, DaemonRuntimeMode::Local) {
        spawn_enable_background_handoff(expected_op_id)?;
        return Ok(());
    }

    if matches!(
        observed
            .daemon_status
            .as_ref()
            .map(|status| status.runtime_mode),
        Some(DaemonRuntimeMode::Background)
    ) {
        let _ = fail_runtime_control_transition(
            DaemonControlErrorCode::TransitionFailed,
            "background runtime is running without matching ownership",
        );
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "reconcile-enable without matching ownership",
        ));
    }

    if observed.background_service.available {
        spawn_enable_background_handoff(expected_op_id)?;
        return Ok(());
    }

    let _ = fail_runtime_control_transition(
        DaemonControlErrorCode::UnsupportedPlatform,
        "background reconcile requires a supported background service",
    );
    Err(RuntimeControlHandoffError::Control(
        BackgroundServiceControlError::UnsupportedPlatform,
    ))
}

fn reconcile_disable_handoff(
    observed: &RuntimeControlObservedState,
) -> Result<(), RuntimeControlHandoffError> {
    if is_current_owned_daemon_in_mode(observed, DaemonRuntimeMode::Local) {
        complete_runtime_control_transition(DaemonRuntimeMode::Local, false)?;
        return Ok(());
    }
    if is_current_owned_daemon_in_mode(observed, DaemonRuntimeMode::Background) {
        let ownership = observed
            .ownership
            .as_ref()
            .expect("owned background reconcile should have ownership");
        let control_plane = read_persisted_runtime_control_state()?;
        let expected_op_id = control_plane
            .pending_transition
            .as_ref()
            .map(|pending| pending.op_id)
            .ok_or(RuntimeControlHandoffError::MissingExpectedTransitionOpId)?;
        spawn_disable_background_handoff(
            expected_op_id,
            Some((
                ownership.daemon_instance_id.as_str(),
                ownership.control_token.as_str(),
            )),
        )?;
        return Ok(());
    }

    disable_background_service().map_err(|error| {
        record_transition_failure(&error);
        RuntimeControlHandoffError::from(error)
    })?;
    if let Some(ownership) = observed.ownership.as_ref() {
        clear_runtime_control_ownership_if_matches(Some((
            ownership.daemon_instance_id.as_str(),
            &ownership.control_token,
        )))
        .map_err(|error| {
            record_transition_failure(&error);
            RuntimeControlHandoffError::from(error)
        })?;
    }
    complete_runtime_control_transition(DaemonRuntimeMode::Local, false).map_err(|error| {
        record_transition_failure(&error);
        RuntimeControlHandoffError::from(error)
    })?;
    Ok(())
}

fn is_current_owned_daemon_in_mode(
    observed: &RuntimeControlObservedState,
    mode: DaemonRuntimeMode,
) -> bool {
    matches!(
        (observed.daemon_status.as_ref(), observed.ownership.as_ref()),
        (Some(status), Some(ownership))
            if ownership.daemon_instance_id == status.daemon_instance_id
                && status.runtime_mode == mode
    )
}

fn wait_for_daemon_shutdown(
    socket_address: &SocketAddress,
) -> Result<(), RuntimeControlHandoffError> {
    let client = daemon_client(socket_address);
    let deadline = Instant::now() + HANDOFF_SHUTDOWN_TIMEOUT;

    loop {
        match current_status(&client) {
            Ok(_) if Instant::now() < deadline => thread::sleep(HANDOFF_POLL_INTERVAL),
            Ok(_) => {
                return Err(RuntimeControlHandoffError::ShutdownTimeout {
                    socket: client.config().socket_address.to_string(),
                });
            }
            Err(error) if is_daemon_unavailable(&error) => return Ok(()),
            Err(_error) if Instant::now() < deadline => thread::sleep(HANDOFF_POLL_INTERVAL),
            Err(error) => return Err(RuntimeControlHandoffError::Rpc(error)),
        }
    }
}

fn wait_for_background_daemon_ready(
    socket_address: &SocketAddress,
) -> Result<DaemonStatusResult, RuntimeControlHandoffError> {
    let client = daemon_client(socket_address);
    let deadline = Instant::now() + HANDOFF_SHUTDOWN_TIMEOUT;

    loop {
        match current_status(&client) {
            Ok(status)
                if status.ready && matches!(status.runtime_mode, DaemonRuntimeMode::Background) =>
            {
                return Ok(status);
            }
            Ok(_) if Instant::now() < deadline => thread::sleep(HANDOFF_POLL_INTERVAL),
            Ok(_) => {
                return Err(RuntimeControlHandoffError::BackgroundStartTimeout {
                    socket: client.config().socket_address.to_string(),
                });
            }
            Err(_error) if Instant::now() < deadline => {
                thread::sleep(HANDOFF_POLL_INTERVAL);
            }
            Err(error) => Err(RuntimeControlHandoffError::Rpc(error))?,
        }
    }
}

fn stop_current_daemon_via_control_token(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    let Some(control_token) = config.launch_config.control_token.as_ref() else {
        return Err(RuntimeControlHandoffError::ActionNotAllowed(
            "stop-current-runtime without control token",
        ));
    };
    let client = PersistentJsonRpcClient::connect(ClientConfig {
        service_name: HANDOFF_CLIENT_NAME.to_string(),
        socket_address: config.socket_address.clone(),
        io_timeout: HANDOFF_REQUEST_TIMEOUT,
    })?;
    let result = (|| {
        client.call::<_, DaemonInitializeResult>(
            METHOD_DAEMON_INITIALIZE,
            &DaemonInitializeParams {
                client_name: HANDOFF_CLIENT_NAME.to_string(),
                client_credential: None,
                client_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: crate::DAEMON_PROTOCOL_VERSION.to_string(),
                capabilities: DaemonClientCapabilities {
                    notifications: true,
                    event_subscriptions: true,
                },
            },
        )?;
        client.call::<_, InternalDaemonStopResult>(
            METHOD_DAEMON_INTERNAL_STOP,
            &InternalDaemonStopParams {
                control_token: control_token.as_str().to_string(),
            },
        )
    })();
    client.close();
    result?;
    Ok(())
}

fn daemon_client(socket_address: &SocketAddress) -> JsonRpcClient {
    JsonRpcClient::new(ClientConfig {
        service_name: HANDOFF_CLIENT_NAME.to_string(),
        socket_address: socket_address.clone(),
        io_timeout: HANDOFF_REQUEST_TIMEOUT,
    })
}

fn current_status(client: &JsonRpcClient) -> Result<DaemonStatusResult, JsonRpcClientError> {
    client.call(METHOD_DAEMON_STATUS, &DaemonStatusParams {})
}

fn required_expected_transition_op_id(
    config: &RuntimeControlHandoffConfig,
) -> Result<u64, RuntimeControlHandoffError> {
    config
        .expected_transition_op_id
        .ok_or(RuntimeControlHandoffError::MissingExpectedTransitionOpId)
}

fn transition_matches(
    kind: PendingTransitionKind,
    expected_op_id: u64,
) -> Result<bool, RuntimeControlHandoffError> {
    let control_plane = read_persisted_runtime_control_state()?;
    Ok(matches!(
        control_plane.pending_transition.as_ref(),
        Some(pending) if pending.kind == kind && pending.op_id == expected_op_id
    ))
}

fn advance_step_if_matches(
    kind: PendingTransitionKind,
    expected_op_id: u64,
    step: TransitionStep,
) -> Result<bool, RuntimeControlHandoffError> {
    if !transition_matches(kind, expected_op_id)? {
        return Ok(false);
    }
    advance_runtime_control_transition(step)?;
    Ok(true)
}

fn complete_transition_if_matches(
    kind: PendingTransitionKind,
    expected_op_id: u64,
    desired_mode: DaemonRuntimeMode,
    background_opt_in: bool,
) -> Result<bool, RuntimeControlHandoffError> {
    if !transition_matches(kind, expected_op_id)? {
        return Ok(false);
    }
    complete_runtime_control_transition(desired_mode, background_opt_in)?;
    Ok(true)
}

fn clear_expected_runtime_control_ownership(
    config: &RuntimeControlHandoffConfig,
) -> Result<(), RuntimeControlHandoffError> {
    let expected_daemon_instance_id = config
        .expected_daemon_instance_id
        .as_deref()
        .ok_or(RuntimeControlHandoffError::MissingExpectedOwnership)?;
    let expected_control_token = config
        .expected_control_token
        .as_ref()
        .ok_or(RuntimeControlHandoffError::MissingExpectedOwnership)?;
    clear_runtime_control_ownership_if_matches(Some((
        expected_daemon_instance_id,
        expected_control_token,
    )))
    .map(|_| ())
    .map_err(RuntimeControlHandoffError::from)
}

fn record_transition_failure(error: &impl std::fmt::Display) {
    let _ = fail_runtime_control_transition(
        DaemonControlErrorCode::TransitionFailed,
        error.to_string(),
    );
}

fn record_transition_failure_for_transition(
    kind: PendingTransitionKind,
    expected_op_id: u64,
    error: &impl std::fmt::Display,
) {
    let Ok(true) = transition_matches(kind, expected_op_id) else {
        return;
    };
    record_transition_failure(error);
}

#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use super::*;
    use crate::host::config::ControlToken;

    static TEST_RUNTIME_CONTROL_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn background_disable_request_persists_pending_transition_before_spawn() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-disable", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Background, true)
                .expect("seed control plane");

            let status = request_background_disable_handoff_with(
                || Ok(observed_state(DaemonRuntimeMode::Background)),
                |_, _| Ok(()),
            )
            .expect("disable handoff should start");

            assert_eq!(status.desired_mode, DaemonRuntimeMode::Local);
            assert_eq!(
                status.transition_status,
                ta_protocol::wire::DaemonTransitionStatus::Applying
            );
            assert!(status.pending_transition.is_some());
            assert_eq!(
                status.allowed_actions,
                vec![ta_protocol::wire::DaemonControlAction::Reconcile]
            );
        });
    }

    #[test]
    fn background_enable_request_persists_pending_transition_before_spawn() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-enable", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)
                .expect("seed control plane");

            let status = request_background_enable_handoff_with(
                || Ok(observed_state(DaemonRuntimeMode::Local)),
                |_| Ok(()),
            )
            .expect("enable handoff should start");

            assert_eq!(status.desired_mode, DaemonRuntimeMode::Background);
            assert_eq!(
                status.transition_status,
                ta_protocol::wire::DaemonTransitionStatus::Applying
            );
            assert!(status.pending_transition.is_some());
            assert_eq!(
                status.allowed_actions,
                vec![ta_protocol::wire::DaemonControlAction::Reconcile]
            );

            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)
                .expect("reset control plane");
        });
    }

    #[test]
    fn reconcile_clears_error_when_no_pending_transition_remains() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-reconcile-clear", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)
                .expect("seed control plane");
            fail_runtime_control_transition(
                DaemonControlErrorCode::TransitionFailed,
                "stale error",
            )
            .expect("seed error");

            let status = request_reconcile_handoff(|| Ok(observed_state(DaemonRuntimeMode::Local)))
                .expect("reconcile should clear error");

            assert_eq!(
                status.transition_status,
                ta_protocol::wire::DaemonTransitionStatus::Idle
            );
            assert!(!status.reconcile_required);
            assert!(
                read_persisted_runtime_control_state()
                    .expect("control plane should load")
                    .last_error
                    .is_none()
            );
        });
    }

    #[test]
    fn reconcile_completes_pending_enable_when_background_daemon_is_already_owned() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-reconcile-enable", || {
            start_runtime_control_transition(
                PendingTransitionKind::EnableBackground,
                DaemonRuntimeMode::Background,
                true,
                TransitionStep::WaitForBackgroundRuntime,
            )
            .expect("seed pending transition");

            let status = request_reconcile_handoff(|| {
                Ok(RuntimeControlObservedState {
                    daemon_status: Some(daemon_status(DaemonRuntimeMode::Background)),
                    background_service: background_service_state(true),
                    ownership: Some(RuntimeControlOwnershipRecord {
                        runtime_mode: DaemonRuntimeMode::Background,
                        daemon_instance_id: "daemon-1".to_string(),
                        control_token: ControlToken::new("control-token".to_string()),
                        process_id: Some(42),
                    }),
                    socket_path: "/tmp/taugentic.sock".to_string(),
                    log_path: "/tmp/taugentic.log".to_string(),
                    daemon_version: Some("0.0.1-test".to_string()),
                })
            })
            .expect("reconcile should complete");

            assert_eq!(
                status.transition_status,
                ta_protocol::wire::DaemonTransitionStatus::Idle
            );
            assert_eq!(status.desired_mode, DaemonRuntimeMode::Background);
            assert!(!status.reconcile_required);
            assert!(status.pending_transition.is_none());
        });
    }

    #[test]
    fn reconcile_disable_keeps_owned_local_ownership() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-reconcile-disable-local", || {
            start_runtime_control_transition(
                PendingTransitionKind::DisableBackground,
                DaemonRuntimeMode::Local,
                false,
                TransitionStep::StopBackgroundService,
            )
            .expect("seed pending transition");
            write_runtime_control_ownership(&RuntimeControlOwnershipRecord {
                runtime_mode: DaemonRuntimeMode::Local,
                daemon_instance_id: "daemon-1".to_string(),
                control_token: ControlToken::new("control-token".to_string()),
                process_id: Some(42),
            })
            .expect("seed ownership");

            let status = request_reconcile_handoff(|| Ok(observed_state(DaemonRuntimeMode::Local)))
                .expect("reconcile should complete");

            assert_eq!(
                status.transition_status,
                ta_protocol::wire::DaemonTransitionStatus::Idle
            );
            assert_eq!(status.desired_mode, DaemonRuntimeMode::Local);
            assert_eq!(
                crate::read_runtime_control_ownership()
                    .expect("ownership should read")
                    .expect("ownership should remain")
                    .runtime_mode,
                DaemonRuntimeMode::Local
            );
        });
    }

    #[test]
    fn reconcile_enable_requires_matching_ownership_before_completion() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-reconcile-enable-missing-owner", || {
            start_runtime_control_transition(
                PendingTransitionKind::EnableBackground,
                DaemonRuntimeMode::Background,
                true,
                TransitionStep::WaitForBackgroundRuntime,
            )
            .expect("seed pending transition");

            let error = request_reconcile_handoff(|| {
                Ok(RuntimeControlObservedState {
                    daemon_status: Some(daemon_status(DaemonRuntimeMode::Background)),
                    background_service: background_service_state(true),
                    ownership: None,
                    socket_path: "/tmp/taugentic.sock".to_string(),
                    log_path: "/tmp/taugentic.log".to_string(),
                    daemon_version: Some("0.0.1-test".to_string()),
                })
            })
            .expect_err("reconcile should not complete without ownership");

            assert!(matches!(
                error,
                RuntimeControlHandoffError::ActionNotAllowed(
                    "reconcile-enable without matching ownership"
                )
            ));
            assert!(
                read_persisted_runtime_control_state()
                    .expect("control plane should load")
                    .last_error
                    .is_some()
            );
        });
    }

    #[test]
    fn stop_local_request_spawns_handoff_with_expected_ownership() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-stop-local", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)
                .expect("seed control plane");

            let observed = observed_state(DaemonRuntimeMode::Local);
            let mut spawned = None;
            let status = request_stop_handoff_with(
                DaemonRuntimeMode::Local,
                || Ok(observed.clone()),
                |ownership| {
                    spawned = Some(ownership_to_tuple(ownership));
                    Ok(())
                },
                |_| panic!("background stop should not be used for local mode"),
            )
            .expect("local stop should start");

            assert_eq!(
                spawned,
                Some(("daemon-1".to_string(), "control-token".to_string()))
            );
            assert!(
                status
                    .allowed_actions
                    .contains(&ta_protocol::wire::DaemonControlAction::Stop)
            );
        });
    }

    #[test]
    fn desktop_local_lease_matches_only_the_exact_owned_local_instance() {
        let local = observed_state(DaemonRuntimeMode::Local);
        assert!(desktop_local_stop_ownership(&local, "daemon-1", 42).is_some());
        assert!(desktop_local_stop_ownership(&local, "daemon-1", 43).is_none());

        let mut replaced = local.clone();
        replaced
            .daemon_status
            .as_mut()
            .expect("status")
            .daemon_instance_id = "daemon-replaced".to_string();
        assert!(desktop_local_stop_ownership(&replaced, "daemon-1", 42).is_none());

        let background = observed_state(DaemonRuntimeMode::Background);
        assert!(desktop_local_stop_ownership(&background, "daemon-1", 42).is_none());
    }

    #[test]
    fn stop_background_request_spawns_handoff_with_expected_ownership() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-stop-background", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Background, true)
                .expect("seed control plane");

            let observed = observed_state(DaemonRuntimeMode::Background);
            let mut spawned = None;
            let status = request_stop_handoff_with(
                DaemonRuntimeMode::Background,
                || Ok(observed.clone()),
                |_| panic!("local stop should not be used for background mode"),
                |ownership| {
                    spawned = Some(ownership_to_tuple(ownership));
                    Ok(())
                },
            )
            .expect("background stop should start");

            assert_eq!(
                spawned,
                Some(("daemon-1".to_string(), "control-token".to_string()))
            );
            assert!(
                status
                    .allowed_actions
                    .contains(&ta_protocol::wire::DaemonControlAction::Stop)
            );
        });
    }

    #[test]
    fn stop_local_request_rejects_missing_ownership() {
        let _guard = TEST_RUNTIME_CONTROL_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::with_test_config_home("handoff-stop-local-denied", || {
            complete_runtime_control_transition(DaemonRuntimeMode::Local, false)
                .expect("seed control plane");

            let mut observed = observed_state(DaemonRuntimeMode::Local);
            observed.ownership = None;
            let error = request_stop_handoff_with(
                DaemonRuntimeMode::Local,
                || Ok(observed.clone()),
                |_| panic!("local handoff should not spawn when stop is denied"),
                |_| panic!("background handoff should not spawn when stop is denied"),
            )
            .expect_err("local stop without ownership should be denied");

            assert!(matches!(
                error,
                RuntimeControlHandoffError::ActionNotAllowed("stop-local without ownership")
            ));
        });
    }

    fn observed_state(runtime_mode: DaemonRuntimeMode) -> RuntimeControlObservedState {
        RuntimeControlObservedState {
            daemon_status: Some(daemon_status(runtime_mode)),
            background_service: background_service_state(true),
            ownership: Some(RuntimeControlOwnershipRecord {
                runtime_mode,
                daemon_instance_id: "daemon-1".to_string(),
                control_token: ControlToken::new("control-token".to_string()),
                process_id: Some(42),
            }),
            socket_path: "/tmp/taugentic.sock".to_string(),
            log_path: "/tmp/taugentic.log".to_string(),
            daemon_version: Some("0.0.1-test".to_string()),
        }
    }

    fn daemon_status(runtime_mode: DaemonRuntimeMode) -> DaemonStatusResult {
        DaemonStatusResult {
            ready: true,
            daemon_instance_id: "daemon-1".to_string(),
            runtime_mode,
            socket_path: "/tmp/taugentic.sock".to_string(),
            log_path: "/tmp/taugentic.log".to_string(),
            version: "0.0.1-test".to_string(),
        }
    }

    fn background_service_state(running: bool) -> crate::BackgroundServiceState {
        crate::BackgroundServiceState {
            available: true,
            enabled: running,
            loaded: running,
            running,
            process_id: if running { Some(42) } else { None },
            service_name: Some("taugentic-daemon.service".to_string()),
        }
    }

    fn ownership_to_tuple(ownership: (&str, &str)) -> (String, String) {
        (ownership.0.to_string(), ownership.1.to_string())
    }
}
