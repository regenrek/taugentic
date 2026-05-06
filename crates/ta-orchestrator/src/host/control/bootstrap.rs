use std::{
    ffi::OsString,
    io::{self, Write},
};

use serde_json::to_writer;
use thiserror::Error;

use crate::host::config::{DaemonConfig, DaemonConfigError};
use crate::{
    RuntimeControlBootstrapConfig, parse_runtime_control_bootstrap_action,
    run_runtime_control_bootstrap_action,
};

#[derive(Debug, Error)]
pub enum RuntimeControlBootstrapError {
    #[error(transparent)]
    Config(#[from] DaemonConfigError),
    #[error(transparent)]
    Control(#[from] Box<crate::RuntimeControlBootstrapError>),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn try_run_from_args<I>(args: I) -> Result<bool, RuntimeControlBootstrapError>
where
    I: IntoIterator<Item = OsString>,
{
    let Some(action) = parse_runtime_control_bootstrap_action(args)
        .map_err(|error| RuntimeControlBootstrapError::Control(Box::new(error)))?
    else {
        return Ok(false);
    };

    let config = DaemonConfig::load()?;
    let status = run_runtime_control_bootstrap_action(
        action,
        &RuntimeControlBootstrapConfig {
            socket_address: config.socket_address().clone(),
            launch_config: config.daemon_control_launch_config(),
            runtime_mode: config.runtime_mode,
        },
    )
    .map_err(|error| RuntimeControlBootstrapError::Control(Box::new(error)))?;

    let stdout = io::stdout();
    let mut lock = stdout.lock();
    to_writer(&mut lock, &status)?;
    lock.write_all(b"\n")?;
    lock.flush()?;
    Ok(true)
}
