use std::net::{TcpListener, TcpStream};
use std::thread;
use std::thread::sleep;
use std::time::Instant;

use super::*;
use crate::{
    DAEMON_PROTOCOL_VERSION, DaemonClientCapabilities, DaemonInitializeParams, DaemonStatusParams,
    DaemonStatusResult, RequestId,
    host::{bootstrap::boot, config::RemoteAuthToken},
};

const TEST_AUTH_TOKEN: &str = "0123456789abcdef0123456789abcdef";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_TEST_IO_TIMEOUT: Duration = Duration::from_secs(5);
const SHORT_IO_TIMEOUT: Duration = Duration::from_millis(100);

#[test]
fn authenticated_remote_websocket_round_trips_daemon_status() {
    init_test_tracing();
    let server = TestRemoteServer::spawn();
    let mut socket = server
        .connect(TEST_AUTH_TOKEN)
        .expect("authenticated websocket");

    write_client_request(
        &mut socket,
        JsonRpcRequest::new(
            RequestId::Integer(1),
            crate::METHOD_DAEMON_STATUS,
            Some(serde_json::to_value(DaemonStatusParams {}).expect("status params")),
        ),
    );

    let response = read_client_message(&mut socket);
    let JsonRpcMessage::Response(response) = response else {
        panic!("expected JSON-RPC response");
    };
    let status: DaemonStatusResult =
        serde_json::from_value(response.result).expect("status result should deserialize");
    assert!(status.ready);
    assert_eq!(response.id, RequestId::Integer(1));
}

#[test]
fn remote_websocket_rejects_missing_bearer_token() {
    init_test_tracing();
    let server = TestRemoteServer::spawn();
    let uri: tungstenite::http::Uri = server.remote.endpoint_url().parse().expect("uri");
    let error = tungstenite::connect(uri).expect_err("missing authorization header should fail");
    let tungstenite::Error::Http(response) = error else {
        panic!("expected HTTP handshake error, got {error:?}");
    };
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn remote_websocket_rejects_wrong_bearer_token() {
    init_test_tracing();
    let server = TestRemoteServer::spawn();
    let error = server
        .connect("wrong-token")
        .expect_err("wrong authorization header should fail");
    let tungstenite::Error::Http(response) = error else {
        panic!("expected HTTP handshake error, got {error:?}");
    };
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn remote_websocket_rejects_wrong_path() {
    init_test_tracing();
    let server = TestRemoteServer::spawn();
    let error = server
        .connect_at_path(TEST_AUTH_TOKEN, "/wrong-path")
        .expect_err("wrong websocket path should fail");
    let tungstenite::Error::Http(response) = error else {
        panic!("expected HTTP handshake error, got {error:?}");
    };
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn remote_websocket_returns_parse_error_for_malformed_jsonrpc_payload() {
    init_test_tracing();
    let server = TestRemoteServer::spawn();
    let mut socket = server
        .connect(TEST_AUTH_TOKEN)
        .expect("authenticated websocket");

    socket
        .send(Message::Text("{".to_string().into()))
        .expect("malformed payload should send");

    let response = read_client_message(&mut socket);
    let JsonRpcMessage::Error(response) = response else {
        panic!("expected JSON-RPC error");
    };
    assert_eq!(response.id, None);
    assert_eq!(response.error.code, crate::PARSE_ERROR_CODE);
    assert!(
        response
            .error
            .message
            .contains("failed to decode JSON-RPC message"),
        "unexpected parse error message: {}",
        response.error.message
    );
}

#[test]
fn remote_websocket_times_out_idle_connection_before_initialize() {
    init_test_tracing();
    let server = TestRemoteServer::spawn_with_io_timeout(SHORT_IO_TIMEOUT);
    let mut socket = server
        .connect(TEST_AUTH_TOKEN)
        .expect("authenticated websocket");
    set_client_read_timeout(&mut socket, Duration::from_secs(1));

    sleep(Duration::from_millis(250));

    match socket.read() {
        Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {}
        Err(tungstenite::Error::Protocol(
            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
        )) => {}
        Err(tungstenite::Error::Io(error))
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset
                    | io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::BrokenPipe
            ) => {}
        other => {
            panic!("expected websocket idle timeout closure before initialize, got {other:?}")
        }
    }
}

#[test]
fn remote_websocket_allows_idle_connection_after_initialize() {
    init_test_tracing();
    let server = TestRemoteServer::spawn_with_io_timeout(SHORT_IO_TIMEOUT);
    let mut socket = server
        .connect(TEST_AUTH_TOKEN)
        .expect("authenticated websocket");

    initialize_client(&mut socket, 1);
    sleep(Duration::from_millis(350));

    write_client_request(
        &mut socket,
        JsonRpcRequest::new(
            RequestId::Integer(2),
            crate::METHOD_DAEMON_STATUS,
            Some(serde_json::to_value(DaemonStatusParams {}).expect("status params")),
        ),
    );

    let response = read_client_message(&mut socket);
    let JsonRpcMessage::Response(response) = response else {
        panic!("expected JSON-RPC response");
    };
    assert_eq!(response.id, RequestId::Integer(2));
}

struct TestRemoteServer {
    remote: RemoteWebsocketConfig,
    shutdown_requested: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<Result<(), RemoteWebsocketServerError>>>,
}

impl TestRemoteServer {
    fn spawn() -> Self {
        Self::spawn_with_io_timeout(DEFAULT_TEST_IO_TIMEOUT)
    }

    fn spawn_with_io_timeout(io_timeout: Duration) -> Self {
        let bind_address = reserve_socket_address();
        let remote = RemoteWebsocketConfig {
            bind_address,
            auth_token: RemoteAuthToken::new(TEST_AUTH_TOKEN.to_string()),
            path: crate::host::config::DAEMON_REMOTE_WS_PATH.to_string(),
        };
        let mut config = crate::host::config::test_config();
        config.server.io_timeout = io_timeout;
        config.remote = Some(remote.clone());
        let state = boot(config);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let state_for_thread = state.clone();
        let shutdown_for_thread = Arc::clone(&shutdown_requested);
        let handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(serve_remote_until(state_for_thread, shutdown_for_thread))
        });
        wait_for_listening(&remote);
        Self {
            remote,
            shutdown_requested,
            handle: Some(handle),
        }
    }

    fn connect(
        &self,
        auth_token: &str,
    ) -> Result<
        tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
        tungstenite::Error,
    > {
        self.connect_at_path(auth_token, self.remote.path.as_str())
    }

    fn connect_at_path(
        &self,
        auth_token: &str,
        path: &str,
    ) -> Result<
        tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
        tungstenite::Error,
    > {
        let uri: tungstenite::http::Uri = format!("ws://{}{}", self.remote.bind_address, path)
            .parse()
            .expect("uri");
        let request = tungstenite::ClientRequestBuilder::new(uri)
            .with_header("Authorization", format!("Bearer {auth_token}"));
        let (socket, _response) = tungstenite::connect(request)?;
        Ok(socket)
    }
}

impl Drop for TestRemoteServer {
    fn drop(&mut self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .expect("remote websocket thread should join")
                .expect("remote websocket server should stop cleanly");
        }
    }
}

fn initialize_client(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    request_id: i64,
) {
    write_client_request(
        socket,
        JsonRpcRequest::new(
            RequestId::Integer(request_id),
            crate::METHOD_DAEMON_INITIALIZE,
            Some(
                serde_json::to_value(DaemonInitializeParams {
                    client_name: "ta-orchestrator-tests".to_string(),
                    client_credential: None,
                    client_version: "0.0.1".to_string(),
                    protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                    capabilities: DaemonClientCapabilities {
                        notifications: true,
                        event_subscriptions: true,
                    },
                })
                .expect("initialize params"),
            ),
        ),
    );
    let response = read_client_message(socket);
    let JsonRpcMessage::Response(response) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(response.id, RequestId::Integer(request_id));
}

fn write_client_request(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    request: JsonRpcRequest,
) {
    let payload =
        serde_json::to_string(&JsonRpcMessage::Request(request)).expect("request should serialize");
    socket
        .send(Message::Text(payload.into()))
        .expect("request should send");
}

fn read_client_message(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
) -> JsonRpcMessage {
    loop {
        match socket.read().expect("socket should read") {
            Message::Text(payload) => {
                return serde_json::from_str(payload.as_str()).expect("message should decode");
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .expect("pong should send");
            }
            Message::Pong(_) => {}
            Message::Close(frame) => panic!("unexpected close frame: {frame:?}"),
            _ => {}
        }
    }
}

fn wait_for_listening(remote: &RemoteWebsocketConfig) {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        let uri: tungstenite::http::Uri = remote
            .endpoint_url()
            .parse()
            .expect("test uri should parse");
        let request = tungstenite::ClientRequestBuilder::new(uri)
            .with_header("Authorization", format!("Bearer {}", TEST_AUTH_TOKEN));
        match tungstenite::connect(request) {
            Ok((mut socket, _response)) => {
                let _ = socket.close(None);
                return;
            }
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::WouldBlock
                ) =>
            {
                thread::sleep(POLL_INTERVAL);
            }
            Err(_) => thread::sleep(POLL_INTERVAL),
        }
    }

    panic!("timed out waiting for remote websocket server to listen");
}

fn reserve_socket_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral listener");
    let address = listener.local_addr().expect("listener address");
    drop(listener);
    address
}

fn set_client_read_timeout(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) {
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream
            .set_read_timeout(Some(timeout))
            .expect("client read timeout should set");
    }
}

fn init_test_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}
