use crate::{
    DaemonStatusResult, RuntimeControlObservedState, host::bootstrap::BootstrapState,
    read_background_service_state, read_runtime_control_ownership,
};

pub fn observe_runtime_control_state(
    state: &BootstrapState,
) -> Result<RuntimeControlObservedState, crate::BackgroundServiceControlError> {
    let daemon_status = DaemonStatusResult {
        ready: state.runtime.capabilities().is_ready(),
        daemon_instance_id: state.runtime.daemon_instance_id(),
        runtime_mode: state.config.runtime_mode,
        socket_path: state.config.socket_address().to_string(),
        log_path: state.config.log_path().display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    Ok(RuntimeControlObservedState {
        daemon_status: Some(daemon_status.clone()),
        background_service: read_background_service_state()?,
        ownership: read_runtime_control_ownership()?,
        socket_path: daemon_status.socket_path,
        log_path: daemon_status.log_path,
        daemon_version: Some(daemon_status.version),
    })
}
