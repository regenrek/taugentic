use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader},
    time,
};

use crate::{
    DEFAULT_OUTBOUND_QUEUE_DEPTH, INTERNAL_ERROR_CODE, INVALID_PARAMS_ERROR_CODE, JsonLineCodec,
    JsonLineCodecError, JsonRpcConnectionAdapter, JsonRpcConnectionLoopEvent,
    JsonRpcConnectionRuntime, JsonRpcError, JsonRpcErrorObject, JsonRpcHandlerFuture,
    JsonRpcHandlerResult, JsonRpcMessage, JsonRpcRequest, JsonRpcRequestHandler,
    JsonRpcRequestProcessingContext, JsonRpcServerSession, METHOD_NOT_FOUND_ERROR_CODE,
    OutboundQueueError, ProcessedJsonRpcMessage, SocketAddress, SocketIoError,
    TokioSocketConnection, bind_listener_tokio, process_jsonrpc_request,
    run_jsonrpc_connection_loop,
};

const DEFAULT_SERVER_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub service_name: String,
    pub socket_address: SocketAddress,
    pub max_in_flight_requests: usize,
    pub io_timeout: Duration,
}

impl ServerConfig {
    pub fn local_default(service_name: &str, app_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            socket_address: SocketAddress::for_current_user(app_name),
            max_in_flight_requests: 128,
            io_timeout: DEFAULT_SERVER_IO_TIMEOUT,
        }
    }
}

pub const JSON_RPC_METHOD_NOT_FOUND: i64 = METHOD_NOT_FOUND_ERROR_CODE;
pub const JSON_RPC_INVALID_PARAMS: i64 = INVALID_PARAMS_ERROR_CODE;
pub const JSON_RPC_INTERNAL_ERROR: i64 = INTERNAL_ERROR_CODE;

type JsonRpcConnectionHandler = JsonRpcRequestHandler;
type JsonRpcConnectionHandlerFactory =
    dyn Fn(JsonRpcServerSession) -> Box<JsonRpcConnectionHandler> + Send + Sync + 'static;

struct LocalSocketConnectionAdapter<'a> {
    reader: &'a mut TokioBufReader<tokio::io::ReadHalf<TokioSocketConnection>>,
    request_line: &'a mut String,
    handler: &'a JsonRpcConnectionHandler,
    service_name: &'a str,
    connection_id: usize,
    io_timeout: Duration,
}

impl JsonRpcConnectionAdapter for LocalSocketConnectionAdapter<'_> {
    type Message = String;
    type Error = JsonRpcServerError;

    async fn drain_outbound(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn read_next(
        &mut self,
        timeout: Duration,
        request_timeout_armed: bool,
    ) -> Result<JsonRpcConnectionLoopEvent<Self::Message>, Self::Error> {
        self.request_line.clear();
        let bytes_read =
            match time::timeout(timeout, self.reader.read_line(self.request_line)).await {
                Ok(Ok(bytes_read)) => bytes_read,
                Ok(Err(error)) => {
                    return Err(map_read_error(
                        self.io_timeout,
                        request_timeout_armed,
                        error,
                    ));
                }
                Err(_) if !request_timeout_armed => return Ok(JsonRpcConnectionLoopEvent::Timeout),
                Err(_) => {
                    return Err(JsonRpcServerError::RequestTimeout {
                        timeout: self.io_timeout,
                        source: io::Error::new(
                            io::ErrorKind::TimedOut,
                            "timed out waiting for client request",
                        ),
                    });
                }
            };
        if bytes_read == 0 {
            return Ok(JsonRpcConnectionLoopEvent::Closed);
        }
        Ok(JsonRpcConnectionLoopEvent::Message(
            self.request_line.clone(),
        ))
    }

    async fn process_message(
        &mut self,
        message: Self::Message,
    ) -> Result<ProcessedJsonRpcMessage, Self::Error> {
        Ok(process_connection_message(
            self.handler,
            self.service_name,
            self.connection_id,
            &message,
        )
        .await)
    }

    fn map_outbound_error(&self, error: OutboundQueueError) -> Self::Error {
        map_outbound_queue_error(error)
    }
}

#[derive(Debug, Error)]
pub enum JsonRpcServerError {
    #[error(transparent)]
    Socket(#[from] SocketIoError),
    #[error(transparent)]
    Codec(#[from] JsonLineCodecError),
    #[error("failed to read request from client: {0}")]
    Read(#[source] std::io::Error),
    #[error("failed to configure client socket deadlines: {0}")]
    ConfigureTimeout(#[source] std::io::Error),
    #[error("timed out waiting {timeout:?} for client request")]
    RequestTimeout {
        timeout: Duration,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write response to client: {0}")]
    Write(#[source] std::io::Error),
    #[error("failed to flush response to client: {0}")]
    Flush(#[source] std::io::Error),
    #[error("failed to clone client stream for bidirectional session IO: {0}")]
    CloneStream(#[source] std::io::Error),
    #[error("failed to send outbound JSON-RPC message: connection already closed")]
    ConnectionClosed,
    #[error("json-rpc outbound queue overflowed for slow connection")]
    OutboundBackpressure,
    #[error("json-rpc writer thread panicked")]
    WriterPanicked,
}

#[derive(Clone)]
pub struct JsonRpcServer {
    config: ServerConfig,
    codec: JsonLineCodec,
    handler_factory: Arc<JsonRpcConnectionHandlerFactory>,
    persistent_request_method: Option<String>,
}

impl JsonRpcServer {
    pub fn new<F>(config: ServerConfig, handler_factory: F) -> Self
    where
        F: Fn(JsonRpcServerSession) -> Box<JsonRpcConnectionHandler> + Send + Sync + 'static,
    {
        Self {
            config,
            codec: JsonLineCodec,
            handler_factory: Arc::new(handler_factory),
            persistent_request_method: None,
        }
    }

    pub fn new_stateless<F>(config: ServerConfig, handler: F) -> Self
    where
        F: Fn(JsonRpcRequest) -> JsonRpcHandlerResult + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        Self::new(config, move |_| {
            let handler = Arc::clone(&handler);
            Box::new(move |request| {
                let result = handler(request);
                Box::pin(async move { result }) as JsonRpcHandlerFuture
            })
        })
    }

    pub fn with_persistent_request_method(mut self, method: impl Into<String>) -> Self {
        self.persistent_request_method = Some(method.into());
        self
    }

    pub async fn serve(&self) -> Result<(), JsonRpcServerError> {
        self.serve_until(Arc::new(std::sync::atomic::AtomicBool::new(false)))
            .await
    }

    pub async fn serve_until(
        &self,
        shutdown_requested: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), JsonRpcServerError> {
        let listener = bind_listener_tokio(&self.config.socket_address)?;
        let in_flight = Arc::new(AtomicUsize::new(0));
        let next_connection_id = Arc::new(AtomicUsize::new(1));

        tracing::info!(
            socket.address = %self.config.socket_address,
            server.max_in_flight_requests = self.config.max_in_flight_requests,
            "json-rpc server listening"
        );

        loop {
            if shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
                tracing::info!("json-rpc server shutdown requested");
                return Ok(());
            }

            let stream = match listener.accept().await {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::error!(error = %error, "failed to accept json-rpc connection");
                    return Err(error.into());
                }
            };
            if shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
                tracing::info!("json-rpc server shutdown accepted wake connection");
                return Ok(());
            }
            let handler_factory = Arc::clone(&self.handler_factory);
            let in_flight = Arc::clone(&in_flight);
            let codec = self.codec;
            let service_name = self.config.service_name.clone();
            let max_in_flight_requests = self.config.max_in_flight_requests;
            let io_timeout = self.config.io_timeout;
            let persistent_request_method = self.persistent_request_method.clone();
            let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed);

            if in_flight.fetch_add(1, Ordering::SeqCst) >= max_in_flight_requests {
                in_flight.fetch_sub(1, Ordering::SeqCst);
                let _ = reject_overloaded_connection_async(
                    codec,
                    stream,
                    connection_id,
                    max_in_flight_requests,
                )
                .await;
                continue;
            }

            tokio::spawn(async move {
                if let Err(error) = handle_connection(
                    codec,
                    stream,
                    &handler_factory,
                    service_name.as_str(),
                    connection_id,
                    io_timeout,
                    persistent_request_method.as_deref(),
                )
                .await
                {
                    match error {
                        JsonRpcServerError::RequestTimeout { timeout, .. } => {
                            tracing::warn!(
                                taugentic.connection_id = connection_id as u64,
                                timeout_ms = timeout.as_millis() as u64,
                                "json-rpc client timed out before sending a request"
                            );
                        }
                        other => {
                            tracing::error!(
                                taugentic.connection_id = connection_id as u64,
                                error = %other,
                                "json-rpc connection failed"
                            );
                        }
                    }
                }
                in_flight.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }

    pub async fn serve_once(&self) -> Result<(), JsonRpcServerError> {
        let listener = bind_listener_tokio(&self.config.socket_address)?;
        let stream = listener.accept().await?;
        handle_connection(
            self.codec,
            stream,
            &self.handler_factory,
            self.config.service_name.as_str(),
            1,
            self.config.io_timeout,
            self.persistent_request_method.as_deref(),
        )
        .await
    }
}

pub fn method_not_found(method: &str) -> JsonRpcErrorObject {
    JsonRpcErrorObject::method_not_found(method)
}

pub fn invalid_params(message: impl Into<String>) -> JsonRpcErrorObject {
    JsonRpcErrorObject::invalid_params(message)
}

pub fn internal_error(message: impl Into<String>) -> JsonRpcErrorObject {
    JsonRpcErrorObject::internal_error(message)
}

pub fn parse_params<T>(request: &JsonRpcRequest) -> Result<T, JsonRpcErrorObject>
where
    T: serde::de::DeserializeOwned,
{
    let params = request
        .params
        .clone()
        .unwrap_or(Value::Object(Default::default()));
    serde_json::from_value(params).map_err(|error| invalid_params(error.to_string()))
}

async fn handle_connection(
    codec: JsonLineCodec,
    stream: TokioSocketConnection,
    handler_factory: &Arc<JsonRpcConnectionHandlerFactory>,
    service_name: &str,
    connection_id: usize,
    io_timeout: Duration,
    persistent_request_method: Option<&str>,
) -> Result<(), JsonRpcServerError> {
    let (mut connection_runtime, mut outbound_rx) =
        JsonRpcConnectionRuntime::new(connection_id, DEFAULT_OUTBOUND_QUEUE_DEPTH);
    let handler = handler_factory(connection_runtime.session());
    let (reader_stream, mut writer_stream) = tokio::io::split(stream);
    let writer_is_open = connection_runtime.open_state();
    let writer_handle = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let line = codec.encode_message(&message)?;
            writer_stream
                .write_all(line.as_bytes())
                .await
                .map_err(JsonRpcServerError::Write)?;
            writer_stream
                .flush()
                .await
                .map_err(JsonRpcServerError::Flush)?;
        }
        writer_is_open.store(false, Ordering::SeqCst);
        Ok::<(), JsonRpcServerError>(())
    });
    let mut reader = TokioBufReader::new(reader_stream);
    let mut request_line = String::new();
    let mut adapter = LocalSocketConnectionAdapter {
        reader: &mut reader,
        request_line: &mut request_line,
        handler: &*handler,
        service_name,
        connection_id,
        io_timeout,
    };
    let outcome = run_jsonrpc_connection_loop(
        &mut connection_runtime,
        io_timeout,
        persistent_request_method,
        &mut adapter,
    )
    .await?;

    if !outcome.saw_message {
        tracing::debug!(
            taugentic.connection_id = connection_id as u64,
            "json-rpc connection closed without a request"
        );
    }

    drop(handler);
    drop(connection_runtime);
    match writer_handle.await {
        Ok(result) => result,
        Err(_) => Err(JsonRpcServerError::WriterPanicked),
    }
}

#[cfg(test)]
fn enqueue_outbound_message(
    outbound_tx: &tokio::sync::mpsc::Sender<JsonRpcMessage>,
    is_open: &Arc<std::sync::atomic::AtomicBool>,
    message: JsonRpcMessage,
) -> Result<(), JsonRpcServerError> {
    crate::enqueue_outbound_message(outbound_tx, is_open, message).map_err(map_outbound_queue_error)
}

fn map_outbound_queue_error(error: OutboundQueueError) -> JsonRpcServerError {
    match error {
        OutboundQueueError::Backpressure => JsonRpcServerError::OutboundBackpressure,
        OutboundQueueError::Closed => JsonRpcServerError::ConnectionClosed,
    }
}

async fn reject_overloaded_connection_async(
    codec: JsonLineCodec,
    stream: TokioSocketConnection,
    connection_id: usize,
    max_in_flight_requests: usize,
) -> Result<(), JsonRpcServerError> {
    let mut request_line = String::new();
    let mut reader = TokioBufReader::new(stream);
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|error| map_read_error(DEFAULT_SERVER_IO_TIMEOUT, true, error))?;

    if request_line.is_empty() {
        tracing::debug!(
            taugentic.connection_id = connection_id as u64,
            "json-rpc overload connection closed before sending a request"
        );
        return Ok(());
    }

    let response = overload_response_for_request_line(
        codec,
        &request_line,
        connection_id,
        max_in_flight_requests,
    );
    let mut stream = reader.into_inner();
    let line = codec.encode_message(&response)?;
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(JsonRpcServerError::Write)?;
    stream.flush().await.map_err(JsonRpcServerError::Flush)?;
    Ok(())
}

fn overload_response_for_request_line(
    codec: JsonLineCodec,
    request_line: &str,
    connection_id: usize,
    max_in_flight_requests: usize,
) -> JsonRpcMessage {
    match codec.decode_message(request_line) {
        Ok(JsonRpcMessage::Request(request)) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                server.max_in_flight_requests = max_in_flight_requests,
                rpc.method = %request.method,
                rpc.request_id = %request.id,
                "rejecting JSON-RPC request because the in-flight limit was reached"
            );
            JsonRpcMessage::Error(JsonRpcError::new(
                Some(request.id),
                internal_error("too many in-flight JSON-RPC requests"),
            ))
        }
        Ok(JsonRpcMessage::Notification(notification)) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                server.max_in_flight_requests = max_in_flight_requests,
                rpc.method = %notification.method,
                "rejected overloaded JSON-RPC notification"
            );
            JsonRpcMessage::Error(JsonRpcError::new(
                None,
                JsonRpcErrorObject::invalid_request(format!(
                    "notifications are not supported: {}",
                    notification.method
                )),
            ))
        }
        Ok(other) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                server.max_in_flight_requests = max_in_flight_requests,
                message.kind = ?other,
                "expected a JSON-RPC request while rejecting overloaded connection"
            );
            JsonRpcMessage::Error(JsonRpcError::new(
                None,
                JsonRpcErrorObject::invalid_request("expected a JSON-RPC request"),
            ))
        }
        Err(error) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                server.max_in_flight_requests = max_in_flight_requests,
                error = %error,
                "failed to decode overloaded JSON-RPC request"
            );
            JsonRpcMessage::Error(JsonRpcError::new(
                None,
                JsonRpcErrorObject::parse_error(error.to_string()),
            ))
        }
    }
}

async fn process_connection_message(
    handler: &JsonRpcConnectionHandler,
    service_name: &str,
    connection_id: usize,
    request_line: &str,
) -> ProcessedJsonRpcMessage {
    match JsonLineCodec.decode_message(request_line) {
        Ok(JsonRpcMessage::Request(request)) => {
            process_jsonrpc_request(
                handler,
                JsonRpcRequestProcessingContext {
                    service_name,
                    transport: "local_socket",
                    connection_id,
                },
                request,
            )
            .await
        }
        Ok(JsonRpcMessage::Notification(notification)) => {
            tracing::debug!(
                taugentic.connection_id = connection_id as u64,
                rpc.method = %notification.method,
                "received client JSON-RPC notification"
            );
            ProcessedJsonRpcMessage {
                request: None,
                response: None,
            }
        }
        Ok(other) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                message.kind = ?other,
                "expected a JSON-RPC request"
            );
            ProcessedJsonRpcMessage {
                request: None,
                response: Some(JsonRpcMessage::Error(JsonRpcError::new(
                    None,
                    JsonRpcErrorObject::invalid_request("expected a JSON-RPC request"),
                ))),
            }
        }
        Err(error) => {
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                error = %error,
                "failed to decode JSON-RPC request"
            );
            ProcessedJsonRpcMessage {
                request: None,
                response: Some(JsonRpcMessage::Error(JsonRpcError::new(
                    None,
                    JsonRpcErrorObject::parse_error(error.to_string()),
                ))),
            }
        }
    }
}

fn map_read_error(
    timeout: Duration,
    request_timeout_armed: bool,
    error: io::Error,
) -> JsonRpcServerError {
    if request_timeout_armed && is_timeout_error(&error) {
        return JsonRpcServerError::RequestTimeout {
            timeout,
            source: error,
        };
    }

    JsonRpcServerError::Read(error)
}

fn is_timeout_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

#[cfg(test)]
mod tests;
