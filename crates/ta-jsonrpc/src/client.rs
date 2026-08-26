use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

use serde::{Serialize, de::DeserializeOwned};
use ta_observability::rpc_client_request_span;
use thiserror::Error;

use crate::{
    JsonLineCodec, JsonLineCodecError, JsonRpcError, JsonRpcMessage, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, RequestId, SocketAddress, SocketIoError,
    configure_connection_timeouts, connect_socket, connect_socket_tokio,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader},
    sync::{mpsc as tokio_mpsc, oneshot},
};

const DEFAULT_CLIENT_IO_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub service_name: String,
    pub socket_address: SocketAddress,
    pub io_timeout: Duration,
}

impl ClientConfig {
    pub fn local_default(service_name: &str, app_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            socket_address: SocketAddress::for_current_user(app_name),
            io_timeout: DEFAULT_CLIENT_IO_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonRpcClient {
    config: ClientConfig,
    codec: JsonLineCodec,
    next_request_id: Arc<AtomicI64>,
}

/// A single persistent JSON-RPC connection which safely separates replies from
/// notifications. Transport framing and request correlation live here; callers
/// retain ownership of the meaning of notifications and any durable state.
#[derive(Debug, Clone)]
pub struct PersistentJsonRpcClient {
    inner: Arc<PersistentClientInner>,
}

#[derive(Debug)]
struct PersistentClientInner {
    commands: tokio_mpsc::UnboundedSender<ActorCommand>,
    next_request_id: AtomicI64,
    next_subscription_id: AtomicU64,
    closed: AtomicBool,
}

enum ActorCommand {
    Request {
        request: JsonRpcRequest,
        response: oneshot::Sender<Result<JsonRpcResponse, JsonRpcClientError>>,
    },
    Subscribe {
        id: u64,
        sender: SyncSender<Result<JsonRpcNotification, NotificationReceiveError>>,
        terminal: Arc<Mutex<Option<NotificationReceiveError>>>,
        registered: Arc<AtomicBool>,
    },
    Unsubscribe {
        id: u64,
    },
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NotificationReceiveError {
    #[error("persistent JSON-RPC connection closed")]
    ConnectionClosed,
    #[error("notification subscriber fell behind the daemon stream")]
    Backpressure,
}

pub struct JsonRpcNotificationSubscription {
    id: u64,
    receiver: Receiver<Result<JsonRpcNotification, NotificationReceiveError>>,
    terminal: Arc<Mutex<Option<NotificationReceiveError>>>,
    commands: tokio_mpsc::UnboundedSender<ActorCommand>,
    registered: Arc<AtomicBool>,
}

impl JsonRpcNotificationSubscription {
    pub fn recv(&self) -> Result<JsonRpcNotification, NotificationReceiveError> {
        if let Some(error) = *self
            .terminal
            .lock()
            .expect("notification terminal lock poisoned")
        {
            return Err(error);
        }
        self.receiver
            .recv()
            .unwrap_or(Err(NotificationReceiveError::ConnectionClosed))
    }
}

impl Drop for JsonRpcNotificationSubscription {
    fn drop(&mut self) {
        // The actor is the sole owner of registrations. Dropping the public
        // handle releases its registration even if no further notification is
        // ever received (for example after a failed N-API callback).
        let _ = self
            .commands
            .send(ActorCommand::Unsubscribe { id: self.id });
    }
}

impl PersistentJsonRpcClient {
    pub fn connect(config: ClientConfig) -> Result<Self, JsonRpcClientError> {
        let (commands, receiver) = tokio_mpsc::unbounded_channel();
        let inner = Arc::new(PersistentClientInner {
            commands,
            next_request_id: AtomicI64::new(1),
            next_subscription_id: AtomicU64::new(1),
            closed: AtomicBool::new(false),
        });
        let actor_inner = Arc::clone(&inner);
        // A single owned Tokio runtime and actor own every read, write and
        // pending-response registry for this connection.
        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build();
            match runtime {
                Ok(runtime) => {
                    runtime.block_on(run_actor(config, receiver, Arc::clone(&actor_inner)))
                }
                Err(_) => actor_inner.closed.store(true, Ordering::Release),
            }
        });
        Ok(Self { inner })
    }

    pub fn call<Params, Response>(
        &self,
        method: &str,
        params: &Params,
    ) -> Result<Response, JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        let request_id =
            RequestId::Integer(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed));
        let request = JsonRpcRequest::new(
            request_id,
            method,
            Some(serde_json::to_value(params).map_err(JsonRpcClientError::Serialize)?),
        );
        let response = self.send_request(request)?;
        serde_json::from_value(response.result).map_err(JsonRpcClientError::Deserialize)
    }

    pub fn subscribe_notifications(&self, capacity: usize) -> JsonRpcNotificationSubscription {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        let terminal = Arc::new(Mutex::new(None));
        let registered = Arc::new(AtomicBool::new(false));
        let id = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::Relaxed);
        if self.inner.closed.load(Ordering::Acquire)
            || self
                .inner
                .commands
                .send(ActorCommand::Subscribe {
                    id,
                    sender,
                    terminal: Arc::clone(&terminal),
                    registered: Arc::clone(&registered),
                })
                .is_err()
        {
            // Dropping the sender makes recv report the static closed failure.
        }
        JsonRpcNotificationSubscription {
            id,
            receiver,
            terminal,
            commands: self.inner.commands.clone(),
            registered,
        }
    }

    /// Atomically installs the notification receiver in the actor before its
    /// subscribe request enters the serialized writer queue.
    pub fn subscribe_then_call<Params, Response>(
        &self,
        method: &str,
        params: &Params,
        capacity: usize,
    ) -> Result<(JsonRpcNotificationSubscription, Response), JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        let terminal = Arc::new(Mutex::new(None));
        let registered = Arc::new(AtomicBool::new(false));
        let id = self
            .inner
            .next_subscription_id
            .fetch_add(1, Ordering::Relaxed);
        let request_id =
            RequestId::Integer(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed));
        let request = JsonRpcRequest::new(
            request_id,
            method,
            Some(serde_json::to_value(params).map_err(JsonRpcClientError::Serialize)?),
        );
        let (reply, wait) = oneshot::channel();
        self.inner
            .commands
            .send(ActorCommand::Subscribe {
                id,
                sender,
                terminal: Arc::clone(&terminal),
                registered: Arc::clone(&registered),
            })
            .map_err(|_| JsonRpcClientError::ConnectionClosed)?;
        self.inner
            .commands
            .send(ActorCommand::Request {
                request,
                response: reply,
            })
            .map_err(|_| JsonRpcClientError::ConnectionClosed)?;
        let response = wait
            .blocking_recv()
            .unwrap_or(Err(JsonRpcClientError::ConnectionClosed))?;
        let decoded =
            serde_json::from_value(response.result).map_err(JsonRpcClientError::Deserialize)?;
        Ok((
            JsonRpcNotificationSubscription {
                id,
                receiver,
                terminal,
                commands: self.inner.commands.clone(),
                registered,
            },
            decoded,
        ))
    }

    pub fn close(&self) {
        if !self.inner.closed.swap(true, Ordering::AcqRel) {
            let _ = self.inner.commands.send(ActorCommand::Close);
        }
    }

    fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, JsonRpcClientError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(JsonRpcClientError::ConnectionClosed);
        }
        let (sender, receiver) = oneshot::channel();
        self.inner
            .commands
            .send(ActorCommand::Request {
                request,
                response: sender,
            })
            .map_err(|_| JsonRpcClientError::ConnectionClosed)?;
        receiver
            .blocking_recv()
            .unwrap_or(Err(JsonRpcClientError::ConnectionClosed))
    }
}

async fn run_actor(
    config: ClientConfig,
    mut commands: tokio_mpsc::UnboundedReceiver<ActorCommand>,
    inner: Arc<PersistentClientInner>,
) {
    let stream = match connect_socket_tokio(&config.socket_address).await {
        Ok(stream) => stream,
        Err(_) => {
            inner.closed.store(true, Ordering::Release);
            return;
        }
    };
    use interprocess::local_socket::traits::tokio::Stream as _;
    let (read_half, mut write_half) = stream.split();
    let mut reader = TokioBufReader::new(read_half);
    let codec = JsonLineCodec;
    let mut pending: HashMap<
        RequestId,
        oneshot::Sender<Result<JsonRpcResponse, JsonRpcClientError>>,
    > = HashMap::new();
    let mut subscribers: HashMap<
        u64,
        (
            SyncSender<Result<JsonRpcNotification, NotificationReceiveError>>,
            Arc<Mutex<Option<NotificationReceiveError>>>,
            Arc<AtomicBool>,
        ),
    > = HashMap::new();
    let mut line = String::new();
    loop {
        line.clear();
        tokio::select! {
            command = commands.recv() => match command {
                Some(ActorCommand::Close) | None => break,
                Some(ActorCommand::Subscribe { id, sender, terminal, registered }) => {
                    registered.store(true, Ordering::Release);
                    subscribers.insert(id, (sender, terminal, registered));
                }
                Some(ActorCommand::Unsubscribe { id }) => {
                    if let Some((_, _, registered)) = subscribers.remove(&id) {
                        registered.store(false, Ordering::Release);
                    }
                }
                Some(ActorCommand::Request { request, response }) => {
                    let id = request.id.clone();
                    let encoded = match codec.encode_message(&JsonRpcMessage::Request(request)) {
                        Ok(value) => value,
                        Err(error) => { let _ = response.send(Err(JsonRpcClientError::Codec(error))); continue; }
                    };
                    // Register before the serialized write; the sole reader can now
                    // route an immediate response without a race.
                    pending.insert(id.clone(), response);
                    if write_half.write_all(encoded.as_bytes()).await.is_err() || write_half.flush().await.is_err() {
                        pending.remove(&id);
                        break;
                    }
                }
            },
            read = reader.read_line(&mut line) => match read {
                Ok(0) | Err(_) => break,
                Ok(_) => match codec.decode_message(&line) {
                    Ok(JsonRpcMessage::Response(response)) => {
                        // An unknown response means the peer has violated the only
                        // correlation contract on this connection. Continuing would
                        // strand a caller behind an untrusted stream.
                        let Some(waiter) = pending.remove(&response.id) else { break; };
                        let _ = waiter.send(Ok(response));
                    }
                    Ok(JsonRpcMessage::Error(error)) => {
                        let Some(id) = error.id.clone() else { break; };
                        let Some(waiter) = pending.remove(&id) else { break; };
                        let _ = waiter.send(Err(JsonRpcClientError::Remote(error)));
                    }
                    Ok(JsonRpcMessage::Notification(notification)) => {
                        // Every notification is fanned out unchanged. A lagged
                        // subscriber receives a distinct terminal Backpressure error.
                        subscribers.retain(|_, (sender, terminal, registered)| match sender.try_send(Ok(notification.clone())) {
                            Ok(()) => true,
                            Err(TrySendError::Full(_)) => {
                                *terminal.lock().expect("notification terminal lock poisoned") = Some(NotificationReceiveError::Backpressure);
                                registered.store(false, Ordering::Release);
                                false
                            }
                            Err(TrySendError::Disconnected(_)) => {
                                registered.store(false, Ordering::Release);
                                false
                            }
                        });
                    }
                    Ok(_) | Err(_) => break,
                }
            }
        }
    }
    inner.closed.store(true, Ordering::Release);
    for (_, waiter) in pending {
        let _ = waiter.send(Err(JsonRpcClientError::ConnectionClosed));
    }
    for (_, (subscriber, terminal, registered)) in subscribers {
        registered.store(false, Ordering::Release);
        let mut terminal = terminal
            .lock()
            .expect("notification terminal lock poisoned");
        if terminal.is_none() {
            *terminal = Some(NotificationReceiveError::ConnectionClosed);
            let _ = subscriber.try_send(Err(NotificationReceiveError::ConnectionClosed));
        }
    }
}

impl JsonRpcClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            codec: JsonLineCodec,
            next_request_id: Arc::new(AtomicI64::new(1)),
        }
    }

    pub fn local_default(service_name: &str, app_name: &str) -> Self {
        Self::new(ClientConfig::local_default(service_name, app_name))
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn call<Params, Response>(
        &self,
        method: &str,
        params: &Params,
    ) -> Result<Response, JsonRpcClientError>
    where
        Params: Serialize,
        Response: DeserializeOwned,
    {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let request = JsonRpcRequest::new(
            request_id.clone(),
            method,
            Some(serde_json::to_value(params).map_err(JsonRpcClientError::Serialize)?),
        );
        let response = self.send_request(request)?;
        serde_json::from_value(response.result).map_err(JsonRpcClientError::Deserialize)
    }

    pub fn send_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, JsonRpcClientError> {
        let request_span = rpc_client_request_span(
            self.config.service_name.as_str(),
            "local_socket",
            request.method.as_str(),
            &request.id,
            request.params.is_some(),
        );
        let _request_guard = request_span.enter();

        let mut stream = connect_socket(&self.config.socket_address)?;
        configure_connection_timeouts(&stream, Some(self.config.io_timeout))
            .map_err(JsonRpcClientError::ConfigureTimeout)?;
        let line = self
            .codec
            .encode_message(&JsonRpcMessage::Request(request.clone()))?;
        tracing::trace!(
            socket.address = %self.config.socket_address,
            "sending JSON-RPC request"
        );
        stream
            .write_all(line.as_bytes())
            .map_err(JsonRpcClientError::Write)?;
        stream.flush().map_err(JsonRpcClientError::Flush)?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        let bytes_read = reader
            .read_line(&mut response_line)
            .map_err(|error| map_read_error(self.config.io_timeout, error))?;
        if bytes_read == 0 {
            return Err(JsonRpcClientError::ConnectionClosed);
        }

        match self.codec.decode_message(&response_line)? {
            JsonRpcMessage::Response(message) => {
                if message.id != request.id {
                    tracing::warn!(
                        expected.request_id = %request.id,
                        actual.request_id = %message.id,
                        "json-rpc response id mismatch"
                    );
                    return Err(JsonRpcClientError::MismatchedResponseId {
                        expected: request.id,
                        actual: message.id,
                    });
                }
                tracing::debug!("received JSON-RPC response");
                Ok(message)
            }
            JsonRpcMessage::Error(message) => {
                if message.id.as_ref() != Some(&request.id) {
                    tracing::warn!(
                        expected.request_id = %request.id,
                        actual.request_id = ?message.id,
                        "json-rpc error id mismatch"
                    );
                    return Err(JsonRpcClientError::MismatchedErrorId {
                        expected: Some(request.id),
                        actual: message.id,
                    });
                }
                tracing::warn!(
                    error.code = message.error.code,
                    error.message = %message.error.message,
                    "received remote JSON-RPC error"
                );
                Err(JsonRpcClientError::Remote(message))
            }
            other => {
                tracing::warn!(message.kind = ?other, "received unexpected JSON-RPC message");
                Err(JsonRpcClientError::UnexpectedMessage(other))
            }
        }
    }
}

fn map_read_error(timeout: Duration, error: io::Error) -> JsonRpcClientError {
    if is_timeout_error(&error) {
        return JsonRpcClientError::ResponseTimeout {
            timeout,
            source: error,
        };
    }

    JsonRpcClientError::Read(error)
}

fn is_timeout_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

#[derive(Debug, Error)]
pub enum JsonRpcClientError {
    #[error(transparent)]
    Socket(#[from] SocketIoError),
    #[error(transparent)]
    Codec(#[from] JsonLineCodecError),
    #[error("failed to serialize request params: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to deserialize response payload: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("failed to configure daemon socket deadlines: {0}")]
    ConfigureTimeout(#[source] std::io::Error),
    #[error("failed to write request to daemon: {0}")]
    Write(#[source] std::io::Error),
    #[error("failed to flush request to daemon: {0}")]
    Flush(#[source] std::io::Error),
    #[error("failed to read response from daemon: {0}")]
    Read(#[source] std::io::Error),
    #[error("timed out waiting {timeout:?} for daemon response")]
    ResponseTimeout {
        timeout: Duration,
        #[source]
        source: std::io::Error,
    },
    #[error("server closed the connection before sending a response")]
    ConnectionClosed,
    #[error("daemon event subscriber fell behind")]
    Backpressure,
    #[error("remote JSON-RPC error {code}: {message}", code = .0.error.code, message = .0.error.message)]
    Remote(JsonRpcError),
    #[error("response id mismatch: expected {expected:?}, got {actual:?}")]
    MismatchedResponseId {
        expected: RequestId,
        actual: RequestId,
    },
    #[error("error id mismatch: expected {expected:?}, got {actual:?}")]
    MismatchedErrorId {
        expected: Option<RequestId>,
        actual: Option<RequestId>,
    },
    #[error("unexpected JSON-RPC message: {0:?}")]
    UnexpectedMessage(JsonRpcMessage),
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, BufRead, Write},
        sync::{Arc, Mutex},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;
    use tracing::subscriber::set_default;
    use tracing::{Level, info};

    use super::*;
    use crate::{SocketAddress, bind_listener};

    #[test]
    fn times_out_when_peer_never_sends_a_response() {
        let socket_address = unique_socket_address("ta-client-timeout");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let timeout = Duration::from_millis(50);

        let server_handle = thread::spawn(move || {
            let _stream = listener
                .accept()
                .expect("listener should accept one client");
            thread::sleep(Duration::from_millis(200));
        });

        let client = JsonRpcClient::new(ClientConfig {
            service_name: "ta-client-test".to_string(),
            socket_address,
            io_timeout: timeout,
        });
        let error = client
            .call::<_, serde_json::Value>("daemon.status", &json!({}))
            .expect_err("silent peer should trigger a response timeout");

        match error {
            JsonRpcClientError::ResponseTimeout {
                timeout: actual_timeout,
                ..
            } => {
                assert_eq!(actual_timeout, timeout);
            }
            other => panic!("expected response timeout, got {other:?}"),
        }

        server_handle.join().expect("server thread should complete");
    }

    #[test]
    fn request_span_uses_client_service_name() {
        let socket_address = unique_socket_address("ta-client-service-name");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_level(true)
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .with_writer(BufferWriterFactory {
                buffer: Arc::clone(&buffer),
            })
            .finish();
        let _guard = set_default(subscriber);

        let server_handle = thread::spawn(move || {
            let mut stream = listener
                .accept()
                .expect("listener should accept one client");
            let mut request_line = String::new();
            {
                let mut reader = BufReader::new(&mut stream);
                reader
                    .read_line(&mut request_line)
                    .expect("request should read");
            }
            let line = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    RequestId::Integer(1),
                    json!({ "ready": true }),
                )))
                .expect("response should encode");
            stream
                .write_all(line.as_bytes())
                .expect("response should write");
            stream.flush().expect("response should flush");
        });

        let client = JsonRpcClient::new(ClientConfig {
            service_name: "ta-cli".to_string(),
            socket_address,
            io_timeout: Duration::from_secs(1),
        });
        let response: serde_json::Value = client
            .call("daemon.status", &json!({}))
            .expect("client should receive response");

        info!(?response, "client call completed");
        server_handle.join().expect("server thread should complete");

        let rendered =
            String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf8 logs");
        assert!(rendered.contains("service.name=\"ta-cli\""), "{rendered}");
        assert!(
            rendered.contains("rpc.method=\"daemon.status\""),
            "{rendered}"
        );
    }

    #[test]
    fn persistent_client_reports_explicit_backpressure_without_blocking_reader() {
        let socket_address = unique_socket_address("ta-persistent-client-mux");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let server_handle = thread::spawn(move || {
            let mut stream = listener
                .accept()
                .expect("listener should accept one client");
            let mut request_line = String::new();
            BufReader::new(&mut stream)
                .read_line(&mut request_line)
                .expect("request should read");
            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            let notification = JsonLineCodec
                .encode_message(&JsonRpcMessage::Notification(JsonRpcNotification::new(
                    "daemon.run.event",
                    Some(json!({ "sequence": "18446744073709551615" })),
                )))
                .expect("notification should encode");
            let response = JsonLineCodec
                .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    json!({ "ready": true }),
                )))
                .expect("response should encode");
            stream
                .write_all(notification.as_bytes())
                .expect("notification should write");
            stream
                .write_all(notification.as_bytes())
                .expect("second notification should write");
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
            stream.flush().expect("frames should flush");
        });
        let config = ClientConfig {
            service_name: "ta-persistent-client-test".to_string(),
            socket_address,
            io_timeout: Duration::from_secs(1),
        };
        let client = PersistentJsonRpcClient::connect(config).expect("client should connect");
        let (subscription, response): (_, serde_json::Value) = client
            .subscribe_then_call("daemon.status", &json!({}), 1)
            .expect("response should arrive after notification");
        assert_eq!(response, json!({ "ready": true }));
        assert_eq!(
            subscription.recv(),
            Err(NotificationReceiveError::Backpressure)
        );
        server_handle.join().expect("server should complete");
    }

    #[test]
    fn dropping_notification_handle_unregisters_actor_registration_before_next_request() {
        let socket_address = unique_socket_address("ta-persistent-client-unsubscribe");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one connection");
            let mut reader = BufReader::new(&mut stream);
            for _ in 0..2 {
                let mut line = String::new();
                reader.read_line(&mut line).expect("request");
                let request = match JsonLineCodec.decode_message(&line).expect("decode") {
                    JsonRpcMessage::Request(request) => request,
                    other => panic!("expected request, got {other:?}"),
                };
                let response = JsonLineCodec
                    .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                        request.id,
                        json!({ "ready": true }),
                    )))
                    .expect("encode");
                reader
                    .get_mut()
                    .write_all(response.as_bytes())
                    .expect("response");
                reader.get_mut().flush().expect("flush");
            }
        });
        let client = PersistentJsonRpcClient::connect(ClientConfig {
            service_name: "test".into(),
            socket_address,
            io_timeout: Duration::from_secs(1),
        })
        .expect("connect");
        let (subscription, _): (_, serde_json::Value) = client
            .subscribe_then_call("daemon.subscribe", &json!({}), 1)
            .expect("subscribe response");
        let registered = Arc::clone(&subscription.registered);
        assert!(registered.load(Ordering::Acquire));
        drop(subscription);
        let _: serde_json::Value = client
            .call("daemon.status", &json!({}))
            .expect("second response");
        assert!(!registered.load(Ordering::Acquire));
        server.join().expect("server join");
    }

    #[test]
    fn persistent_client_correlates_out_of_order_concurrent_calls_on_one_connection() {
        let socket_address = unique_socket_address("ta-persistent-client-correlation");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one connection");
            let mut reader = BufReader::new(&mut stream);
            let mut first = String::new();
            let mut second = String::new();
            reader.read_line(&mut first).expect("first request");
            reader.read_line(&mut second).expect("second request");
            let first = match JsonLineCodec.decode_message(&first).expect("decode") {
                JsonRpcMessage::Request(request) => request,
                other => panic!("request: {other:?}"),
            };
            let second = match JsonLineCodec.decode_message(&second).expect("decode") {
                JsonRpcMessage::Request(request) => request,
                other => panic!("request: {other:?}"),
            };
            for request in [second, first] {
                let line = JsonLineCodec
                    .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                        request.id,
                        json!({ "method": request.method }),
                    )))
                    .expect("encode");
                reader
                    .get_mut()
                    .write_all(line.as_bytes())
                    .expect("response");
            }
            reader.get_mut().flush().expect("flush");
        });
        let client = PersistentJsonRpcClient::connect(ClientConfig {
            service_name: "test".into(),
            socket_address,
            io_timeout: Duration::from_secs(1),
        })
        .expect("connect");
        let left = client.clone();
        let right = client.clone();
        let a = thread::spawn(move || {
            left.call::<_, serde_json::Value>("first", &json!({}))
                .expect("first response")
        });
        let b = thread::spawn(move || {
            right
                .call::<_, serde_json::Value>("second", &json!({}))
                .expect("second response")
        });
        assert_eq!(a.join().expect("first join"), json!({ "method": "first" }));
        assert_eq!(
            b.join().expect("second join"),
            json!({ "method": "second" })
        );
        server.join().expect("server join");
    }

    #[test]
    fn persistent_client_close_wakes_pending_calls_for_all_clones() {
        let socket_address = unique_socket_address("ta-persistent-client-close");
        let listener = bind_listener(&socket_address).expect("listener should bind");
        let (seen_sender, seen_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let mut stream = listener.accept().expect("one connection");
            let mut line = String::new();
            BufReader::new(&mut stream)
                .read_line(&mut line)
                .expect("request");
            seen_sender.send(()).expect("request signal");
            release_receiver.recv().expect("release server socket");
        });
        let client = PersistentJsonRpcClient::connect(ClientConfig {
            service_name: "test".into(),
            socket_address,
            io_timeout: Duration::from_secs(1),
        })
        .expect("connect");
        let waiting = client.clone();
        let call = thread::spawn(move || waiting.call::<_, serde_json::Value>("wait", &json!({})));
        seen_receiver.recv().expect("request was written");
        client.close();
        assert!(matches!(
            call.join().expect("call join"),
            Err(JsonRpcClientError::ConnectionClosed)
        ));
        release_sender.send(()).expect("release server");
        server.join().expect("server join");
    }

    #[test]
    fn persistent_client_fails_closed_for_malformed_idless_and_unknown_reply_frames() {
        for (suffix, reply) in [
            ("malformed", "not-json\n".to_string()),
            (
                "idless",
                "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-1,\"message\":\"no id\"}}\n".to_string(),
            ),
            (
                "unknown",
                JsonLineCodec
                    .encode_message(&JsonRpcMessage::Response(JsonRpcResponse::new(
                        RequestId::Integer(999),
                        json!({}),
                    )))
                    .expect("unknown reply"),
            ),
        ] {
            let socket_address = unique_socket_address(&format!("ta-persistent-client-{suffix}"));
            let listener = bind_listener(&socket_address).expect("listener should bind");
            let (release_sender, release_receiver) = std::sync::mpsc::channel();
            let server = thread::spawn(move || {
                let mut stream = listener.accept().expect("one connection");
                let mut reader = BufReader::new(&mut stream);
                for _ in 0..2 {
                    let mut request = String::new();
                    reader.read_line(&mut request).expect("request");
                }
                stream.write_all(reply.as_bytes()).expect("reply");
                stream.flush().expect("flush");
                release_receiver
                    .recv()
                    .expect("release peer after client fails closed");
            });
            let client = PersistentJsonRpcClient::connect(ClientConfig {
                service_name: "test".into(),
                socket_address,
                io_timeout: Duration::from_secs(1),
            })
            .expect("connect");
            let barrier = Arc::new(std::sync::Barrier::new(3));
            let first = client.clone();
            let first_barrier = Arc::clone(&barrier);
            let first_call = thread::spawn(move || {
                first_barrier.wait();
                first.call::<_, serde_json::Value>("wait.first", &json!({}))
            });
            let second = client.clone();
            let second_barrier = Arc::clone(&barrier);
            let second_call = thread::spawn(move || {
                second_barrier.wait();
                second.call::<_, serde_json::Value>("wait.second", &json!({}))
            });
            barrier.wait();
            for call in [first_call, second_call] {
                assert!(matches!(
                    call.join().expect("call join"),
                    Err(JsonRpcClientError::ConnectionClosed)
                ));
            }
            release_sender.send(()).expect("release peer");
            server.join().expect("server join");
        }
    }

    #[cfg(unix)]
    fn unique_socket_address(prefix: &str) -> SocketAddress {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        SocketAddress::Unix(std::env::temp_dir().join(format!("{prefix}-{nanos}.sock")))
    }

    #[cfg(windows)]
    fn unique_socket_address(prefix: &str) -> SocketAddress {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        SocketAddress::NamedPipe(format!("{prefix}-{nanos}"))
    }

    #[derive(Clone)]
    struct BufferWriterFactory {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    struct BufferWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriterFactory {
        type Writer = BufferWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter {
                buffer: Arc::clone(&self.buffer),
            }
        }
    }

    impl Write for BufferWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buffer
                .lock()
                .expect("buffer lock")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
