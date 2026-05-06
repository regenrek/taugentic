use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use ta_jsonrpc::{JsonRpcServer, JsonRpcServerError};
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::host::{bootstrap::BootstrapState, rpc::make_session_handler};

use super::remote_websocket::serve_remote_until;

#[derive(Debug, Error)]
pub enum DaemonServeError {
    #[error(transparent)]
    Local(#[from] JsonRpcServerError),
    #[error(transparent)]
    Remote(#[from] super::remote_websocket::RemoteWebsocketServerError),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error("remote websocket server task panicked")]
    RemoteJoinPanicked,
}

pub async fn serve(state: BootstrapState) -> Result<(), DaemonServeError> {
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let local_server = build_local_server(&state, Arc::clone(&shutdown_requested));
    let mut remote_handle = spawn_remote_server(state.clone(), Arc::clone(&shutdown_requested));

    let local_future = local_server.serve_until(Arc::clone(&shutdown_requested));
    tokio::pin!(local_future);

    // Supervise local and remote together so a panic or early exit from the
    // remote websocket task immediately requests local shutdown instead of
    // letting the daemon silently keep serving local traffic with a dead
    // remote listener.
    let local_result = match remote_handle.as_mut() {
        Some(remote) => {
            tokio::select! {
                biased;
                remote_outcome = &mut *remote => {
                    shutdown_requested.store(true, Ordering::SeqCst);
                    wake_local_server_accept_loop(&state);
                    let local_result = local_future.as_mut().await;
                    return finalize_with_remote_first(remote_outcome, local_result);
                }
                local_result = local_future.as_mut() => {
                    shutdown_requested.store(true, Ordering::SeqCst);
                    local_result
                }
            }
        }
        None => local_future.as_mut().await,
    };

    shutdown_requested.store(true, Ordering::SeqCst);

    if let Some(remote_handle) = remote_handle {
        match remote_handle.await {
            Ok(remote_result) => {
                if let Err(error) = local_result {
                    return Err(DaemonServeError::Local(error));
                }
                remote_result?;
            }
            Err(_) => {
                if let Err(error) = local_result {
                    return Err(DaemonServeError::Local(error));
                }
                return Err(DaemonServeError::RemoteJoinPanicked);
            }
        }
    }

    local_result.map_err(DaemonServeError::Local)
}

fn finalize_with_remote_first(
    remote_outcome: Result<
        Result<(), super::remote_websocket::RemoteWebsocketServerError>,
        tokio::task::JoinError,
    >,
    local_result: Result<(), JsonRpcServerError>,
) -> Result<(), DaemonServeError> {
    match remote_outcome {
        Ok(Ok(())) => local_result.map_err(DaemonServeError::Local),
        Ok(Err(error)) => {
            tracing::error!(
                error = %error,
                "remote websocket listener exited unexpectedly; daemon shutting down",
            );
            if let Err(local_error) = local_result {
                tracing::error!(
                    error = %local_error,
                    "local json-rpc server reported error during forced shutdown",
                );
            }
            Err(DaemonServeError::Remote(error))
        }
        Err(_) => {
            tracing::error!("remote websocket listener task panicked; daemon shutting down",);
            if let Err(local_error) = local_result {
                tracing::error!(
                    error = %local_error,
                    "local json-rpc server reported error during forced shutdown",
                );
            }
            Err(DaemonServeError::RemoteJoinPanicked)
        }
    }
}

fn build_local_server(
    state: &BootstrapState,
    shutdown_requested: Arc<AtomicBool>,
) -> JsonRpcServer {
    let state = state.clone();
    JsonRpcServer::new(state.config.server.clone(), move |session| {
        make_session_handler(state.clone(), Arc::clone(&shutdown_requested), session)
    })
    .with_persistent_request_method(crate::METHOD_DAEMON_INITIALIZE)
}

fn spawn_remote_server(
    state: BootstrapState,
    shutdown_requested: Arc<AtomicBool>,
) -> Option<JoinHandle<Result<(), super::remote_websocket::RemoteWebsocketServerError>>> {
    state.config.remote.as_ref()?;

    Some(tokio::spawn(async move {
        let result = serve_remote_until(state.clone(), Arc::clone(&shutdown_requested)).await;
        if result.is_err() {
            shutdown_requested.store(true, Ordering::SeqCst);
            wake_local_server_accept_loop(&state);
        }
        result
    }))
}

fn wake_local_server_accept_loop(state: &BootstrapState) {
    if let Err(error) = crate::connect_socket(state.config.socket_address()) {
        tracing::debug!(
            error.message = %error.to_string(),
            socket.address = %state.config.socket_address(),
            "failed to wake local json-rpc accept loop"
        );
    }
}
