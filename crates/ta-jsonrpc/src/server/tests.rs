use std::{
    io::{self, BufRead, BufReader, Write},
    sync::atomic::AtomicBool,
    sync::mpsc,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use tracing::{Dispatch, Level};

use super::*;
use crate::{JsonRpcClient, JsonRpcResponse, SocketConnection, bind_listener};

const TEST_PERSISTENT_METHOD: &str = "__test.initialize__";

fn spawn_async_connection_server(
    socket_address: SocketAddress,
    handler_factory: Arc<JsonRpcConnectionHandlerFactory>,
    timeout: Duration,
    connection_id: usize,
    service_name: &'static str,
    persistent_request_method: Option<&'static str>,
    ready_tx: mpsc::Sender<()>,
) -> thread::JoinHandle<Result<(), JsonRpcServerError>> {
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = bind_listener_tokio(&socket_address)?;
            ready_tx.send(()).expect("ready signal");
            let stream = listener.accept().await?;
            handle_connection(
                JsonLineCodec,
                stream,
                &handler_factory,
                service_name,
                connection_id,
                timeout,
                persistent_request_method,
            )
            .await
        })
    })
}

#[test]
fn round_trips_over_local_socket() {
    let socket_address = unique_socket_address("ta-transport-roundtrip");
    let handler_factory = stateless_handler_factory(|request| {
        if request.method != "daemon.status" {
            return Err(method_not_found(&request.method));
        }

        let params: serde_json::Map<String, Value> = parse_params(&request)?;
        assert!(params.is_empty());
        Ok(json!({
            "ready": true,
            "socketPath": "/tmp/test.sock",
            "logPath": "/tmp/taugentic-daemon/test/ta-daemon.log.jsonl",
            "version": "0.0.1"
        }))
    });

    let (ready_tx, ready_rx) = mpsc::channel();
    let server_handle = spawn_async_connection_server(
        socket_address.clone(),
        handler_factory,
        DEFAULT_SERVER_IO_TIMEOUT,
        1,
        "ta-daemon-test",
        None,
        ready_tx,
    );
    ready_rx.recv().expect("ready signal should arrive");

    let client = JsonRpcClient::new(crate::ClientConfig {
        service_name: "ta-cli-test".to_string(),
        socket_address,
        io_timeout: Duration::from_secs(1),
    });

    let response: serde_json::Value = client
        .call("daemon.status", &json!({}))
        .expect("client should receive response");

    assert_eq!(
        response,
        json!({
            "ready": true,
            "socketPath": "/tmp/test.sock",
            "logPath": "/tmp/taugentic-daemon/test/ta-daemon.log.jsonl",
            "version": "0.0.1"
        })
    );

    server_handle
        .join()
        .expect("server thread should complete")
        .expect("server should handle request");
}

#[test]
fn serve_once_times_out_when_client_never_sends_a_request() {
    let socket_address = unique_socket_address("ta-server-timeout");
    let timeout = Duration::from_millis(50);
    let handler_factory = stateless_handler_factory(|_| Ok(json!({ "ok": true })));
    let (ready_tx, ready_rx) = mpsc::channel();

    let server_handle = spawn_async_connection_server(
        socket_address.clone(),
        handler_factory,
        timeout,
        1,
        "ta-daemon-test",
        None,
        ready_tx,
    );
    ready_rx.recv().expect("ready signal should arrive");
    let _client = crate::connect_socket(&socket_address).expect("client should connect");

    let result = server_handle.join().expect("server thread should complete");
    match result {
        Err(JsonRpcServerError::RequestTimeout {
            timeout: actual_timeout,
            ..
        }) => {
            assert_eq!(actual_timeout, timeout);
        }
        other => panic!("expected request timeout, got {other:?}"),
    }
}

#[test]
fn initialize_disables_idle_request_timeout_for_persistent_connections() {
    let socket_address = unique_socket_address("ta-server-post-init-idle");
    let timeout = Duration::from_millis(50);
    let handler_factory = stateless_handler_factory(|request| match request.method.as_str() {
        TEST_PERSISTENT_METHOD => Ok(json!({ "initialized": true })),
        "daemon.status" => Ok(json!({
            "ready": true,
            "socketPath": "/tmp/test.sock",
            "logPath": "/tmp/taugentic-daemon/test/ta-daemon.log.jsonl",
            "version": "0.0.1"
        })),
        other => Err(method_not_found(other)),
    });
    let (ready_tx, ready_rx) = mpsc::channel();

    let server_handle = spawn_async_connection_server(
        socket_address.clone(),
        handler_factory,
        timeout,
        1,
        "ta-daemon-test",
        Some(TEST_PERSISTENT_METHOD),
        ready_tx,
    );
    ready_rx.recv().expect("ready signal should arrive");

    let mut stream = crate::connect_socket(&socket_address).expect("client should connect");
    let initialize_line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(1),
            TEST_PERSISTENT_METHOD,
            Some(json!({ "client": "test" })),
        )))
        .expect("initialize request should encode");
    let status_line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(2),
            "daemon.status",
            Some(json!({})),
        )))
        .expect("status request should encode");
    stream
        .write_all(initialize_line.as_bytes())
        .expect("initialize request should write");
    stream.flush().expect("initialize request should flush");

    let mut reader = BufReader::new(stream);
    let first = read_nonempty_line(&mut reader);
    let first: serde_json::Value = serde_json::from_str(&first).expect("first line should parse");
    assert_eq!(first.get("id").and_then(serde_json::Value::as_i64), Some(1));

    thread::sleep(Duration::from_millis(100));

    reader
        .get_mut()
        .write_all(status_line.as_bytes())
        .expect("status request should write");
    reader
        .get_mut()
        .flush()
        .expect("status request should flush");
    let second = read_nonempty_line(&mut reader);
    let second: serde_json::Value =
        serde_json::from_str(&second).expect("second line should parse");
    assert_eq!(
        second.get("id").and_then(serde_json::Value::as_i64),
        Some(2)
    );

    drop(reader.into_inner());
    server_handle
        .join()
        .expect("server thread should complete")
        .expect("server should keep connection alive after initialize");
}

#[test]
fn invalid_initialize_keeps_pre_init_request_timeout_armed() {
    let socket_address = unique_socket_address("ta-server-bad-init-timeout");
    let timeout = Duration::from_millis(50);
    let handler_factory = stateless_handler_factory(|request| match request.method.as_str() {
        TEST_PERSISTENT_METHOD => Err(invalid_params("bad initialize")),
        "daemon.status" => Ok(json!({
            "ready": true,
            "socketPath": "/tmp/test.sock",
            "logPath": "/tmp/taugentic-daemon/test/ta-daemon.log.jsonl",
            "version": "0.0.1"
        })),
        other => Err(method_not_found(other)),
    });
    let (ready_tx, ready_rx) = mpsc::channel();

    let server_handle = spawn_async_connection_server(
        socket_address.clone(),
        handler_factory,
        timeout,
        1,
        "ta-daemon-test",
        Some(TEST_PERSISTENT_METHOD),
        ready_tx,
    );
    ready_rx.recv().expect("ready signal should arrive");

    let mut stream = crate::connect_socket(&socket_address).expect("client should connect");
    let initialize_line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(1),
            TEST_PERSISTENT_METHOD,
            Some(json!({ "client": "test" })),
        )))
        .expect("initialize request should encode");
    stream
        .write_all(initialize_line.as_bytes())
        .expect("initialize request should write");
    stream.flush().expect("initialize request should flush");

    let mut reader = BufReader::new(stream);
    let first = read_nonempty_line(&mut reader);
    let first: serde_json::Value = serde_json::from_str(&first).expect("first line should parse");
    assert_eq!(first.get("id").and_then(serde_json::Value::as_i64), Some(1));
    assert!(
        first.get("error").is_some(),
        "invalid initialize should return JSON-RPC error"
    );

    thread::sleep(Duration::from_millis(100));
    drop(reader.into_inner());

    let result = server_handle.join().expect("server thread should complete");
    match result {
        Err(JsonRpcServerError::RequestTimeout {
            timeout: actual_timeout,
            ..
        }) => assert_eq!(actual_timeout, timeout),
        other => panic!("expected request timeout after invalid initialize, got {other:?}"),
    }
}

#[test]
fn session_notifications_fail_closed_when_outbound_queue_overflows() {
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(1);
    outbound_tx
        .try_send(JsonRpcMessage::Response(JsonRpcResponse::new(
            crate::RequestId::Integer(1),
            json!({ "ok": true }),
        )))
        .expect("seed outbound queue");
    let session = JsonRpcServerSession::new(3, outbound_tx, Arc::new(AtomicBool::new(true)));

    let error = session
        .send_notification("daemon.event", Some(json!({ "sequence": 2 })))
        .expect_err("overflow should close the session");

    assert!(matches!(
        error,
        crate::JsonRpcSessionError::OutboundBackpressure
    ));
    assert!(!session.is_open());
    outbound_rx.close();
}

#[test]
fn response_queue_fails_closed_when_outbound_queue_overflows() {
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(1);
    let is_open = Arc::new(AtomicBool::new(true));
    outbound_tx
        .try_send(JsonRpcMessage::Response(JsonRpcResponse::new(
            crate::RequestId::Integer(1),
            json!({ "ok": true }),
        )))
        .expect("seed outbound queue");

    let error = enqueue_outbound_message(
        &outbound_tx,
        &is_open,
        JsonRpcMessage::Response(JsonRpcResponse::new(
            crate::RequestId::Integer(2),
            json!({ "ok": false }),
        )),
    )
    .expect_err("overflow should close the connection");

    assert!(matches!(error, JsonRpcServerError::OutboundBackpressure));
    assert!(!is_open.load(Ordering::SeqCst));
    outbound_rx.close();
}

#[test]
fn request_span_uses_server_service_name() {
    let socket_address = unique_socket_address("ta-server-service-name");
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let dispatch = Dispatch::new(
        tracing_subscriber::fmt()
            .with_level(true)
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .with_writer(BufferWriterFactory {
                buffer: Arc::clone(&buffer),
            })
            .finish(),
    );
    let handler_factory = stateless_handler_factory(|request| {
        let _: serde_json::Map<String, Value> = parse_params(&request)?;
        Ok(json!({ "ready": true }))
    });
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_socket_address = socket_address.clone();
    let server_handle = thread::spawn(move || {
        let _guard = tracing::dispatcher::set_default(&dispatch);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener =
                bind_listener_tokio(&server_socket_address).expect("listener should bind");
            ready_tx.send(()).expect("ready signal");
            let stream = listener
                .accept()
                .await
                .expect("listener should accept one client");
            handle_connection(
                JsonLineCodec,
                stream,
                &handler_factory,
                "ta-daemon",
                4,
                Duration::from_secs(1),
                None,
            )
            .await
        })
    });
    ready_rx.recv().expect("ready signal should arrive");

    let mut stream = crate::connect_socket(&socket_address).expect("client should connect");
    let line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(7),
            "daemon.status",
            Some(json!({})),
        )))
        .expect("request should encode");
    stream
        .write_all(line.as_bytes())
        .expect("request should write");
    stream.flush().expect("request should flush");
    let mut response_line = String::new();
    let mut reader = BufReader::new(stream);
    reader
        .read_line(&mut response_line)
        .expect("response should read");
    drop(reader.into_inner());
    server_handle
        .join()
        .expect("server thread should complete")
        .expect("server should handle request");
    let rendered =
        String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf8 logs");
    assert!(
        rendered.contains("service.name=\"ta-daemon\""),
        "{rendered}"
    );
    assert!(
        rendered.contains("rpc.method=\"daemon.status\""),
        "{rendered}"
    );
    assert!(rendered.contains("taugentic.connection_id=4"), "{rendered}");
}

#[test]
fn persistent_connection_handles_multiple_requests_and_notifications() {
    let socket_address = unique_socket_address("ta-server-persistent-session");
    let handler_factory: Arc<JsonRpcConnectionHandlerFactory> =
        Arc::new(move |session: JsonRpcServerSession| {
            Box::new(move |request: JsonRpcRequest| {
                let session = session.clone();
                Box::pin(async move {
                    match request.method.as_str() {
                        "daemon.status" => Ok(json!({
                            "ready": true,
                            "socketPath": "/tmp/test.sock",
                            "logPath": "/tmp/taugentic-daemon/test/ta-daemon.log.jsonl",
                            "version": "0.0.1"
                        })),
                        "daemon.subscribe" => {
                            let notification_session = session.clone();
                            session.defer_until_response(Box::new(move || {
                                notification_session
                                    .send_notification(
                                        "daemon.event",
                                        Some(json!({
                                            "sequence": "1",
                                            "daemonInstanceId": "daemon-test",
                                            "occurredAtMs": "42",
                                            "event": {
                                                "run": {
                                                    "runId": "run-1",
                                                    "status": "queued",
                                                    "detail": "queued"
                                                }
                                            }
                                        })),
                                    )
                                    .expect("server should send notification");
                            }));
                            Ok(json!({
                                "subscribed": true,
                                "replayed": [],
                                "latestCursor": null
                            }))
                        }
                        other => Err(method_not_found(other)),
                    }
                }) as JsonRpcHandlerFuture
            }) as Box<JsonRpcConnectionHandler>
        });

    let (ready_tx, ready_rx) = mpsc::channel();
    let server_handle = spawn_async_connection_server(
        socket_address.clone(),
        handler_factory,
        DEFAULT_SERVER_IO_TIMEOUT,
        1,
        "ta-daemon-test",
        None,
        ready_tx,
    );
    ready_rx.recv().expect("ready signal should arrive");

    let mut stream = crate::connect_socket(&socket_address).expect("client should connect");
    let status_line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(1),
            "daemon.status",
            Some(json!({})),
        )))
        .expect("status request should encode");
    let subscribe_line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            crate::RequestId::Integer(2),
            "daemon.subscribe",
            Some(json!({ "kinds": ["run"] })),
        )))
        .expect("subscribe request should encode");
    stream
        .write_all(status_line.as_bytes())
        .expect("status request should write");
    stream
        .write_all(subscribe_line.as_bytes())
        .expect("subscribe request should write");
    stream.flush().expect("requests should flush");

    let mut reader = BufReader::new(stream);
    let first = read_nonempty_line(&mut reader);
    let second = read_nonempty_line(&mut reader);
    let third = read_nonempty_line(&mut reader);
    drop(reader.into_inner());

    let first: serde_json::Value = serde_json::from_str(&first).expect("first line should parse");
    let second: serde_json::Value =
        serde_json::from_str(&second).expect("second line should parse");
    let third: serde_json::Value = serde_json::from_str(&third).expect("third line should parse");

    assert_eq!(first.get("id").and_then(serde_json::Value::as_i64), Some(1));
    assert_eq!(
        second.get("id").and_then(serde_json::Value::as_i64),
        Some(2)
    );
    assert_eq!(
        third.get("method").and_then(serde_json::Value::as_str),
        Some("daemon.event")
    );

    server_handle
        .join()
        .expect("server thread should complete")
        .expect("server should handle persistent session");
}

#[test]
fn overload_error_preserves_request_id_for_requests() {
    let response = overload_response_for_request_line(
        JsonLineCodec,
        r#"{"jsonrpc":"2.0","id":42,"method":"daemon.status","params":{}}"#,
        3,
        1,
    );

    let JsonRpcMessage::Error(error) = response else {
        panic!("expected json-rpc error response");
    };

    assert_eq!(error.id, Some(crate::RequestId::Integer(42)));
    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert_eq!(error.error.message, "too many in-flight JSON-RPC requests");
}

#[cfg(unix)]
#[test]
fn rebinds_after_stale_socket_file_is_left_behind() {
    let socket_address = unique_socket_address("ta-transport-stale");
    let SocketAddress::Unix(path) = &socket_address else {
        panic!("unix test should create a unix socket address");
    };

    let stale_listener =
        std::os::unix::net::UnixListener::bind(path).expect("stale listener should bind");
    drop(stale_listener);
    assert!(path.exists(), "stale socket file should remain on disk");

    let listener = bind_listener(&socket_address).expect("bind should replace stale socket");
    drop(listener);
}

#[cfg(unix)]
#[test]
fn refuses_to_replace_live_socket_listener() {
    let socket_address = unique_socket_address("ta-transport-live");
    let SocketAddress::Unix(path) = &socket_address else {
        panic!("unix test should create a unix socket address");
    };

    let live_listener =
        std::os::unix::net::UnixListener::bind(path).expect("live listener should bind");

    let error = bind_listener(&socket_address).expect_err("bind should refuse live socket");
    let SocketIoError::Bind { source, .. } = error else {
        panic!("live socket should fail during bind preparation");
    };

    assert_eq!(source.kind(), std::io::ErrorKind::AddrInUse);
    assert!(
        source.to_string().contains("daemon already running"),
        "live listener error should explain already-running state: {source}"
    );
    assert!(
        path.exists(),
        "live listener socket path should remain intact after refusal"
    );
    std::os::unix::net::UnixStream::connect(path)
        .expect("live listener should remain reachable after refusal");

    drop(live_listener);
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

fn stateless_handler_factory<F>(handler: F) -> Arc<JsonRpcConnectionHandlerFactory>
where
    F: Fn(JsonRpcRequest) -> JsonRpcHandlerResult + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    Arc::new(move |_| {
        let handler = Arc::clone(&handler);
        Box::new(move |request| {
            let result = handler(request);
            Box::pin(async move { result }) as JsonRpcHandlerFuture
        })
    })
}

fn read_nonempty_line(reader: &mut BufReader<SocketConnection>) -> String {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("line should read from socket");
    line.trim().to_string()
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
