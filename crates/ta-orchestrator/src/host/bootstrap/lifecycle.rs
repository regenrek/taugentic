use std::sync::Arc;
use ta_observability::{ObservabilityHandle, ObservabilityInitError, init};
use ta_work_source::HostSecretsGitHubCredentialProvider;
use thiserror::Error;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio_util::sync::CancellationToken;

use crate::host::{
    config::DaemonConfig,
    control::{
        bootstrap::RuntimeControlBootstrapError,
        bootstrap::try_run_from_args as try_run_bootstrap_from_args,
        handoff::RuntimeControlHandoffError,
        handoff::try_run_from_args as try_run_handoff_from_args,
    },
    transport::local_jsonrpc::{DaemonServeError, serve},
};

use super::{BootstrapStateError, open_bootstrap_state};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error(transparent)]
    Observability(#[from] ObservabilityInitError),
    #[error(transparent)]
    Config(#[from] crate::host::config::DaemonConfigError),
    #[error(transparent)]
    BootstrapState(#[from] BootstrapStateError),
    #[error(transparent)]
    Store(#[from] ta_store::StoreError),
    #[error(transparent)]
    WorkSource(#[from] ta_work_source::WorkSourceError),
    #[error(transparent)]
    Server(#[from] DaemonServeError),
    #[error(transparent)]
    RuntimeControlHandoff(#[from] Box<RuntimeControlHandoffError>),
    #[error(transparent)]
    RuntimeControlBootstrap(#[from] Box<RuntimeControlBootstrapError>),
    #[error("failed to construct daemon async runtime: {0}")]
    Runtime(#[source] std::io::Error),
}

pub fn run() -> Result<(), DaemonError> {
    if try_run_bootstrap_from_args(std::env::args_os())
        .map_err(|error| DaemonError::RuntimeControlBootstrap(Box::new(error)))?
    {
        return Ok(());
    }
    if try_run_handoff_from_args(std::env::args_os())
        .map_err(|error| DaemonError::RuntimeControlHandoff(Box::new(error)))?
    {
        return Ok(());
    }
    daemon_runtime()?.block_on(run_async())
}

async fn run_async() -> Result<(), DaemonError> {
    let config = DaemonConfig::load()?;
    let _observability: ObservabilityHandle = init(config.observability.clone())?;
    let state = open_bootstrap_state(config)?;
    tracing::info!(
        daemon.instance_id = %state.runtime.daemon_instance_id(),
        socket.address = %state.config.socket_address(),
        runtime.capabilities.shell = state.runtime.capabilities().supports_shell,
        runtime.capabilities.file_edits = state.runtime.capabilities().supports_file_edits,
        runtime.capabilities.subagents = state.runtime.capabilities().supports_subagents,
        "daemon boot complete"
    );
    let poller_cancellation = CancellationToken::new();
    let github_credentials = Arc::new(HostSecretsGitHubCredentialProvider::from_default_store()?);
    let poller = state
        .app
        .spawn_work_source_poller(poller_cancellation.clone(), github_credentials);
    let serve_result = serve(state).await;
    poller_cancellation.cancel();
    let _ = poller.await;
    serve_result?;
    Ok(())
}

fn daemon_runtime() -> Result<Runtime, DaemonError> {
    RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(DaemonError::Runtime)
}
