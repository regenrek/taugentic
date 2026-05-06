use tracing::{Span, info_span};

pub fn rpc_server_request_span(
    service_name: &str,
    transport: &'static str,
    connection_id: usize,
    method: &str,
    request_id: &impl std::fmt::Display,
    jsonrpc_version: &str,
    params_present: bool,
) -> Span {
    info_span!(
        "taugentic.rpc.request",
        otel.kind = "server",
        otel.name = method,
        service.name = service_name,
        rpc.system = "jsonrpc",
        rpc.method = method,
        rpc.transport = transport,
        rpc.request_id = %request_id,
        rpc.jsonrpc_version = jsonrpc_version,
        rpc.params_present = params_present,
        taugentic.connection_id = connection_id as u64,
    )
}

pub fn rpc_client_request_span(
    service_name: &str,
    transport: &'static str,
    method: &str,
    request_id: &impl std::fmt::Display,
    params_present: bool,
) -> Span {
    info_span!(
        "taugentic.rpc.request",
        otel.kind = "client",
        otel.name = method,
        service.name = service_name,
        rpc.system = "jsonrpc",
        rpc.method = method,
        rpc.transport = transport,
        rpc.request_id = %request_id,
        rpc.params_present = params_present,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        sync::{Arc, Mutex},
    };

    use tracing::subscriber::set_default;
    use tracing::{Level, info};

    use super::{rpc_client_request_span, rpc_server_request_span};

    #[test]
    fn rpc_server_request_span_records_expected_fields() {
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

        let span = rpc_server_request_span(
            "ta-daemon",
            "local_socket",
            7,
            "daemon.status",
            &99,
            "2.0",
            true,
        );
        let _entered = span.enter();
        info!("handled request");

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
        assert!(rendered.contains("rpc.request_id=99"), "{rendered}");
        assert!(
            rendered.contains("rpc.transport=\"local_socket\""),
            "{rendered}"
        );
        assert!(rendered.contains("rpc.params_present=true"), "{rendered}");
        assert!(rendered.contains("taugentic.connection_id=7"), "{rendered}");
    }

    #[test]
    fn rpc_client_request_span_records_expected_fields() {
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

        let span = rpc_client_request_span("ta-cli", "local_socket", "daemon.status", &99, false);
        let _entered = span.enter();
        info!("sent request");

        let rendered =
            String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf8 logs");
        assert!(rendered.contains("service.name=\"ta-cli\""), "{rendered}");
        assert!(rendered.contains("otel.kind=\"client\""), "{rendered}");
        assert!(
            rendered.contains("otel.name=\"daemon.status\""),
            "{rendered}"
        );
        assert!(rendered.contains("rpc.system=\"jsonrpc\""), "{rendered}");
        assert!(
            rendered.contains("rpc.method=\"daemon.status\""),
            "{rendered}"
        );
        assert!(rendered.contains("rpc.request_id=99"), "{rendered}");
        assert!(
            rendered.contains("rpc.transport=\"local_socket\""),
            "{rendered}"
        );
        assert!(rendered.contains("rpc.params_present=false"), "{rendered}");
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
