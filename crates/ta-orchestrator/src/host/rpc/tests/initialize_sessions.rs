use super::*;

#[test]
fn daemon_initialize_marks_session_ready_for_subscriptions() {
    let state = boot(test_config());
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
            id: crate::RequestId::Integer(3),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": "desktop-main",
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should succeed");

    let result: DaemonInitializeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(result.protocol_version, DAEMON_PROTOCOL_VERSION);
    assert_eq!(
        result.daemon_instance_id,
        state.runtime.daemon_instance_id()
    );
    assert!(result.client_credential.len() >= 32);
    assert!(session_state.lock().expect("session lock").initialized);
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .client_credential
            .as_deref(),
        Some(result.client_credential.as_str())
    );
    assert!(
        session_state
            .lock()
            .expect("session lock")
            .principal_id
            .as_deref()
            .is_some_and(|principal_id| principal_id.starts_with("principal-"))
    );
    assert!(
        session_state
            .lock()
            .expect("session lock")
            .attached_session_id
            .is_none()
    );
}

#[test]
fn daemon_initialize_rejects_empty_client_name() {
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
            id: crate::RequestId::Integer(4),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": "   ",
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect_err("daemon.initialize should reject empty client name");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("non-empty clientName"));
}

#[test]
fn daemon_initialize_reuses_supplied_client_credential() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let issued_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(5),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": TEST_CLIENT_NAME,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should issue a new credential");
    let issued: DaemonInitializeResult =
        serde_json::from_value(issued_response).expect("response should deserialize");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(5),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": TEST_CLIENT_NAME,
                "clientCredential": issued.client_credential,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should succeed with supplied credential");

    let result: DaemonInitializeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(result.client_credential, issued.client_credential);
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .client_credential
            .as_deref(),
        Some(issued.client_credential.as_str())
    );
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .client_name
            .as_deref(),
        Some(TEST_CLIENT_NAME)
    );
}

#[test]
fn daemon_initialize_reuses_canonical_client_name_for_known_credential() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let session = test_session();

    let issued_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(5),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": TEST_CLIENT_NAME,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should issue a new credential");
    let issued: DaemonInitializeResult =
        serde_json::from_value(issued_response).expect("response should deserialize");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(6),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": "spoofed-client",
                "clientCredential": issued.client_credential,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should reuse known credential");
    let result: DaemonInitializeResult =
        serde_json::from_value(response).expect("response should deserialize");

    assert_eq!(result.client_credential, issued.client_credential);
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .client_name
            .as_deref(),
        Some(TEST_CLIENT_NAME)
    );
}

#[test]
fn daemon_initialize_rotates_unknown_client_credential() {
    let state = boot(test_config());
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
            id: crate::RequestId::Integer(6),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": TEST_CLIENT_NAME,
                "clientCredential": TEST_CLIENT_CREDENTIAL,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true
                }
            })),
        },
    )
    .expect("daemon.initialize should succeed by rotating unknown credential");

    let result: DaemonInitializeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_ne!(result.client_credential, TEST_CLIENT_CREDENTIAL);
    assert!(
        session_state
            .lock()
            .expect("session lock")
            .principal_id
            .as_deref()
            .is_some_and(|principal_id| principal_id.starts_with("principal-"))
    );
}

#[test]
fn daemon_session_attach_requires_initialize_first() {
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
            id: crate::RequestId::Integer(31),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: SessionId::new("session-1").expect("session id"),
                    session_authority: test_session_authority(),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.session.attach should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_SESSION_ATTACH));
}

#[test]
fn daemon_session_open_requires_initialize_first() {
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
            id: crate::RequestId::Integer(30),
            method: METHOD_DAEMON_SESSION_OPEN.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionOpenParams {
                    title: "Build daemon app server".to_string(),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.session.open should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_SESSION_OPEN));
}

#[test]
fn daemon_session_open_creates_idle_session_summary() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(31),
            method: METHOD_DAEMON_SESSION_OPEN.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionOpenParams {
                    title: "Build daemon app server".to_string(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.session.open should succeed");

    let opened: DaemonSessionOpenResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert!(opened.session.id.as_str().starts_with("session-"));
    assert_eq!(opened.session.title, "Build daemon app server");
    assert_eq!(opened.session.status, SessionStatus::Idle);
    assert!(
        opened
            .latest_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.sequence > 0)
    );
    assert!(opened.session_authority.as_str().len() >= 32);
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .attached_session_id,
        Some(opened.session.id)
    );
}

#[test]
fn daemon_session_open_rejects_blank_title() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(32),
            method: METHOD_DAEMON_SESSION_OPEN.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionOpenParams {
                    title: "   ".to_string(),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.session.open should reject blank titles");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("session title must not be empty"));
}

#[test]
fn daemon_session_attach_returns_session_summary_and_records_binding() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();

    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(32),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: opened.session_authority.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.session.attach should succeed");

    let attached: DaemonSessionAttachResult =
        serde_json::from_value(response).expect("response should deserialize");
    let latest_cursor = state
        .app
        .latest_event_cursor_for_session(&opened.id)
        .expect("latest cursor should load");
    assert_eq!(attached.session.id, opened.id);
    assert_eq!(attached.session.title, "Build daemon app server");
    assert_eq!(attached.latest_cursor, latest_cursor);
    assert_ne!(attached.session_authority, opened.session_authority);
    let opened_session_authority = opened.session_authority.clone();
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .attached_session_id,
        Some(attached.session.id)
    );

    let recovered = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(33),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: opened_session_authority.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("recovery authority should recover once");
    let recovered: DaemonSessionAttachResult =
        serde_json::from_value(recovered).expect("response should deserialize");
    assert_ne!(recovered.session_authority, attached.session_authority);

    let stale_attached_error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(34),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: attached.session_authority,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("consumed recovery flow should not leave attached authority valid");
    assert_eq!(stale_attached_error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(
        stale_attached_error
            .message
            .contains("session authority rejected")
    );

    let stale_error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(35),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: opened_session_authority,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("oldest authority should fail");
    assert_eq!(stale_error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(stale_error.message.contains("session authority rejected"));
}

#[test]
fn daemon_session_attach_returns_latest_runtime_cursor_when_available() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(34),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: opened.session_authority.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.session.attach should succeed");

    let latest_cursor = state
        .app
        .latest_event_cursor_for_session(&opened.id)
        .expect("latest cursor should load");
    let attached: DaemonSessionAttachResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(attached.latest_cursor, latest_cursor);
    assert_eq!(
        session_state
            .lock()
            .expect("session lock")
            .attached_session_id,
        Some(opened.id.clone())
    );
}

#[test]
fn daemon_session_attach_rejects_foreign_owned_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(OTHER_TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(OTHER_TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));
    let session = test_session();
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(35),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.id.clone(),
                    session_authority: opened.session_authority,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.session.attach should collapse foreign ownership to not found");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("session does not exist"));
}
