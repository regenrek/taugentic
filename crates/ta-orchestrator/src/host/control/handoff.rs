use std::ffi::OsString;

use thiserror::Error;

use crate::{
    RuntimeControlHandoffConfig,
    host::config::{DaemonConfig, RuntimeControlHandoffExpectation},
    parse_runtime_control_handoff_action, run_runtime_control_handoff_action,
};

#[derive(Debug, Error)]
pub enum RuntimeControlHandoffError {
    #[error(transparent)]
    Config(#[from] super::super::config::DaemonConfigError),
    #[error(transparent)]
    Control(#[from] crate::RuntimeControlHandoffError),
}

pub fn try_run_from_args<I>(args: I) -> Result<bool, RuntimeControlHandoffError>
where
    I: IntoIterator<Item = OsString>,
{
    let Some(action) = parse_runtime_control_handoff_action(args)? else {
        return Ok(false);
    };
    let config = DaemonConfig::load()?;
    let expected = RuntimeControlHandoffExpectation::from_env();
    run_runtime_control_handoff_action(
        action,
        &RuntimeControlHandoffConfig {
            socket_address: config.socket_address().clone(),
            launch_config: config.daemon_control_launch_config(),
            expected_transition_op_id: expected.expected_transition_op_id,
            expected_daemon_instance_id: expected.expected_daemon_instance_id,
            expected_control_token: expected.expected_control_token,
        },
    )?;
    Ok(true)
}
