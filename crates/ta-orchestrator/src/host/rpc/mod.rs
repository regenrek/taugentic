use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::{thread, time::Duration};

use serde::Serialize;

use crate::{
    DaemonEventEnvelope, DaemonNavigationInvalidatedParams, DaemonStatusResult,
    JsonRpcHandlerFuture, JsonRpcHandlerResult, JsonRpcRequest, JsonRpcServerSession,
    METHOD_DAEMON_EVENT, METHOD_DAEMON_NAVIGATION_INVALIDATED, METHOD_DAEMON_RUN_EVENT,
    PublicDaemonEventEnvelope, RunEventStreamItem, RunEventStreamPayload, RunId, connect_socket,
    host::bootstrap::BootstrapState, internal_error,
};

mod dispatch;
mod errors;
mod request;
mod state;
#[cfg(test)]
mod tests;

use state::DaemonRpcSessionState;

const SESSION_FORWARDER_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub fn make_session_handler(
    state: BootstrapState,
    shutdown_requested: Arc<AtomicBool>,
    session: JsonRpcServerSession,
) -> Box<dyn Fn(JsonRpcRequest) -> JsonRpcHandlerFuture + Send + Sync + 'static> {
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    Box::new(move |request| {
        let state = state.clone();
        let shutdown_requested = Arc::clone(&shutdown_requested);
        let session = session.clone();
        let session_state = Arc::clone(&session_state);
        Box::pin(async move {
            dispatch::handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                request,
            )
            .await
        }) as JsonRpcHandlerFuture
    })
}

fn daemon_status_result(state: &BootstrapState) -> DaemonStatusResult {
    DaemonStatusResult {
        ready: state.runtime.capabilities().is_ready(),
        daemon_instance_id: state.runtime.daemon_instance_id(),
        runtime_mode: state.config.runtime_mode,
        socket_path: state.config.socket_address().to_string(),
        log_path: state.config.log_path().display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn json_result<T>(value: T) -> JsonRpcHandlerResult
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|error| internal_error(error.to_string()))
}

fn spawn_event_forwarder(
    session: JsonRpcServerSession,
    backlog: Vec<DaemonEventEnvelope>,
    receiver: mpsc::Receiver<DaemonEventEnvelope>,
    overflowed: Arc<AtomicBool>,
    cleanup: Option<crate::host::event_hub::RuntimeEventSubscriptionCleanup>,
) {
    let connection_id = session.connection_id();
    let thread_name = format!("daemon-event-forwarder-{connection_id}");
    let spawn_result = thread::Builder::new().name(thread_name).spawn(move || {
        let _cleanup = cleanup;
        for event in backlog {
            if !session.is_open() {
                return;
            }
            let Some(params) = serialize_event_notification_params(&session, &event) else {
                return;
            };
            if let Err(error) = session.send_notification(METHOD_DAEMON_EVENT, Some(params)) {
                tracing::debug!(error = %error, "stopping daemon event forwarder");
                return;
            }
        }

        loop {
            if !session.is_open() {
                return;
            }

            if close_session_on_subscriber_overflow(&session, &overflowed) {
                session.close();
                return;
            }

            match receiver.recv_timeout(SESSION_FORWARDER_POLL_INTERVAL) {
                Ok(event) => {
                    let Some(params) = serialize_event_notification_params(&session, &event) else {
                        return;
                    };
                    if let Err(error) = session.send_notification(METHOD_DAEMON_EVENT, Some(params))
                    {
                        tracing::debug!(error = %error, "stopping daemon event forwarder");
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if close_session_on_subscriber_overflow(&session, &overflowed) {
                        session.close();
                        return;
                    }
                    tracing::warn!(
                        taugentic.connection_id = session.connection_id() as u64,
                        "daemon event subscriber disconnected; closing session"
                    );
                    session.close();
                    return;
                }
            }
        }
    });

    if let Err(error) = spawn_result {
        tracing::error!(
            taugentic.connection_id = connection_id as u64,
            error = %error,
            "failed to spawn daemon event forwarder thread",
        );
    }
}

fn spawn_run_event_forwarder(
    session: JsonRpcServerSession,
    run_id: RunId,
    subscription: crate::orchestration::RunEventSubscription,
) {
    let connection_id = session.connection_id();
    let thread_name = format!("daemon-run-event-forwarder-{connection_id}");
    let spawn_result = thread::Builder::new().name(thread_name).spawn(move || {
        let subscription = subscription;
        loop {
            if !session.is_open() {
                return;
            }

            match subscription
                .receiver
                .recv_timeout(SESSION_FORWARDER_POLL_INTERVAL)
            {
                Ok(item) => {
                    let terminal = item.is_err();
                    let Some(params) =
                        serialize_run_event_notification_params(&session, &run_id, item)
                    else {
                        return;
                    };
                    if let Err(error) =
                        session.send_notification(METHOD_DAEMON_RUN_EVENT, Some(params))
                    {
                        tracing::debug!(error = %error, "stopping daemon run event forwarder");
                        return;
                    }
                    if terminal {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });

    if let Err(error) = spawn_result {
        tracing::error!(
            taugentic.connection_id = connection_id as u64,
            error = %error,
            "failed to spawn daemon run event forwarder thread",
        );
    }
}

fn spawn_navigation_invalidation_forwarder(
    session: JsonRpcServerSession,
    subscription: crate::host::event_hub::NavigationInvalidationSubscription,
) {
    let connection_id = session.connection_id();
    let thread_name = format!("daemon-navigation-forwarder-{connection_id}");
    let spawn_result = thread::Builder::new().name(thread_name).spawn(move || {
        let crate::host::event_hub::NavigationInvalidationSubscription { cleanup, receiver } =
            subscription;
        let _cleanup = cleanup;
        while receiver.recv().is_ok() {
            if !session.is_open() {
                return;
            }
            let params = match serde_json::to_value(DaemonNavigationInvalidatedParams {}) {
                Ok(params) => params,
                Err(_) => {
                    session.close();
                    return;
                }
            };
            if session
                .send_notification(METHOD_DAEMON_NAVIGATION_INVALIDATED, Some(params))
                .is_err()
            {
                return;
            }
        }
    });

    if let Err(error) = spawn_result {
        tracing::error!(
            taugentic.connection_id = connection_id as u64,
            error = %error,
            "failed to spawn daemon navigation forwarder thread",
        );
    }
}

fn defer_publish_records(
    session: &JsonRpcServerSession,
    runtime: crate::RuntimeService,
    records: Vec<ta_store::EventRecord>,
) {
    session.defer_until_response(Box::new(move || {
        for record in records {
            runtime.publish_record(&record);
        }
    }));
}

fn defer_navigation_invalidation(
    session: &JsonRpcServerSession,
    runtime: crate::RuntimeService,
    principal_id: String,
) {
    session.defer_until_response(Box::new(move || {
        runtime.publish_navigation_for_principal(&principal_id);
    }));
}

fn json_deferred_mutation_result<T: Serialize, U>(
    session: &JsonRpcServerSession,
    runtime: crate::RuntimeService,
    result: crate::orchestration::AppDeferredMutationResult<U>,
    body: impl FnOnce(crate::orchestration::AppDeferredMutationResult<U>) -> T,
) -> JsonRpcHandlerResult {
    let records = result.deferred_records.clone();
    defer_publish_records(session, runtime, records);
    json_result(body(result))
}

fn close_session_on_subscriber_overflow(
    session: &JsonRpcServerSession,
    overflowed: &Arc<AtomicBool>,
) -> bool {
    if !overflowed.load(Ordering::SeqCst) {
        return false;
    }

    tracing::warn!(
        taugentic.connection_id = session.connection_id() as u64,
        "daemon event subscriber overflowed; closing session"
    );
    true
}

fn serialize_event_notification_params(
    session: &JsonRpcServerSession,
    event: &DaemonEventEnvelope,
) -> Option<serde_json::Value> {
    match serde_json::to_value(PublicDaemonEventEnvelope::from(event.clone())) {
        Ok(params) => Some(params),
        Err(error) => {
            close_session_for_event_serialization_failure(session, &error);
            None
        }
    }
}

fn serialize_run_event_notification_params(
    session: &JsonRpcServerSession,
    run_id: &RunId,
    item: Result<crate::RunEventDelta, crate::RunEventStreamError>,
) -> Option<serde_json::Value> {
    let payload = match item {
        Ok(delta) => RunEventStreamPayload::Delta { delta },
        Err(error) => RunEventStreamPayload::Error { error },
    };
    match serde_json::to_value(RunEventStreamItem {
        run_id: run_id.clone(),
        payload,
    }) {
        Ok(params) => Some(params),
        Err(error) => {
            close_session_for_event_serialization_failure(session, &error);
            None
        }
    }
}

fn close_session_for_event_serialization_failure(
    session: &JsonRpcServerSession,
    error: &serde_json::Error,
) {
    tracing::error!(error = %error, "failed to serialize daemon event");
    session.close();
}

pub(crate) fn wake_local_server_accept_loop(state: &BootstrapState) {
    if let Err(error) = connect_socket(state.config.socket_address()) {
        tracing::debug!(
            error = %error,
            socket.address = %state.config.socket_address(),
            "failed to wake local json-rpc accept loop"
        );
    }
}
