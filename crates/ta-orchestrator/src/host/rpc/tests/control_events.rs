use super::*;

#[test]
fn daemon_control_status_returns_runtime_control_snapshot() {
    with_test_config_home("rpc-control-status", || {
        let mut config = test_config();
        config.server = ServerConfig::local_default("ta-daemon", DAEMON_DEFAULT_SOCKET_NAME);
        let state = boot(config);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        let session = test_session();

        let response = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(11),
                method: METHOD_DAEMON_CONTROL_STATUS.to_string(),
                params: Some(serde_json::json!({})),
            },
        )
        .expect("daemon.control.status should succeed");

        let status: DaemonControlStatusResult =
            serde_json::from_value(response).expect("response should deserialize");
        assert_eq!(
            status.socket_path,
            state.config.socket_address().to_string()
        );
        assert_eq!(
            status.log_path,
            state.config.log_path().display().to_string()
        );
        assert_eq!(status.protocol_version, DAEMON_PROTOCOL_VERSION);
    });
}

#[test]
fn mutating_public_control_methods_are_not_exposed() {
    with_test_config_home("rpc-control-public-disabled", || {
        let state = boot(test_config());
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
        let session = test_session();
        for (id, method) in [
            (12, "daemon.background.enable"),
            (13, "daemon.background.disable"),
            (14, "daemon.control.reconcile"),
            (15, "daemon.control.stop"),
        ] {
            let error = handle_request(
                &state,
                &shutdown_requested,
                &session,
                &session_state,
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: crate::RequestId::Integer(id),
                    method: method.to_string(),
                    params: Some(serde_json::json!({})),
                },
            )
            .expect_err("mutating public control rpc should be disabled");

            assert_eq!(error.code, METHOD_NOT_FOUND_ERROR_CODE);
            assert_eq!(error.message, format!("method not found: {method}"));
        }
        assert!(!shutdown_requested.load(Ordering::SeqCst));
    });
}

#[test]
fn event_forwarder_closes_session_when_runtime_subscription_overflows() {
    let session = test_session();
    let (_sender, receiver) = mpsc::channel();
    let overflowed = Arc::new(AtomicBool::new(true));

    spawn_event_forwarder(session.clone(), Vec::new(), receiver, overflowed, None);

    let deadline = Instant::now() + Duration::from_secs(1);
    while session.is_open() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    assert!(!session.is_open());
}

#[test]
fn event_forwarder_closes_session_when_overflow_disconnect_races_before_next_poll() {
    let session = test_session();
    let (sender, receiver) = mpsc::channel();
    let overflowed = Arc::new(AtomicBool::new(false));

    spawn_event_forwarder(
        session.clone(),
        Vec::new(),
        receiver,
        Arc::clone(&overflowed),
        None,
    );
    thread::sleep(Duration::from_millis(10));
    overflowed.store(true, Ordering::SeqCst);
    drop(sender);

    let deadline = Instant::now() + Duration::from_secs(1);
    while session.is_open() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    assert!(!session.is_open());
}

#[test]
fn event_forwarder_closes_session_when_subscriber_disconnects_without_overflow() {
    let session = test_session();
    let (sender, receiver) = mpsc::channel();
    let overflowed = Arc::new(AtomicBool::new(false));

    spawn_event_forwarder(session.clone(), Vec::new(), receiver, overflowed, None);
    thread::sleep(Duration::from_millis(10));
    drop(sender);

    let deadline = Instant::now() + Duration::from_secs(1);
    while session.is_open() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    assert!(!session.is_open());
}

#[test]
fn serialization_failure_closes_session() {
    let session = test_session();
    let error = serde_json::from_str::<serde_json::Value>("not-json").expect_err("invalid json");

    close_session_for_event_serialization_failure(&session, &error);

    assert!(!session.is_open());
}

#[test]
fn unknown_method_returns_json_rpc_error() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();
    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(1),
            method: "unknown.method".to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("unknown method should fail");

    assert_eq!(error.code, METHOD_NOT_FOUND_ERROR_CODE);
}

#[test]
fn daemon_internal_stop_requests_shutdown_and_returns_ack() {
    let mut config = test_config();
    config.control_token = Some(ControlToken::new("control-token".to_string()));
    let state = boot(config);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        HANDOFF_CLIENT_NAME,
    );
    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(2),
            method: METHOD_DAEMON_INTERNAL_STOP.to_string(),
            params: Some(serde_json::json!({ "controlToken": "control-token" })),
        },
    )
    .expect("daemon.internal.stop should succeed");

    let result: InternalDaemonStopResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(result, InternalDaemonStopResult { stopping: true });
    assert!(shutdown_requested.load(Ordering::SeqCst));
}

#[test]
fn daemon_internal_stop_rejects_missing_control_token() {
    let mut config = test_config();
    config.control_token = Some(ControlToken::new("control-token".to_string()));
    let state = boot(config);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        HANDOFF_CLIENT_NAME,
    );

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(3),
            method: METHOD_DAEMON_INTERNAL_STOP.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("daemon.internal.stop without a token should fail");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(
        error.message.contains("missing field") || error.message.contains("control token"),
        "unexpected error message: {}",
        error.message
    );
    assert!(!shutdown_requested.load(Ordering::SeqCst));
}

#[test]
fn daemon_internal_stop_rejects_mismatched_control_token() {
    let mut config = test_config();
    config.control_token = Some(ControlToken::new("control-token".to_string()));
    let state = boot(config);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        HANDOFF_CLIENT_NAME,
    );

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(4),
            method: METHOD_DAEMON_INTERNAL_STOP.to_string(),
            params: Some(serde_json::json!({ "controlToken": "wrong-token" })),
        },
    )
    .expect_err("daemon.internal.stop with the wrong token should fail");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("control token mismatch"));
    assert!(!shutdown_requested.load(Ordering::SeqCst));
}

#[test]
fn daemon_internal_stop_rejects_non_handoff_client() {
    let mut config = test_config();
    config.control_token = Some(ControlToken::new("control-token".to_string()));
    let state = boot(config);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        "desktop-main",
    );

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(5),
            method: METHOD_DAEMON_INTERNAL_STOP.to_string(),
            params: Some(serde_json::json!({ "controlToken": "control-token" })),
        },
    )
    .expect_err("daemon.internal.stop from a public client should fail");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(
        error
            .message
            .contains("reserved for the internal handoff client")
    );
    assert!(!shutdown_requested.load(Ordering::SeqCst));
}
