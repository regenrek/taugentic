mod background;
mod bootstrap;
mod error;
mod handoff;
mod operations;
mod paths;
mod process;
mod state;

pub use background::{
    BackgroundServiceState, disable_background_service, enable_background_service,
    read_background_service_state, stop_background_service,
};
pub(crate) use bootstrap::bootstrap_desktop_runtime;
pub use bootstrap::{
    RuntimeControlBootstrapConfig, RuntimeControlBootstrapError,
    parse_runtime_control_bootstrap_action, run_runtime_control_bootstrap_action,
};
pub use error::{BackgroundServiceControlError, DaemonControlConfigError};
pub(crate) use handoff::{
    HANDOFF_CLIENT_NAME, HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR,
    HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR, HANDOFF_EXPECTED_OP_ID_ENV_VAR,
};
pub use handoff::{
    RuntimeControlHandoffConfig, RuntimeControlHandoffError, parse_runtime_control_handoff_action,
    request_background_disable_handoff, request_background_enable_handoff,
    request_reconcile_handoff, request_stop_handoff, run_runtime_control_handoff_action,
};
pub use operations::{
    DaemonControlOperationError, DesktopRuntimeHandle, DesktopRuntimeStartStage,
    invoke_runtime_control_bootstrap, is_daemon_unavailable, resolve_daemon_binary,
    spawn_daemon_process, start_desktop_runtime,
};
pub use paths::{
    DAEMON_CONTROL_TOKEN_ENV_VAR, DAEMON_RUNTIME_MODE_ENV_VAR, daemon_log_path_for_socket_address,
    daemon_runtime_mode_file_path, runtime_control_state_file_path,
};
pub(crate) use process::{process_is_running, terminate_process};
pub use state::{
    PendingTransitionKind, PersistedRuntimeControlState, RuntimeControlObservedState,
    RuntimeControlOwnershipRecord, TransitionStep, acquire_runtime_control_lock,
    advance_runtime_control_transition, clear_runtime_control_error,
    clear_runtime_control_ownership_if_matches, complete_runtime_control_transition,
    daemon_control_status, fail_runtime_control_transition, read_persisted_runtime_control_state,
    read_runtime_control_ownership, start_runtime_control_transition,
    write_runtime_control_ownership,
};

#[cfg(test)]
pub use paths::with_test_config_home;
