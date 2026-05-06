use std::{
    any::Any,
    future::Future,
    mem,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures_util::FutureExt;
use serde_json::{Value, json};
use ta_observability::rpc_server_request_span;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    JsonRpcError, JsonRpcErrorObject, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse,
};

pub const DEFAULT_OUTBOUND_QUEUE_DEPTH: usize = 256;
pub const DEFAULT_PERSISTENT_CONNECTION_POLL_INTERVAL: Duration = Duration::from_millis(250);
type JsonRpcAfterResponseAction = Box<dyn FnOnce() + Send + 'static>;

pub type JsonRpcHandlerResult = Result<Value, JsonRpcErrorObject>;
pub type JsonRpcHandlerFuture =
    Pin<Box<dyn Future<Output = JsonRpcHandlerResult> + Send + 'static>>;
pub type JsonRpcRequestHandler =
    dyn Fn(JsonRpcRequest) -> JsonRpcHandlerFuture + Send + Sync + 'static;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JsonRpcSessionError {
    #[error("failed to send outbound JSON-RPC message: connection already closed")]
    ConnectionClosed,
    #[error("json-rpc outbound queue overflowed for slow connection")]
    OutboundBackpressure,
}

#[derive(Clone)]
pub struct JsonRpcServerSession {
    connection_id: usize,
    outbound_tx: mpsc::Sender<JsonRpcMessage>,
    is_open: Arc<AtomicBool>,
    after_response_actions: Arc<Mutex<Vec<JsonRpcAfterResponseAction>>>,
}

impl std::fmt::Debug for JsonRpcServerSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JsonRpcServerSession")
            .field("connection_id", &self.connection_id)
            .field("is_open", &self.is_open())
            .finish()
    }
}

impl JsonRpcServerSession {
    pub fn new(
        connection_id: usize,
        outbound_tx: mpsc::Sender<JsonRpcMessage>,
        is_open: Arc<AtomicBool>,
    ) -> Self {
        Self {
            connection_id,
            outbound_tx,
            is_open,
            after_response_actions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn connection_id(&self) -> usize {
        self.connection_id
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        self.is_open.store(false, Ordering::SeqCst);
    }

    pub fn send_notification(
        &self,
        method: impl Into<String>,
        params: Option<Value>,
    ) -> Result<(), JsonRpcSessionError> {
        self.send_message(JsonRpcMessage::Notification(JsonRpcNotification::new(
            method, params,
        )))
    }

    pub fn defer_until_response(&self, action: JsonRpcAfterResponseAction) {
        self.after_response_actions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(action);
    }

    fn take_after_response_actions(&self) -> Vec<JsonRpcAfterResponseAction> {
        let mut actions = self
            .after_response_actions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        mem::take(&mut *actions)
    }

    fn send_message(&self, message: JsonRpcMessage) -> Result<(), JsonRpcSessionError> {
        if !self.is_open() {
            return Err(JsonRpcSessionError::ConnectionClosed);
        }

        match enqueue_outbound_message(&self.outbound_tx, &self.is_open, message) {
            Ok(()) => Ok(()),
            Err(OutboundQueueError::Backpressure) => Err(JsonRpcSessionError::OutboundBackpressure),
            Err(OutboundQueueError::Closed) => Err(JsonRpcSessionError::ConnectionClosed),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedJsonRpcMessage {
    pub request: Option<JsonRpcRequest>,
    pub response: Option<JsonRpcMessage>,
}

#[derive(Debug, Clone, Copy)]
pub struct JsonRpcRequestProcessingContext<'a> {
    pub service_name: &'a str,
    pub transport: &'static str,
    pub connection_id: usize,
}

pub async fn process_jsonrpc_request(
    handler: &(impl Fn(JsonRpcRequest) -> JsonRpcHandlerFuture + ?Sized),
    context: JsonRpcRequestProcessingContext<'_>,
    request: JsonRpcRequest,
) -> ProcessedJsonRpcMessage {
    let request_span = rpc_server_request_span(
        context.service_name,
        context.transport,
        context.connection_id,
        request.method.as_str(),
        &request.id,
        request.jsonrpc.as_str(),
        request.params.is_some(),
    );
    let _request_guard = request_span.enter();
    tracing::trace!("received JSON-RPC request");

    let response = match AssertUnwindSafe(async { handler(request.clone()).await })
        .catch_unwind()
        .await
    {
        Ok(Ok(result)) => {
            tracing::debug!("json-rpc request completed successfully");
            JsonRpcMessage::Response(JsonRpcResponse::new(request.id.clone(), result))
        }
        Ok(Err(error)) => {
            tracing::warn!(
                error.code = error.code,
                error.message = %error.message,
                "json-rpc request completed with an error"
            );
            JsonRpcMessage::Error(JsonRpcError::new(Some(request.id.clone()), error))
        }
        Err(payload) => {
            let panic_message = panic_payload_to_string(payload);
            tracing::error!(
                rpc.method = %request.method,
                rpc.request_id = %request.id,
                panic.message = %panic_message,
                "RPC handler panicked"
            );
            JsonRpcMessage::Error(JsonRpcError::new(
                Some(request.id.clone()),
                handler_panicked_error(&request.method),
            ))
        }
    };

    ProcessedJsonRpcMessage {
        request: Some(request),
        response: Some(response),
    }
}

fn handler_panicked_error(method: &str) -> JsonRpcErrorObject {
    JsonRpcErrorObject::internal_error("Internal error").with_data(json!({
        "kind": "handler_panicked",
        "method": method,
    }))
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<unprintable panic payload>".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundQueueError {
    Backpressure,
    Closed,
}

pub fn enqueue_outbound_message(
    outbound_tx: &mpsc::Sender<JsonRpcMessage>,
    is_open: &Arc<AtomicBool>,
    message: JsonRpcMessage,
) -> Result<(), OutboundQueueError> {
    match outbound_tx.try_send(message) {
        Ok(()) => Ok(()),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            is_open.store(false, Ordering::SeqCst);
            Err(OutboundQueueError::Backpressure)
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err(OutboundQueueError::Closed),
    }
}

#[derive(Debug)]
pub struct JsonRpcConnectionRuntime {
    session: JsonRpcServerSession,
    poll_controller: PersistentPollController,
}

impl JsonRpcConnectionRuntime {
    pub fn new(
        connection_id: usize,
        outbound_queue_depth: usize,
    ) -> (Self, mpsc::Receiver<JsonRpcMessage>) {
        let (outbound_tx, outbound_rx) = mpsc::channel(outbound_queue_depth);
        let is_open = Arc::new(AtomicBool::new(true));
        let session =
            JsonRpcServerSession::new(connection_id, outbound_tx.clone(), Arc::clone(&is_open));
        (
            Self {
                session,
                poll_controller: PersistentPollController::default(),
            },
            outbound_rx,
        )
    }

    pub fn session(&self) -> JsonRpcServerSession {
        self.session.clone()
    }

    pub fn is_open(&self) -> bool {
        self.session.is_open()
    }

    pub fn open_state(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.session.is_open)
    }

    pub fn close(&self) {
        self.session.close();
    }

    pub fn current_timeout(&self, request_timeout: Duration) -> Duration {
        self.poll_controller.current_timeout(request_timeout)
    }

    pub fn request_timeout_armed(&self) -> bool {
        self.poll_controller.request_timeout_armed()
    }

    pub fn observe_message(
        &mut self,
        message: &ProcessedJsonRpcMessage,
        persistent_request_method: Option<&str>,
    ) {
        self.poll_controller
            .observe_message(message, persistent_request_method);
    }

    pub fn enqueue_message(&self, message: JsonRpcMessage) -> Result<(), OutboundQueueError> {
        enqueue_outbound_message(&self.session.outbound_tx, &self.session.is_open, message)
    }

    pub fn take_after_response_actions(&self) -> Vec<JsonRpcAfterResponseAction> {
        self.session.take_after_response_actions()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonRpcConnectionLoopEvent<T> {
    Message(T),
    Timeout,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonRpcConnectionLoopOutcome {
    pub saw_message: bool,
}

#[allow(async_fn_in_trait)]
pub trait JsonRpcConnectionAdapter {
    type Message;
    type Error;

    async fn drain_outbound(&mut self) -> Result<(), Self::Error>;

    async fn read_next(
        &mut self,
        timeout: Duration,
        request_timeout_armed: bool,
    ) -> Result<JsonRpcConnectionLoopEvent<Self::Message>, Self::Error>;

    async fn process_message(
        &mut self,
        message: Self::Message,
    ) -> Result<ProcessedJsonRpcMessage, Self::Error>;

    fn map_outbound_error(&self, error: OutboundQueueError) -> Self::Error;
}

pub async fn run_jsonrpc_connection_loop<A>(
    connection_runtime: &mut JsonRpcConnectionRuntime,
    request_timeout: Duration,
    persistent_request_method: Option<&str>,
    adapter: &mut A,
) -> Result<JsonRpcConnectionLoopOutcome, A::Error>
where
    A: JsonRpcConnectionAdapter,
{
    let mut saw_message = false;

    loop {
        if !connection_runtime.is_open() {
            break;
        }

        adapter.drain_outbound().await?;
        let event = adapter
            .read_next(
                connection_runtime.current_timeout(request_timeout),
                connection_runtime.request_timeout_armed(),
            )
            .await?;

        let processed = match event {
            JsonRpcConnectionLoopEvent::Timeout => continue,
            JsonRpcConnectionLoopEvent::Closed => break,
            JsonRpcConnectionLoopEvent::Message(message) => {
                saw_message = true;
                adapter.process_message(message).await?
            }
        };
        let after_response_actions = connection_runtime.take_after_response_actions();

        connection_runtime.observe_message(&processed, persistent_request_method);
        let is_success_response = matches!(processed.response, Some(JsonRpcMessage::Response(_)));
        let Some(response) = processed.response else {
            continue;
        };

        connection_runtime
            .enqueue_message(response)
            .map_err(|error| adapter.map_outbound_error(error))?;
        adapter.drain_outbound().await?;
        if is_success_response {
            run_after_response_actions(after_response_actions, processed.request.as_ref());
        }
    }

    connection_runtime.close();
    Ok(JsonRpcConnectionLoopOutcome { saw_message })
}

fn run_after_response_actions(
    actions: Vec<JsonRpcAfterResponseAction>,
    request: Option<&JsonRpcRequest>,
) {
    for action in actions {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(action)) {
            let panic_message = panic_payload_to_string(payload);
            if let Some(request) = request {
                tracing::error!(
                    rpc.method = %request.method,
                    rpc.request_id = %request.id,
                    panic.message = %panic_message,
                    "JSON-RPC after-response action panicked"
                );
            } else {
                tracing::error!(
                    panic.message = %panic_message,
                    "JSON-RPC after-response action panicked"
                );
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentPollController {
    request_timeout_armed: bool,
    persistent_poll_interval: Duration,
}

impl Default for PersistentPollController {
    fn default() -> Self {
        Self {
            request_timeout_armed: true,
            persistent_poll_interval: DEFAULT_PERSISTENT_CONNECTION_POLL_INTERVAL,
        }
    }
}

impl PersistentPollController {
    pub fn current_timeout(&self, request_timeout: Duration) -> Duration {
        if self.request_timeout_armed {
            request_timeout
        } else {
            self.persistent_poll_interval
        }
    }

    pub fn request_timeout_armed(&self) -> bool {
        self.request_timeout_armed
    }

    pub fn observe_message(
        &mut self,
        message: &ProcessedJsonRpcMessage,
        persistent_request_method: Option<&str>,
    ) {
        if self.request_timeout_armed
            && should_enter_persistent_poll_mode(message, persistent_request_method)
        {
            self.request_timeout_armed = false;
        }
    }
}

pub fn should_enter_persistent_poll_mode(
    message: &ProcessedJsonRpcMessage,
    persistent_request_method: Option<&str>,
) -> bool {
    let Some(persistent_request_method) = persistent_request_method else {
        return false;
    };

    message
        .request
        .as_ref()
        .is_some_and(|request| request.method == persistent_request_method)
        && matches!(message.response, Some(JsonRpcMessage::Response(_)))
}

#[cfg(test)]
mod tests;
