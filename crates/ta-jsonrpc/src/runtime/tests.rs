use std::{
    cell::Cell,
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::future::join_all;
use serde_json::{Value, json};
use tracing::{Dispatch, Level};

use super::*;
use crate::{INTERNAL_ERROR_CODE, INVALID_PARAMS_ERROR_CODE, RequestId};

#[test]
fn handler_panic_returns_internal_error_and_connection_continues() {
    let (result, logs) = capture_tracing(|| {
        run_async(async {
            let handler: Arc<JsonRpcRequestHandler> = Arc::new(|request| {
                Box::pin(async move {
                    match request.method.as_str() {
                        "daemon.panic" => panic!("boom"),
                        "daemon.status" => Ok(json!({ "ready": true })),
                        other => Err(JsonRpcErrorObject::method_not_found(other)),
                    }
                }) as JsonRpcHandlerFuture
            });
            let mut adapter = TestConnectionAdapter::new(
                handler,
                vec![request(1, "daemon.panic"), request(2, "daemon.status")],
            );
            let (mut connection_runtime, mut outbound_rx) =
                JsonRpcConnectionRuntime::new(42, DEFAULT_OUTBOUND_QUEUE_DEPTH);

            run_jsonrpc_connection_loop(
                &mut connection_runtime,
                Duration::from_secs(1),
                None,
                &mut adapter,
            )
            .await
            .expect("connection loop should continue after handler panic");

            let first = outbound_rx
                .try_recv()
                .expect("panicking request should produce an error response");
            let second = outbound_rx
                .try_recv()
                .expect("subsequent request should produce a response");
            (first, second)
        })
    });

    let (first, second) = result;
    assert_handler_panic_error(first, Some(RequestId::Integer(1)), "daemon.panic");
    assert_success_response(second, RequestId::Integer(2), json!({ "ready": true }));
    assert!(logs.contains("RPC handler panicked"), "{logs}");
    assert!(logs.contains("rpc.method=\"daemon.panic\""), "{logs}");
    assert!(logs.contains("rpc.request_id=1"), "{logs}");
    assert!(logs.contains("panic.message=boom"), "{logs}");
}

#[test]
fn after_response_action_panic_is_logged_and_connection_continues() {
    let (result, logs) = capture_tracing(|| {
        run_async(async {
            let (mut connection_runtime, mut outbound_rx) =
                JsonRpcConnectionRuntime::new(42, DEFAULT_OUTBOUND_QUEUE_DEPTH);
            let session = connection_runtime.session();
            let handler: Arc<JsonRpcRequestHandler> = Arc::new(move |request| {
                let session = session.clone();
                Box::pin(async move {
                    match request.method.as_str() {
                        "daemon.subscribe" => {
                            session.defer_until_response(Box::new(|| panic!("deferred boom")));
                            Ok(json!({ "subscribed": true }))
                        }
                        "daemon.status" => Ok(json!({ "ready": true })),
                        other => Err(JsonRpcErrorObject::method_not_found(other)),
                    }
                }) as JsonRpcHandlerFuture
            });
            let mut adapter = TestConnectionAdapter::new(
                handler,
                vec![request(1, "daemon.subscribe"), request(2, "daemon.status")],
            );

            run_jsonrpc_connection_loop(
                &mut connection_runtime,
                Duration::from_secs(1),
                None,
                &mut adapter,
            )
            .await
            .expect("connection loop should continue after deferred action panic");

            let first = outbound_rx
                .try_recv()
                .expect("subscribe request should produce a response");
            let second = outbound_rx
                .try_recv()
                .expect("subsequent request should produce a response");
            (first, second)
        })
    });

    let (first, second) = result;
    assert_success_response(first, RequestId::Integer(1), json!({ "subscribed": true }));
    assert_success_response(second, RequestId::Integer(2), json!({ "ready": true }));
    assert!(
        logs.contains("JSON-RPC after-response action panicked"),
        "{logs}"
    );
    assert!(logs.contains("rpc.method=\"daemon.subscribe\""), "{logs}");
    assert!(logs.contains("rpc.request_id=1"), "{logs}");
    assert!(logs.contains("panic.message=deferred boom"), "{logs}");
}

#[test]
fn handler_panic_with_opaque_payload_uses_fallback_log_message() {
    struct OpaquePanicPayload;

    let (response, logs) = capture_tracing(|| {
        run_async(async {
            let handler = |_request: JsonRpcRequest| {
                Box::pin(async move {
                    std::panic::panic_any(OpaquePanicPayload);
                    #[allow(unreachable_code)]
                    Ok(json!({ "ok": true }))
                }) as JsonRpcHandlerFuture
            };

            process_jsonrpc_request(&handler, context(), request(7, "daemon.opaque_panic")).await
        })
    });

    let response = response
        .response
        .expect("panicking request should produce an error response");
    assert_handler_panic_error(response, Some(RequestId::Integer(7)), "daemon.opaque_panic");
    assert!(
        logs.contains("panic.message=<unprintable panic payload>"),
        "{logs}"
    );
}

#[test]
fn normal_handler_error_shape_is_unchanged() {
    let processed = run_async(async {
        let handler = |_request: JsonRpcRequest| {
            Box::pin(async move { Err(JsonRpcErrorObject::invalid_params("bad params")) })
                as JsonRpcHandlerFuture
        };

        process_jsonrpc_request(&handler, context(), request(3, "daemon.invalid")).await
    });

    let JsonRpcMessage::Error(error) = processed
        .response
        .expect("handler error should produce error response")
    else {
        panic!("expected JSON-RPC error response");
    };
    assert_eq!(error.id, Some(RequestId::Integer(3)));
    assert_eq!(error.error.code, INVALID_PARAMS_ERROR_CODE);
    assert_eq!(error.error.message, "bad params");
    assert_eq!(error.error.data, None);
}

#[test]
fn handler_future_with_non_unwind_safe_state_still_completes() {
    let processed = run_async(async {
        let handler = |_request: JsonRpcRequest| {
            Box::pin(async move {
                let counter = Cell::new(1);
                tokio::task::yield_now().await;
                counter.set(counter.get() + 1);
                Ok(json!({ "counter": counter.get() }))
            }) as JsonRpcHandlerFuture
        };

        process_jsonrpc_request(&handler, context(), request(4, "daemon.cell")).await
    });

    assert_success_response(
        processed
            .response
            .expect("non-UnwindSafe handler should produce response"),
        RequestId::Integer(4),
        json!({ "counter": 2 }),
    );
}

#[test]
fn concurrent_handler_panics_are_isolated_per_request() {
    let processed = run_async(async {
        let handler = |request: JsonRpcRequest| {
            Box::pin(async move {
                tokio::task::yield_now().await;
                let RequestId::Integer(index) = request.id else {
                    panic!("test requests use integer ids");
                };
                if request.method == "daemon.panic" {
                    panic!("boom-{index}");
                }
                Ok(json!({ "index": index }))
            }) as JsonRpcHandlerFuture
        };

        let futures = (0..10).map(|index| {
            let method = if index % 2 == 0 {
                "daemon.panic"
            } else {
                "daemon.ok"
            };
            process_jsonrpc_request(&handler, context(), request(index, method))
        });
        join_all(futures).await
    });

    let mut panicked = 0;
    let mut succeeded = 0;
    for processed in processed {
        let response = processed
            .response
            .expect("each request should produce a JSON-RPC response");
        match response {
            JsonRpcMessage::Error(error) => {
                panicked += 1;
                assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
                assert_eq!(
                    error.error.data.and_then(|data| data.get("kind").cloned()),
                    Some(json!("handler_panicked"))
                );
            }
            JsonRpcMessage::Response(response) => {
                succeeded += 1;
                let index = response.result.get("index").and_then(Value::as_i64);
                assert!(matches!(index, Some(1 | 3 | 5 | 7 | 9)));
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    assert_eq!(panicked, 5);
    assert_eq!(succeeded, 5);
}

struct TestConnectionAdapter {
    handler: Arc<JsonRpcRequestHandler>,
    messages: VecDeque<JsonRpcRequest>,
}

impl TestConnectionAdapter {
    fn new(handler: Arc<JsonRpcRequestHandler>, messages: Vec<JsonRpcRequest>) -> Self {
        Self {
            handler,
            messages: messages.into(),
        }
    }
}

#[derive(Debug)]
enum TestConnectionError {
    Outbound,
}

impl JsonRpcConnectionAdapter for TestConnectionAdapter {
    type Message = JsonRpcRequest;
    type Error = TestConnectionError;

    async fn drain_outbound(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn read_next(
        &mut self,
        _timeout: Duration,
        _request_timeout_armed: bool,
    ) -> Result<JsonRpcConnectionLoopEvent<Self::Message>, Self::Error> {
        Ok(self
            .messages
            .pop_front()
            .map(JsonRpcConnectionLoopEvent::Message)
            .unwrap_or(JsonRpcConnectionLoopEvent::Closed))
    }

    async fn process_message(
        &mut self,
        message: Self::Message,
    ) -> Result<ProcessedJsonRpcMessage, Self::Error> {
        Ok(process_jsonrpc_request(&*self.handler, context(), message).await)
    }

    fn map_outbound_error(&self, error: OutboundQueueError) -> Self::Error {
        let _ = error;
        TestConnectionError::Outbound
    }
}

fn assert_handler_panic_error(
    response: JsonRpcMessage,
    expected_id: Option<RequestId>,
    expected_method: &str,
) {
    let JsonRpcMessage::Error(error) = response else {
        panic!("expected JSON-RPC error response");
    };
    assert_eq!(error.id, expected_id);
    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert_eq!(error.error.message, "Internal error");
    assert_eq!(
        error.error.data,
        Some(json!({
            "kind": "handler_panicked",
            "method": expected_method,
        }))
    );
}

fn assert_success_response(
    response: JsonRpcMessage,
    expected_id: RequestId,
    expected_result: Value,
) {
    let JsonRpcMessage::Response(response) = response else {
        panic!("expected JSON-RPC success response");
    };
    assert_eq!(response.id, expected_id);
    assert_eq!(response.result, expected_result);
}

fn request(id: i64, method: &str) -> JsonRpcRequest {
    JsonRpcRequest::new(RequestId::Integer(id), method, Some(json!({})))
}

fn context() -> JsonRpcRequestProcessingContext<'static> {
    JsonRpcRequestProcessingContext {
        service_name: "ta-daemon-test",
        transport: "test",
        connection_id: 42,
    }
}

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
        .block_on(future)
}

fn capture_tracing<T>(f: impl FnOnce() -> T) -> (T, String) {
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

    let result = tracing::dispatcher::with_default(&dispatch, f);
    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("captured tracing logs should be utf8");
    (result, logs)
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
