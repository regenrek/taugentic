use super::*;

#[test]
fn daemon_session_overview_returns_daemon_owned_visualizer_projection() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
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
    state
        .app
        .open_session(
            "other-client",
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Ignore me".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("other session should open");
    let started = state
        .app
        .start_run(
            &opened.id,
            &StartRunCommand {
                objective: "needs approval".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    assert_eq!(started.body.status, RunStatus::WaitingForApproval);

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(14),
            method: METHOD_DAEMON_SESSION_OVERVIEW.to_string(),
            params: Some(serde_json::to_value(SessionOverviewQuery::default()).expect("params")),
        },
    )
    .expect("daemon.session.overview should succeed");

    let snapshot: SessionOverviewResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].session.id, opened.id);
    assert_eq!(
        snapshot.sessions[0].lane_status,
        SessionOverviewLaneStatus::WaitingForApproval
    );
    assert_eq!(snapshot.sessions[0].pending_approval_count, 1);
    assert_eq!(
        snapshot.sessions[0]
            .latest_run
            .as_ref()
            .map(|run| run.status),
        Some(RunStatus::WaitingForApproval)
    );
}

#[test]
fn daemon_run_list_filters_by_session() {
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
    let other = state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Ignore me".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    state
        .app
        .start_run(
            &opened.id,
            &StartRunCommand {
                objective: "Build daemon app server".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    state
        .app
        .start_run(
            &other.id,
            &StartRunCommand {
                objective: "Ignore me".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(13),
            method: METHOD_DAEMON_RUN_LIST.to_string(),
            params: Some(serde_json::to_value(ListRunsQuery {}).expect("params")),
        },
    )
    .expect("daemon.run.list should succeed");

    let runs: Vec<RunSummary> =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].objective, "Build daemon app server");
}

#[test]
fn daemon_approval_list_filters_by_session() {
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
    let started = state
        .app
        .start_run(
            &opened.id,
            &StartRunCommand {
                objective: "Build daemon app server".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(14),
            method: METHOD_DAEMON_APPROVAL_LIST.to_string(),
            params: Some(
                serde_json::to_value(ListApprovalsQuery {
                    run_id: None,
                    approval_id: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.approval.list should succeed");

    let approvals: ApprovalSnapshotResult =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].run_id, started.body.id);
    assert!(approvals.latest_cursor.is_some());
}

#[test]
fn daemon_run_get_returns_none_outside_selected_session() {
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
    let started = state
        .app
        .start_run(
            &opened.id,
            &StartRunCommand {
                objective: "Build daemon app server".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let selected_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(14),
            method: METHOD_DAEMON_RUN_GET.to_string(),
            params: Some(
                serde_json::to_value(GetRunQuery {
                    run_id: started.body.id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.run.get should succeed");

    let other_session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(SessionId::new("session-2").expect("session id")),
    }));
    let other_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &other_session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(15),
            method: METHOD_DAEMON_RUN_GET.to_string(),
            params: Some(
                serde_json::to_value(GetRunQuery {
                    run_id: started.body.id.clone(),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.run.get should return attached-session scoped none");

    let selected: Option<RunDetail> =
        serde_json::from_value(selected_response).expect("response should deserialize");
    let other: Option<RunDetail> =
        serde_json::from_value(other_response).expect("response should deserialize");

    assert_eq!(
        selected.expect("run should exist").summary.objective,
        "Build daemon app server"
    );
    assert_eq!(other, None);
}

#[test]
fn daemon_session_get_returns_optional_summary() {
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
    state
        .app
        .start_run(
            &opened.id,
            &StartRunCommand {
                objective: "Build daemon app server".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
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
            id: crate::RequestId::Integer(14),
            method: METHOD_DAEMON_SESSION_GET.to_string(),
            params: Some(serde_json::to_value(GetSessionQuery {}).expect("params")),
        },
    )
    .expect("daemon.session.get should succeed");

    let session: Option<SessionSummary> =
        serde_json::from_value(response).expect("response should deserialize");
    assert_eq!(
        session.expect("session should exist").status,
        SessionStatus::Running
    );
}
