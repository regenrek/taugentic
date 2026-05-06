use std::io;
use std::net::SocketAddr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use ta_jsonrpc::{
    DEFAULT_OUTBOUND_QUEUE_DEPTH, JsonRpcConnectionAdapter, JsonRpcConnectionLoopEvent,
    JsonRpcConnectionRuntime, JsonRpcRequestProcessingContext, OutboundQueueError,
    ProcessedJsonRpcMessage, process_jsonrpc_request, run_jsonrpc_connection_loop,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream},
    sync::mpsc,
    time,
};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};
use tungstenite::Message;
use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;

#[cfg(test)]
use crate::JsonRpcRequest;
use crate::{
    INVALID_REQUEST_ERROR_CODE, JsonLineCodec, JsonRpcError, JsonRpcErrorObject, JsonRpcMessage,
    JsonRpcRequestHandler, METHOD_DAEMON_INITIALIZE, host::bootstrap::BootstrapState,
    host::config::RemoteWebsocketConfig, host::rpc::make_session_handler,
};

const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum RemoteWebsocketServerError {
    #[error("failed to bind remote websocket listener on {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("failed to configure remote websocket listener: {0}")]
    ConfigureListener(#[source] io::Error),
    #[error("failed to accept remote websocket connection: {0}")]
    Accept(#[source] io::Error),
    #[error("failed to configure remote websocket connection deadlines: {0}")]
    ConfigureTimeout(#[source] io::Error),
    #[error("remote websocket handshake failed: {0}")]
    Handshake(String),
    #[error("failed to read remote websocket frame: {0}")]
    Read(#[source] tungstenite::Error),
    #[error("failed to write remote websocket frame: {0}")]
    Write(#[source] tungstenite::Error),
    #[error("timed out waiting {timeout:?} for remote client request")]
    RequestTimeout {
        timeout: Duration,
        #[source]
        source: io::Error,
    },
    #[error("remote websocket outbound queue overflowed for slow connection")]
    OutboundBackpressure,
    #[error("remote websocket connection closed")]
    ConnectionClosed,
}

struct RemoteWebsocketConnectionAdapter<'a> {
    websocket: &'a mut WebSocketStream<TokioTcpStream>,
    outbound_rx: &'a mut mpsc::Receiver<JsonRpcMessage>,
    handler: &'a JsonRpcRequestHandler,
    connection_id: usize,
}

impl JsonRpcConnectionAdapter for RemoteWebsocketConnectionAdapter<'_> {
    type Message = Message;
    type Error = RemoteWebsocketServerError;

    async fn drain_outbound(&mut self) -> Result<(), Self::Error> {
        drain_outbound_messages(self.websocket, self.outbound_rx).await
    }

    async fn read_next(
        &mut self,
        timeout: Duration,
        request_timeout_armed: bool,
    ) -> Result<JsonRpcConnectionLoopEvent<Self::Message>, Self::Error> {
        Ok(
            match read_frame(self.websocket, timeout, request_timeout_armed).await? {
                ReadFrame::Timeout => JsonRpcConnectionLoopEvent::Timeout,
                ReadFrame::Closed => JsonRpcConnectionLoopEvent::Closed,
                ReadFrame::Message(message) => JsonRpcConnectionLoopEvent::Message(message),
            },
        )
    }

    async fn process_message(
        &mut self,
        message: Self::Message,
    ) -> Result<ProcessedJsonRpcMessage, Self::Error> {
        match message {
            Message::Text(payload) => {
                process_text_message(self.handler, self.connection_id, payload.as_str()).await
            }
            Message::Binary(_) => Ok(ProcessedJsonRpcMessage {
                request: None,
                response: Some(JsonRpcMessage::Error(JsonRpcError::new(
                    None,
                    JsonRpcErrorObject::invalid_request("expected a text JSON-RPC request"),
                ))),
            }),
            Message::Ping(payload) => {
                send_immediate_frame(self.websocket, Message::Pong(payload)).await?;
                Ok(ProcessedJsonRpcMessage {
                    request: None,
                    response: None,
                })
            }
            Message::Pong(_) | Message::Close(_) => Ok(ProcessedJsonRpcMessage {
                request: None,
                response: None,
            }),
            _ => Ok(ProcessedJsonRpcMessage {
                request: None,
                response: None,
            }),
        }
    }

    fn map_outbound_error(&self, error: OutboundQueueError) -> Self::Error {
        map_outbound_queue_error(error)
    }
}

pub async fn serve_remote_until(
    state: BootstrapState,
    shutdown_requested: Arc<AtomicBool>,
) -> Result<(), RemoteWebsocketServerError> {
    let Some(remote) = state.config.remote.clone() else {
        return Ok(());
    };

    let listener = TokioTcpListener::bind(remote.bind_address)
        .await
        .map_err(|source| RemoteWebsocketServerError::Bind {
            address: remote.bind_address,
            source,
        })?;
    let in_flight = Arc::new(AtomicUsize::new(0));
    let next_connection_id = Arc::new(AtomicUsize::new(1));

    tracing::info!(
        remote.websocket.address = %remote.bind_address,
        remote.websocket.path = %remote.path,
        server.max_in_flight_requests = state.config.server.max_in_flight_requests,
        "remote websocket server listening"
    );

    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            tracing::info!("remote websocket server shutdown requested");
            return Ok(());
        }

        let (stream, peer_address) =
            match time::timeout(ACCEPT_POLL_INTERVAL, listener.accept()).await {
                Ok(Ok(connection)) => connection,
                Ok(Err(error)) => return Err(RemoteWebsocketServerError::Accept(error)),
                Err(_) => continue,
            };
        let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed);
        if in_flight.fetch_add(1, Ordering::SeqCst) >= state.config.server.max_in_flight_requests {
            in_flight.fetch_sub(1, Ordering::SeqCst);
            tracing::warn!(
                taugentic.connection_id = connection_id as u64,
                remote.peer = %peer_address,
                server.max_in_flight_requests = state.config.server.max_in_flight_requests,
                "dropping remote websocket connection because the in-flight limit was reached"
            );
            continue;
        }

        let state = state.clone();
        let remote = remote.clone();
        let shutdown_requested = Arc::clone(&shutdown_requested);
        let in_flight = Arc::clone(&in_flight);
        tokio::spawn(async move {
            let result = handle_remote_connection(
                stream,
                &state,
                &remote,
                Arc::clone(&shutdown_requested),
                connection_id,
            )
            .await;
            if let Err(error) = result {
                tracing::error!(
                    taugentic.connection_id = connection_id as u64,
                    remote.peer = %peer_address,
                    error = %error,
                    "remote websocket connection failed"
                )
            }
            in_flight.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

async fn handle_remote_connection(
    stream: TokioTcpStream,
    state: &BootstrapState,
    remote: &RemoteWebsocketConfig,
    shutdown_requested: Arc<AtomicBool>,
    connection_id: usize,
) -> Result<(), RemoteWebsocketServerError> {
    stream
        .set_nodelay(true)
        .map_err(RemoteWebsocketServerError::ConfigureTimeout)?;
    let websocket = accept_hdr_async(stream, AuthCallback::new(remote.clone()))
        .await
        .map_err(|error| RemoteWebsocketServerError::Handshake(error.to_string()))?;
    let mut websocket = websocket;
    let (mut connection_runtime, mut outbound_rx) =
        JsonRpcConnectionRuntime::new(connection_id, DEFAULT_OUTBOUND_QUEUE_DEPTH);
    let handler = make_session_handler(
        state.clone(),
        shutdown_requested,
        connection_runtime.session(),
    );
    let mut adapter = RemoteWebsocketConnectionAdapter {
        websocket: &mut websocket,
        outbound_rx: &mut outbound_rx,
        handler: &*handler,
        connection_id,
    };

    run_jsonrpc_connection_loop(
        &mut connection_runtime,
        state.config.server.io_timeout,
        Some(METHOD_DAEMON_INITIALIZE),
        &mut adapter,
    )
    .await?;

    drop(handler);
    drop(connection_runtime);
    Ok(())
}

async fn read_frame(
    websocket: &mut WebSocketStream<TokioTcpStream>,
    timeout: Duration,
    request_timeout_armed: bool,
) -> Result<ReadFrame, RemoteWebsocketServerError> {
    match time::timeout(timeout, websocket.next()).await {
        Ok(Some(Ok(message))) => Ok(ReadFrame::Message(message)),
        Ok(Some(Err(tungstenite::Error::Io(error))))
            if is_timeout_error(&error) && request_timeout_armed =>
        {
            Err(RemoteWebsocketServerError::RequestTimeout {
                timeout,
                source: error,
            })
        }
        Ok(Some(Err(tungstenite::Error::Io(error)))) if is_timeout_error(&error) => {
            Ok(ReadFrame::Timeout)
        }
        Ok(Some(Err(tungstenite::Error::Protocol(
            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
        )))) => Ok(ReadFrame::Closed),
        Ok(Some(Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed)))
        | Ok(None) => Ok(ReadFrame::Closed),
        Ok(Some(Err(error))) => Err(RemoteWebsocketServerError::Read(error)),
        Err(_) if !request_timeout_armed => Ok(ReadFrame::Timeout),
        Err(_) => Err(RemoteWebsocketServerError::RequestTimeout {
            timeout,
            source: io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for remote client request",
            ),
        }),
    }
}

async fn process_text_message(
    handler: &JsonRpcRequestHandler,
    connection_id: usize,
    payload: &str,
) -> Result<ProcessedJsonRpcMessage, RemoteWebsocketServerError> {
    let codec = JsonLineCodec;
    Ok(match codec.decode_message(payload) {
        Ok(JsonRpcMessage::Request(request)) => {
            process_jsonrpc_request(
                handler,
                JsonRpcRequestProcessingContext {
                    service_name: "ta-daemon",
                    transport: "websocket",
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
                "received remote websocket JSON-RPC notification"
            );
            ProcessedJsonRpcMessage {
                request: None,
                response: None,
            }
        }
        Ok(other) => ProcessedJsonRpcMessage {
            request: None,
            response: Some(JsonRpcMessage::Error(JsonRpcError::new(
                None,
                JsonRpcErrorObject::new(
                    INVALID_REQUEST_ERROR_CODE,
                    format!("expected a JSON-RPC request, got {other:?}"),
                ),
            ))),
        },
        Err(error) => ProcessedJsonRpcMessage {
            request: None,
            response: Some(JsonRpcMessage::Error(JsonRpcError::new(
                None,
                JsonRpcErrorObject::parse_error(format!(
                    "failed to decode JSON-RPC message: {error}"
                )),
            ))),
        },
    })
}

fn map_outbound_queue_error(error: OutboundQueueError) -> RemoteWebsocketServerError {
    match error {
        OutboundQueueError::Backpressure => RemoteWebsocketServerError::OutboundBackpressure,
        OutboundQueueError::Closed => RemoteWebsocketServerError::ConnectionClosed,
    }
}

async fn drain_outbound_messages(
    websocket: &mut WebSocketStream<TokioTcpStream>,
    outbound_rx: &mut mpsc::Receiver<JsonRpcMessage>,
) -> Result<(), RemoteWebsocketServerError> {
    while let Ok(message) = outbound_rx.try_recv() {
        write_jsonrpc_message(websocket, &message).await?;
    }
    Ok(())
}

async fn write_jsonrpc_message(
    websocket: &mut WebSocketStream<TokioTcpStream>,
    message: &JsonRpcMessage,
) -> Result<(), RemoteWebsocketServerError> {
    let payload = JsonLineCodec
        .encode_message(message)
        .map_err(|error| RemoteWebsocketServerError::Handshake(error.to_string()))?;
    send_immediate_frame(
        websocket,
        Message::Text(payload.trim_end().to_string().into()),
    )
    .await
}

async fn send_immediate_frame(
    websocket: &mut WebSocketStream<TokioTcpStream>,
    message: Message,
) -> Result<(), RemoteWebsocketServerError> {
    websocket
        .send(message)
        .await
        .map_err(RemoteWebsocketServerError::Write)
}

fn is_timeout_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

#[derive(Debug)]
enum ReadFrame {
    Message(Message),
    Timeout,
    Closed,
}

#[derive(Clone)]
struct AuthCallback {
    config: RemoteWebsocketConfig,
}

impl AuthCallback {
    fn new(config: RemoteWebsocketConfig) -> Self {
        Self { config }
    }
}

impl Callback for AuthCallback {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        if request.uri().path() != self.config.path {
            return Err(handshake_error_response(
                StatusCode::NOT_FOUND,
                "remote websocket endpoint not found",
            ));
        }

        let authorization = request
            .headers()
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        let expected = format!("Bearer {}", self.config.auth_token.as_str());
        if authorization != Some(expected.as_str()) {
            return Err(handshake_error_response(
                StatusCode::UNAUTHORIZED,
                "missing or invalid bearer token",
            ));
        }

        Ok(response)
    }
}

fn handshake_error_response(status: StatusCode, message: &str) -> ErrorResponse {
    tungstenite::http::Response::builder()
        .status(status)
        .body(Some(message.to_string()))
        .expect("error response should build")
}

#[cfg(test)]
mod tests;
