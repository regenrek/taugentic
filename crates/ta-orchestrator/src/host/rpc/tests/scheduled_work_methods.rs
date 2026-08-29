use super::*;

#[test]
fn scheduled_work_rpc_list_requires_initialized_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(735),
            method: crate::METHOD_DAEMON_SCHEDULED_WORK_LIST.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("scheduled work list must require initialization");
    assert!(error.message.contains("daemon.initialize"));

    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        TEST_CLIENT_NAME,
    );
    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(736),
            method: crate::METHOD_DAEMON_SCHEDULED_WORK_LIST.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("scheduled work list must require attachment");
    assert!(
        error
            .message
            .contains("daemon.session.open or daemon.session.attach")
    );
}
