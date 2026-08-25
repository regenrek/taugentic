use std::sync::Arc;
use ta_observability::{ObservabilityHandle, ObservabilityInitError, init};
use ta_work_source::HostSecretsGitHubCredentialProvider;
use thiserror::Error;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};
use tokio::time::{Duration, Instant, interval_at};
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
    let model_catalog_refresh = spawn_model_catalog_refresh(
        state.runtime.agent_runtime_strategy_registry(),
        poller_cancellation.clone(),
    );
    let github_credentials = Arc::new(HostSecretsGitHubCredentialProvider::from_default_store()?);
    let poller = state
        .app
        .spawn_work_source_poller(poller_cancellation.clone(), github_credentials);
    let serve_result = serve(state).await;
    poller_cancellation.cancel();
    let _ = poller.await;
    let _ = model_catalog_refresh.await;
    serve_result?;
    Ok(())
}

fn spawn_model_catalog_refresh(
    registry: crate::StrategyRegistry,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        const REFRESH_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
        let source = match ta_model_catalog::ModelsDevCatalogSource::new() {
            Ok(source) => source,
            Err(error) => {
                tracing::error!(error = %error, "model catalog client initialization failed");
                return;
            }
        };
        refresh_model_catalog(&registry, &source).await;
        let mut interval = interval_at(Instant::now() + REFRESH_INTERVAL, REFRESH_INTERVAL);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = interval.tick() => refresh_model_catalog(&registry, &source).await,
            }
        }
    })
}

async fn refresh_model_catalog(
    registry: &crate::StrategyRegistry,
    source: &ta_model_catalog::ModelsDevCatalogSource,
) {
    match source.fetch().await {
        Ok(catalog) => {
            let generated_at = catalog.generated_at.clone();
            let provider_count = catalog.providers.len();
            if let Err(error) = registry.replace_catalog(catalog) {
                tracing::error!(error = %error, "model catalog validation failed");
                return;
            }
            tracing::info!(%generated_at, provider_count, "model catalog refreshed");
        }
        Err(error) => tracing::warn!(error = %error, "model catalog refresh failed"),
    }
}

fn daemon_runtime() -> Result<Runtime, DaemonError> {
    RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(DaemonError::Runtime)
}
