use std::{
    io::{self, BufRead, BufReader, Write},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use serde::{Serialize, de::DeserializeOwned};
use ta_observability::rpc_client_request_span;
use thiserror::Error;

use crate::{
    JsonLineCodec, JsonLineCodecError, JsonRpcError, JsonRpcMessage, JsonRpcRequest,
    JsonRpcResponse, RequestId, SocketAddress, SocketIoError, configure_connection_timeouts,
    connect_socket,
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
        io::{self, Write},
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
