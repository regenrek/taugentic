use super::*;

#[test]
fn daemon_run_start_requires_attached_session() {
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
    let _opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
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
            method: METHOD_DAEMON_RUN_START.to_string(),
            params: Some(
                serde_json::to_value(start_run_command(&state, "Ship app server hard cut"))
                    .expect("params"),
            ),
        },
    )
    .expect_err("daemon.run.start should require attached session");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_RUN_START));
    assert!(error.message.contains("daemon.session.open"));
}

#[test]
fn daemon_run_start_commits_follow_up_transition_before_response() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
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
            id: crate::RequestId::Integer(36),
            method: METHOD_DAEMON_RUN_START.to_string(),
            params: Some(
                serde_json::to_value(start_run_command(&state, "Ship app server hard cut"))
                    .expect("params"),
            ),
        },
    )
    .expect("daemon.run.start should succeed");

    let run: RunSummary = serde_json::from_value(response).expect("response should deserialize");
    let listed = state.app.list_runs(&opened.id).expect("runs should list");
    let selected_session = state
        .app
        .get_session(&opened.id)
        .expect("session should load")
        .expect("session should exist");
    let activity = state
        .app
        .activity_page(
            &opened.id,
            &ActivityPageQuery {
                limit: 10,
                before: None,
                kinds: vec![DaemonEventKind::Run],
            },
        )
        .expect("activity should load");

    assert_eq!(run.status, RunStatus::WaitingForApproval);
    assert_eq!(listed, vec![run.clone()]);
    assert_eq!(selected_session.status, SessionStatus::Running);
    assert_eq!(activity.items.len(), 1);
    assert!(matches!(
        &activity.items[0].event,
        PublicDaemonEvent::Run(crate::RunEvent {
            run_id,
            status,
            detail,
            ..
        })
            if *run_id == run.id
                && *status == RunStatus::WaitingForApproval
                && detail == "Waiting for approval"
    ));
}

#[test]
fn daemon_run_start_queues_a_second_run_behind_the_active_one() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let _first: RunSummary = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(36),
                method: METHOD_DAEMON_RUN_START.to_string(),
                params: Some(
                    serde_json::to_value(start_run_command(&state, "Occupy active slot"))
                        .expect("params"),
                ),
            },
        )
        .expect("first daemon.run.start should succeed"),
    )
    .expect("response should deserialize");

    let queued: RunSummary = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(37),
                method: METHOD_DAEMON_RUN_START.to_string(),
                params: Some(
                    serde_json::to_value(start_run_command(&state, "Queue behind active slot"))
                        .expect("params"),
                ),
            },
        )
        .expect("second daemon.run.start should succeed"),
    )
    .expect("response should deserialize");

    let listed = state.app.list_runs(&opened.id).expect("runs should list");
    let activity = state
        .app
        .activity_page(
            &opened.id,
            &ActivityPageQuery {
                limit: 10,
                before: None,
                kinds: vec![DaemonEventKind::Run],
            },
        )
        .expect("activity should load");

    assert_eq!(queued.status, RunStatus::Queued);
    assert!(
        listed
            .iter()
            .any(|run| run.id == queued.id && run.status == RunStatus::Queued)
    );
    assert!(activity.items.iter().any(|item| {
        matches!(
            &item.event,
            PublicDaemonEvent::Run(crate::RunEvent {
                run_id,
                status,
                detail,
                ..
            })
                if *run_id == queued.id
                    && *status == RunStatus::Queued
                    && detail.contains("Queued behind active run")
        )
    }));
}

#[test]
fn daemon_run_subscribe_events_streams_live_notifications_and_drops_on_disconnect() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&state, &opened.id, "Stream native run");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(opened.id.clone()),
    }));
    let (connection_runtime, mut outbound_rx) =
        JsonRpcConnectionRuntime::new(41, DEFAULT_OUTBOUND_QUEUE_DEPTH);
    let session = connection_runtime.session();

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(38),
            method: METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS.to_string(),
            params: Some(
                serde_json::to_value(SubscribeRunEventsRequest {
                    session_id: opened.id.clone(),
                    run_id: run.body.id.clone(),
                    after_seq: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.run.subscribe_events should succeed");
    let result: SubscribeRunEventsResult =
        serde_json::from_value(response).expect("response should deserialize");
    for action in connection_runtime.take_after_response_actions() {
        action();
    }
    let next_sequence = result.latest_event_seq.expect("seed event sequence") + 1;

    state.runtime.publish_record(&EventRecord {
        sequence: next_sequence,
        session_id: opened.id.clone(),
        occurred_at_ms: next_sequence * 10,
        payload: crate::DaemonEvent::Run(crate::RunEvent {
            run_id: run.body.id.clone(),
            status: RunStatus::Running,
            detail: "live after rpc subscribe".to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        }),
    });
    let item = recv_run_event_stream_item(&mut outbound_rx);
    let RunEventStreamPayload::Delta {
        delta: RunEventDelta { seq, event },
    } = item.payload
    else {
        panic!("expected live delta notification");
    };

    assert_eq!(item.run_id, run.body.id);
    assert_eq!(seq, next_sequence);
    assert!(matches!(
        event,
        PublicDaemonEvent::Run(crate::RunEvent {
            status: RunStatus::Running,
            ..
        })
    ));
    session.close();
    wait_for_subscriber_count(&state, &opened.id, 0);
}

#[test]
fn daemon_run_timeline_returns_root_run_events() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Timeline RPC".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = ensure_running_run(&state, &opened.id, "Timeline root");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
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
            id: crate::RequestId::Integer(39),
            method: METHOD_DAEMON_RUN_TIMELINE.to_string(),
            params: Some(
                serde_json::to_value(GetRunTimelineQuery {
                    session_id: opened.id.clone(),
                    root_run_id: run.body.id.clone(),
                    after_seq: None,
                    limit: Some(10),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.run.timeline should succeed");

    let timeline: RunTimeline = serde_json::from_value(response).expect("timeline response");
    assert_eq!(timeline.root_run_id, run.body.id);
    assert_eq!(timeline.runs.len(), 1);
    assert!(
        timeline
            .events
            .iter()
            .any(|event| event.run_id == run.body.id)
    );
}

#[test]
fn daemon_run_cancel_requires_attached_session() {
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
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run = state
        .app
        .start_run(
            &opened.id,
            &start_run_command(&state, "Ship app server hard cut"),
        )
        .expect("run should start");

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(36),
            method: METHOD_DAEMON_RUN_CANCEL.to_string(),
            params: Some(
                serde_json::to_value(DaemonRunCancelParams {
                    run_id: run.body.id,
                    reason: Some("operator stopped run".to_string()),
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.run.cancel should require attached session");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains(METHOD_DAEMON_RUN_CANCEL));
    assert!(error.message.contains("daemon.session.open"));
}

fn recv_run_event_stream_item(
    outbound_rx: &mut tokio::sync::mpsc::Receiver<JsonRpcMessage>,
) -> RunEventStreamItem {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match outbound_rx.try_recv() {
            Ok(JsonRpcMessage::Notification(notification))
                if notification.method == METHOD_DAEMON_RUN_EVENT =>
            {
                return serde_json::from_value(
                    notification.params.expect("run event params should exist"),
                )
                .expect("run event notification should deserialize");
            }
            Ok(other) => panic!("expected daemon.run.event notification, got {other:?}"),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("timed out waiting for daemon.run.event notification: {error:?}"),
        }
    }
}

fn wait_for_subscriber_count(state: &BootstrapState, session_id: &SessionId, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if state.runtime.subscriber_count_for_session(session_id) == expected {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        state.runtime.subscriber_count_for_session(session_id),
        expected
    );
}

#[test]
fn daemon_run_cancel_redacts_actor_in_public_activity_page_and_marks_run_cancelled() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let opened = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = state
        .app
        .start_run(
            &opened.id,
            &start_run_command(&state, "Ship app server hard cut"),
        )
        .expect("run should start");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
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
            id: crate::RequestId::Integer(37),
            method: METHOD_DAEMON_RUN_CANCEL.to_string(),
            params: Some(
                serde_json::to_value(DaemonRunCancelParams {
                    run_id: started.body.id.clone(),
                    reason: Some("operator stopped run".to_string()),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.run.cancel should succeed");

    let run: RunSummary = serde_json::from_value(response).expect("response should deserialize");
    let selected = state
        .app
        .get_run(
            &opened.id,
            &GetRunQuery {
                run_id: started.body.id.clone(),
            },
        )
        .expect("run should load")
        .expect("run should exist");
    let approvals = state
        .app
        .list_approvals(
            &opened.id,
            &ListApprovalsQuery {
                run_id: Some(started.body.id.clone()),
                approval_id: None,
            },
        )
        .expect("approvals should list");
    let selected_session = state
        .app
        .get_session(&opened.id)
        .expect("session should load")
        .expect("session should exist");
    let activity = state
        .app
        .activity_page(
            &opened.id,
            &ActivityPageQuery {
                limit: 10,
                before: None,
                kinds: vec![DaemonEventKind::Approval],
            },
        )
        .expect("approval activity should load");
    let _resolution = activity
        .items
        .iter()
        .find_map(|item| match &item.event {
            PublicDaemonEvent::Approval(PublicApprovalEvent::Resolved { resolution })
                if resolution.run_id == started.body.id =>
            {
                Some(resolution)
            }
            _ => None,
        })
        .expect("resolved approval activity should exist");

    assert_eq!(run.status, RunStatus::Cancelled);
    assert_eq!(selected.summary.status, RunStatus::Cancelled);
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());
    assert_eq!(selected_session.status, SessionStatus::Idle);
}
