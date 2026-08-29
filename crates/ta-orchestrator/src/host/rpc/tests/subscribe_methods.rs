use super::*;

#[test]
fn navigation_subscribe_requires_initialize_but_not_session_attachment() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let uninitialized = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let request = || JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: crate::RequestId::Integer(401),
        method: METHOD_DAEMON_NAVIGATION_SUBSCRIBE.to_string(),
        params: Some(serde_json::json!({})),
    };

    assert!(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &uninitialized,
            request()
        )
        .is_err()
    );

    let initialized = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: None,
    }));
    let result: DaemonNavigationSubscribeResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &initialized,
            request(),
        )
        .expect("initialized principal may subscribe without an attached session"),
    )
    .expect("empty result");
    assert_eq!(result, DaemonNavigationSubscribeResult {});
}

#[test]
fn navigation_invalidation_is_empty_and_principal_scoped() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let owner_principal_id = issue_test_principal_id(&state, TEST_CLIENT_NAME);
    let owned = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &owner_principal_id,
            &OpenSessionRequest {
                title: "Owned navigation".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let (owner_runtime, mut owner_outbound) =
        JsonRpcConnectionRuntime::new(402, DEFAULT_OUTBOUND_QUEUE_DEPTH);
    let owner_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(owner_principal_id),
        attached_session_id: None,
    }));
    handle_request(
        &state,
        &shutdown_requested,
        &owner_runtime.session(),
        &owner_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(402),
            method: METHOD_DAEMON_NAVIGATION_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect("owner subscribes");
    for action in owner_runtime.take_after_response_actions() {
        action();
    }

    state.runtime.publish_record(&EventRecord {
        sequence: 2,
        session_id: owned.id.clone(),
        occurred_at_ms: 2,
        payload: crate::DaemonEvent::Session(crate::SessionEvent {
            session_id: owned.id.clone(),
            status: SessionStatus::Running,
        }),
    });

    let message = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("notification test runtime")
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(1), owner_outbound.recv())
                .await
                .expect("owner notification should arrive before the deadline")
                .expect("owner notification channel should remain open")
        });
    let JsonRpcMessage::Notification(notification) = message else {
        panic!("expected notification");
    };
    assert_eq!(notification.method, METHOD_DAEMON_NAVIGATION_INVALIDATED);
    assert_eq!(notification.params, Some(serde_json::json!({})));
}

#[test]
fn daemon_session_attach_rejects_unknown_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
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
            id: crate::RequestId::Integer(33),
            method: METHOD_DAEMON_SESSION_ATTACH.to_string(),
            params: Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: SessionId::new("session-missing").expect("session id"),
                    session_authority: test_session_authority(),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.session.attach should reject unknown sessions");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("session does not exist"));
}

#[test]
fn daemon_subscribe_requires_initialize_first() {
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
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("daemon.subscribe should require initialize");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
}

#[test]
fn daemon_subscribe_requires_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
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
            id: crate::RequestId::Integer(41),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({})),
        },
    )
    .expect_err("daemon.subscribe should require attached session");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(
        error
            .message
            .contains("daemon.subscribe requires daemon.session.open or daemon.session.attach")
    );
}

#[test]
fn daemon_subscribe_returns_history_gap_when_cursor_is_stale() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(5),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::Session],
                "afterCursor": {
                    "daemonInstanceId": state.runtime.daemon_instance_id(),
                    "sessionId": opened.id.clone(),
                    "sequence": "0"
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let latest_cursor = state
        .app
        .latest_event_cursor_for_session(
            &session_state
                .lock()
                .expect("session lock")
                .attached_session_id
                .clone()
                .expect("attached session"),
        )
        .expect("latest cursor should load");
    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(result, DaemonSubscribeResult::HistoryGap { latest_cursor });
}

#[test]
fn daemon_subscribe_returns_ready_when_cursor_is_current() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let running = ensure_running_run(&state, &opened.id, "Ship app server hard cut");
    let recorded = state
        .app
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new(format!("artifact-{}", opened.id.as_str())).expect("artifact id"),
            session_id: opened.id.clone(),
            run_id: running.body.id,
            kind: ArtifactKind::Patch,
            metadata: ta_protocol::wire::ArtifactMetadata::Standard,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        })
        .expect("artifact should record");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(6),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::Artifact],
                "afterCursor": {
                    "daemonInstanceId": state.runtime.daemon_instance_id(),
                    "sessionId": opened.id.clone(),
                    "sequence": recorded
                        .deferred_records
                        .last()
                        .expect("artifact event")
                        .sequence
                        .to_string()
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::Ready {
            latest_cursor: Some(DaemonEventCursor {
                daemon_instance_id: state.runtime.daemon_instance_id(),
                session_id: opened.id.clone(),
                sequence: recorded
                    .deferred_records
                    .last()
                    .expect("artifact event")
                    .sequence,
            }),
        }
    );
}

#[test]
fn daemon_subscribe_returns_ready_when_live_lane_backlog_can_resume() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    state.runtime.publish_record(&EventRecord {
        sequence: 1,
        session_id: opened.id.clone(),
        occurred_at_ms: 100,
        payload: crate::DaemonEvent::Session(crate::SessionEvent {
            session_id: opened.id.clone(),
            status: SessionStatus::Idle,
        }),
    });
    state.runtime.publish_record(&EventRecord {
        sequence: 2,
        session_id: opened.id.clone(),
        occurred_at_ms: 200,
        payload: crate::DaemonEvent::AgentStream(AgentStreamEvent {
            run_id: RunId::new("run-1").expect("run id"),
            emission: StreamEmission {
                turn_id: None,
                item_id: None,
                fragment_sequence: Some(1),
                frame: AgentStreamFrame::AssistantMessageDelta {
                    delta: "partial".to_string(),
                },
            },
        }),
    });
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(61),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::AgentStream],
                "afterCursor": {
                    "daemonInstanceId": state.runtime.daemon_instance_id(),
                    "sessionId": opened.id.clone(),
                    "sequence": "1"
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::Ready {
            latest_cursor: Some(DaemonEventCursor {
                daemon_instance_id: state.runtime.daemon_instance_id(),
                session_id: opened.id.clone(),
                sequence: 2,
            }),
        }
    );
}

#[test]
fn daemon_subscribe_caps_history_gap_cursor_to_durable_latest_when_live_only_frames_exist() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_event = EventRecord {
        sequence: 1,
        session_id: opened.id.clone(),
        occurred_at_ms: 100,
        payload: crate::DaemonEvent::Session(crate::SessionEvent {
            session_id: opened.id.clone(),
            status: SessionStatus::Idle,
        }),
    };
    state.runtime.publish_record(&session_event);
    state.runtime.publish_record(&EventRecord {
        sequence: 2,
        session_id: opened.id.clone(),
        occurred_at_ms: 200,
        payload: crate::DaemonEvent::AgentStream(AgentStreamEvent {
            run_id: RunId::new("run-1").expect("run id"),
            emission: StreamEmission {
                turn_id: None,
                item_id: None,
                fragment_sequence: Some(1),
                frame: AgentStreamFrame::AssistantMessageDelta {
                    delta: "partial".to_string(),
                },
            },
        }),
    });
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(62),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::AgentStream],
                "afterCursor": {
                    "daemonInstanceId": "stale-daemon",
                    "sessionId": opened.id.clone(),
                    "sequence": "0"
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: Some(DaemonEventCursor {
                daemon_instance_id: state.runtime.daemon_instance_id(),
                session_id: opened.id.clone(),
                sequence: 1,
            }),
        }
    );
}

#[test]
fn daemon_subscribe_returns_history_gap_when_cursor_epoch_differs() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();
    let current_cursor = state
        .app
        .latest_event_cursor_for_session(&opened.id)
        .expect("latest cursor should load");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(7),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::Session],
                "afterCursor": {
                    "daemonInstanceId": "stale-daemon",
                    "sessionId": opened.id,
                    "sequence": current_cursor
                        .as_ref()
                        .expect("latest cursor")
                        .sequence
                        .to_string()
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: current_cursor,
        }
    );
}

#[test]
fn daemon_subscribe_returns_history_gap_when_cursor_session_differs() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            &issue_test_principal_id(&state, TEST_CLIENT_NAME),
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(issue_test_principal_id(&state, TEST_CLIENT_NAME)),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();
    let current_cursor = state
        .app
        .latest_event_cursor_for_session(&opened.id)
        .expect("latest cursor should load");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(8),
            method: METHOD_DAEMON_SUBSCRIBE.to_string(),
            params: Some(serde_json::json!({
                "kinds": [DaemonEventKind::Session],
                "afterCursor": {
                    "daemonInstanceId": state.runtime.daemon_instance_id(),
                    "sessionId": "session-foreign",
                    "sequence": current_cursor
                        .as_ref()
                        .expect("latest cursor")
                        .sequence
                        .to_string()
                }
            })),
        },
    )
    .expect("daemon.subscribe should succeed");

    let result: DaemonSubscribeResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: current_cursor,
        }
    );
}
