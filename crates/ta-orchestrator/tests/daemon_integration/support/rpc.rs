use super::*;

pub fn write_request(
    codec: &JsonLineCodec,
    stream: &mut SocketConnection,
    request: JsonRpcRequest,
) {
    let line = codec
        .encode_message(&JsonRpcMessage::Request(request))
        .expect("request should encode");
    stream
        .write_all(line.as_bytes())
        .expect("request should write");
    stream.flush().expect("request should flush");
}

pub fn read_response(codec: &JsonLineCodec, stream: &mut SocketConnection) -> JsonRpcResponse {
    match read_message(codec, stream) {
        JsonRpcMessage::Response(response) => response,
        other => panic!("expected JSON-RPC response, got {other:?}"),
    }
}

pub fn read_error(codec: &JsonLineCodec, stream: &mut SocketConnection) -> JsonRpcError {
    match read_message(codec, stream) {
        JsonRpcMessage::Error(error) => error,
        other => panic!("expected JSON-RPC error, got {other:?}"),
    }
}

pub fn read_event_notification(
    codec: &JsonLineCodec,
    stream: &mut SocketConnection,
) -> ta_jsonrpc::JsonRpcNotification {
    match read_message(codec, stream) {
        JsonRpcMessage::Notification(notification) => notification,
        other => panic!("expected JSON-RPC notification, got {other:?}"),
    }
}

pub fn read_public_terminal_run_event(
    codec: &JsonLineCodec,
    stream: &mut SocketConnection,
    run_id: &RunId,
) -> PublicDaemonEventEnvelope {
    loop {
        let event = read_event_notification(codec, stream);
        let envelope: PublicDaemonEventEnvelope =
            serde_json::from_value(event.params.expect("event params should exist"))
                .expect("daemon event params should deserialize");
        if matches!(
            &envelope.event,
            PublicDaemonEvent::Run(ta_protocol::wire::RunEvent::Status(status_event))
                if status_event.run_id() == run_id && matches!(status_event.status(),
                    ta_protocol::wire::RunStatus::Completed
                    | ta_protocol::wire::RunStatus::Failed
                    | ta_protocol::wire::RunStatus::Cancelled,
                )
        ) {
            return envelope;
        }
    }
}

pub fn read_terminal_run_event(
    codec: &JsonLineCodec,
    stream: &mut SocketConnection,
    run_id: &RunId,
) -> DaemonEventEnvelope {
    loop {
        let event = read_event_notification(codec, stream);
        let envelope: DaemonEventEnvelope =
            serde_json::from_value(event.params.expect("event params should exist"))
                .expect("daemon event params should deserialize");
        if matches!(
            &envelope.event,
            ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent::Status(status_event))
                if status_event.run_id() == run_id && matches!(status_event.status(),
                    ta_protocol::wire::RunStatus::Completed
                    | ta_protocol::wire::RunStatus::Failed
                    | ta_protocol::wire::RunStatus::Cancelled,
                )
        ) {
            return envelope;
        }
    }
}

pub fn read_message(codec: &JsonLineCodec, stream: &mut SocketConnection) -> JsonRpcMessage {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
        stream
            .read_exact(&mut byte)
            .expect("daemon should send a response line");
        line.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }

    let line = String::from_utf8(line).expect("daemon response line should be valid utf-8");
    codec.decode_message(&line).expect("response should decode")
}

pub fn initialize_session(
    stream: &mut SocketConnection,
    request_id: RequestId,
) -> DaemonInitializeResult {
    initialize_named_session_with_credential(stream, request_id, "ta-orchestrator-tests", None)
}

pub fn initialize_named_session(
    stream: &mut SocketConnection,
    request_id: RequestId,
    client_name: &str,
) -> DaemonInitializeResult {
    initialize_named_session_with_credential(stream, request_id, client_name, None)
}

pub fn initialize_named_session_with_credential(
    stream: &mut SocketConnection,
    request_id: RequestId,
    client_name: &str,
    client_credential: Option<&str>,
) -> DaemonInitializeResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_INITIALIZE,
            Some(
                serde_json::to_value(DaemonInitializeParams {
                    client_name: client_name.to_string(),
                    client_credential: client_credential.map(str::to_string),
                    client_version: "0.0.1".to_string(),
                    protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                    capabilities: DaemonClientCapabilities {
                        notifications: true,
                        event_subscriptions: true,
                    },
                })
                .expect("initialize params should serialize"),
            ),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    let initialized =
        serde_json::from_value(response.result).expect("initialize result should deserialize");
    initialized
}

pub fn subscribe_run_events(
    stream: &mut SocketConnection,
    request_id: RequestId,
) -> DaemonSubscribeResult {
    subscribe_events(stream, request_id, &[DaemonEventKind::Run])
}

pub fn subscribe_events(
    stream: &mut SocketConnection,
    request_id: RequestId,
    kinds: &[DaemonEventKind],
) -> DaemonSubscribeResult {
    subscribe_events_after_cursor(stream, request_id, kinds, None)
}

pub fn subscribe_events_after_cursor(
    stream: &mut SocketConnection,
    request_id: RequestId,
    kinds: &[DaemonEventKind],
    after_cursor: Option<ta_protocol::wire::DaemonEventCursor>,
) -> DaemonSubscribeResult {
    let codec = JsonLineCodec;
    write_request(
        &codec,
        stream,
        JsonRpcRequest::new(
            request_id.clone(),
            METHOD_DAEMON_SUBSCRIBE,
            Some(json!({
                "kinds": kinds,
                "afterCursor": after_cursor,
            })),
        ),
    );
    let response = read_response(&codec, stream);
    assert_eq!(response.id, request_id);
    serde_json::from_value(response.result).expect("subscribe result should deserialize")
}

pub fn connect_remote_websocket(
    remote_bind: &str,
    auth_token: &str,
) -> Result<
    tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    tungstenite::Error,
> {
    let uri: tungstenite::http::Uri = format!("ws://{remote_bind}/rpc")
        .parse()
        .expect("remote websocket uri should parse");
    let request = tungstenite::ClientRequestBuilder::new(uri)
        .with_header("Authorization", format!("Bearer {auth_token}"));
    let (socket, _response) = tungstenite::connect(request)?;
    Ok(socket)
}

pub fn initialize_remote_client_with_credential(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    request_id: i64,
    client_credential: Option<&str>,
) {
    write_remote_request(
        socket,
        JsonRpcRequest::new(
            RequestId::Integer(request_id),
            METHOD_DAEMON_INITIALIZE,
            Some(
                serde_json::to_value(DaemonInitializeParams {
                    client_name: "ta-orchestrator-tests".to_string(),
                    client_credential: client_credential.map(str::to_string),
                    client_version: "0.0.1".to_string(),
                    protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                    capabilities: DaemonClientCapabilities {
                        notifications: true,
                        event_subscriptions: true,
                    },
                })
                .expect("initialize params should serialize"),
            ),
        ),
    );
    let response = read_remote_response(socket);
    assert_eq!(response.id, RequestId::Integer(request_id));
    let _: DaemonInitializeResult =
        serde_json::from_value(response.result).expect("initialize result should deserialize");
}

pub fn attach_remote_session(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    request_id: i64,
    session_id: SessionId,
    session_authority: SessionAuthority,
) -> DaemonSessionAttachResult {
    write_remote_request(
        socket,
        JsonRpcRequest::new(
            RequestId::Integer(request_id),
            METHOD_DAEMON_SESSION_ATTACH,
            Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id,
                    session_authority,
                })
                .expect("remote session attach params should serialize"),
            ),
        ),
    );
    let response = read_remote_response(socket);
    assert_eq!(response.id, RequestId::Integer(request_id));
    serde_json::from_value(response.result)
        .expect("remote session attach result should deserialize")
}

pub fn subscribe_remote_events_after_cursor(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    request_id: i64,
    kinds: &[DaemonEventKind],
    after_cursor: Option<DaemonEventCursor>,
) -> DaemonSubscribeResult {
    write_remote_request(
        socket,
        JsonRpcRequest::new(
            RequestId::Integer(request_id),
            METHOD_DAEMON_SUBSCRIBE,
            Some(json!({
                "kinds": kinds,
                "afterCursor": after_cursor,
            })),
        ),
    );
    let response = read_remote_response(socket);
    assert_eq!(response.id, RequestId::Integer(request_id));
    serde_json::from_value(response.result).expect("remote subscribe result should deserialize")
}

#[allow(dead_code)]
pub fn decide_remote_approval(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    request_id: i64,
    approval_id: ApprovalId,
    decision: ApprovalDecision,
) -> DaemonApprovalDecideResult {
    write_remote_request(
        socket,
        JsonRpcRequest::new(
            RequestId::Integer(request_id),
            METHOD_DAEMON_APPROVAL_DECIDE,
            Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id,
                    decision,
                    commentary: None,
                })
                .expect("remote approval decide params should serialize"),
            ),
        ),
    );
    let response = read_remote_response(socket);
    assert_eq!(response.id, RequestId::Integer(request_id));
    serde_json::from_value(response.result)
        .expect("remote approval decide result should deserialize")
}

pub fn write_remote_request(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    request: JsonRpcRequest,
) {
    let payload = serde_json::to_string(&JsonRpcMessage::Request(request))
        .expect("remote request should serialize");
    socket
        .send(Message::Text(payload.into()))
        .expect("remote websocket request should send");
}

pub fn read_remote_response(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> JsonRpcResponse {
    match read_remote_message(socket) {
        JsonRpcMessage::Response(response) => response,
        JsonRpcMessage::Error(error) => panic!("expected remote response, got error {error:?}"),
        JsonRpcMessage::Request(request) => {
            panic!("expected remote response, got request {request:?}")
        }
        JsonRpcMessage::Notification(notification) => {
            panic!("expected remote response, got notification {notification:?}")
        }
    }
}

#[allow(dead_code)]
pub fn read_remote_event_notification(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> DaemonEventEnvelope {
    match read_remote_message(socket) {
        JsonRpcMessage::Notification(notification) if notification.method == "daemon.event" => {
            let params = notification
                .params
                .expect("remote event params should exist");
            serde_json::from_value(params).expect("remote event notification should deserialize")
        }
        JsonRpcMessage::Notification(notification) => {
            panic!("expected remote event notification, got notification {notification:?}")
        }
        JsonRpcMessage::Request(request) => {
            panic!("expected remote event notification, got request {request:?}")
        }
        JsonRpcMessage::Response(response) => {
            panic!("expected remote event notification, got response {response:?}")
        }
        JsonRpcMessage::Error(error) => {
            panic!("expected remote event notification, got error {error:?}")
        }
    }
}

#[allow(dead_code)]
pub fn read_remote_terminal_run_event(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    run_id: &RunId,
) -> DaemonEventEnvelope {
    loop {
        let envelope = read_remote_event_notification(socket);
        if matches!(
            &envelope.event,
            ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent::Status(status_event))
                if status_event.run_id() == run_id && matches!(status_event.status(),
                    ta_protocol::wire::RunStatus::Completed
                    | ta_protocol::wire::RunStatus::Failed
                    | ta_protocol::wire::RunStatus::Cancelled,
                )
        ) {
            return envelope;
        }
    }
}

pub fn read_remote_message(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> JsonRpcMessage {
    loop {
        match socket.read().expect("remote websocket should read") {
            Message::Text(payload) => {
                return serde_json::from_str(payload.as_str())
                    .expect("remote websocket message should parse");
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .expect("remote websocket pong should send");
            }
            Message::Pong(_) => {}
            Message::Close(frame) => panic!("unexpected remote websocket close frame: {frame:?}"),
            _ => {}
        }
    }
}
