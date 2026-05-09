use super::*;

#[test]
fn daemon_approval_decide_requires_matching_attached_session() {
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
            &StartRunCommand {
                objective: "Ship app server hard cut".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("test-client".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(SessionId::new("session-other").expect("session id")),
    }));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(37),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id,
                    decision: ApprovalDecision::Approved,
                    commentary: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("daemon.approval.decide should fail when approval is outside attached session");

    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert!(error.message.contains("approval"));
}

#[test]
fn daemon_approval_decide_collapses_missing_and_resolved_public_errors() {
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
            &StartRunCommand {
                objective: "Ship app server hard cut".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    let missing_error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(38),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id: crate::ApprovalId::new("approval-missing").expect("approval id"),
                    decision: ApprovalDecision::Approved,
                    commentary: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("missing approval should return a public invalid-params error");

    handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(39),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id: approval_id.clone(),
                    decision: ApprovalDecision::Approved,
                    commentary: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect("first approval decision should succeed");

    let resolved_error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(40),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id,
                    decision: ApprovalDecision::Rejected,
                    commentary: None,
                })
                .expect("params"),
            ),
        },
    )
    .expect_err("resolved approval should return the same public invalid-params error");

    assert_eq!(missing_error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert_eq!(resolved_error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert_eq!(missing_error.message, "approval is not pending");
    assert_eq!(resolved_error.message, missing_error.message);
}

#[test]
fn daemon_approval_actor_uses_initialized_principal_id() {
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("desktop-main".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: None,
    }));

    let actor = approval_actor_from_session(&session_state, METHOD_DAEMON_APPROVAL_DECIDE)
        .expect("approval actor should resolve");

    assert_eq!(actor.principal_id, TEST_OWNER_PRINCIPAL_ID);
}

#[test]
fn daemon_approval_approve_resumes_run_and_clears_pending_approval() {
    assert_approval_decision_transitions_run_and_clears_pending_approval(
        ApprovalDecision::Approved,
        Some("looks safe".to_string()),
        RunStatus::Running,
    );
}

#[test]
fn daemon_approval_reject_fails_run_and_clears_pending_approval() {
    assert_approval_decision_transitions_run_and_clears_pending_approval(
        ApprovalDecision::Rejected,
        Some("rejecting unsafe request".to_string()),
        RunStatus::Failed,
    );
}

fn assert_approval_decision_transitions_run_and_clears_pending_approval(
    decision: ApprovalDecision,
    commentary: Option<String>,
    expected_status: RunStatus,
) {
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
            &StartRunCommand {
                objective: "Ship app server hard cut".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");
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
            id: crate::RequestId::Integer(38),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id,
                    decision,
                    commentary,
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.approval.decide should succeed");

    let decided: DaemonApprovalDecideResult =
        serde_json::from_value(response).expect("response should deserialize");
    let run = state
        .app
        .get_run(
            &opened.id,
            &GetRunQuery {
                run_id: started.body.id,
            },
        )
        .expect("run should load")
        .expect("run should exist");
    let approvals = state
        .app
        .list_approvals(
            &opened.id,
            &ListApprovalsQuery {
                run_id: Some(run.summary.id.clone()),
                approval_id: None,
            },
        )
        .expect("approvals should list");

    if expected_status == RunStatus::Running {
        // Approval resumes the run; host runtime availability may immediately move it onward.
        assert_ne!(decided.run.status, RunStatus::WaitingForApproval);
        assert_ne!(run.summary.status, RunStatus::WaitingForApproval);
    } else {
        assert_eq!(decided.run.status, expected_status);
        assert_eq!(run.summary.status, expected_status);
    }
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());
}

#[test]
fn daemon_approval_decide_redacts_actor_in_public_activity_page() {
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
            &StartRunCommand {
                objective: "Ship app server hard cut".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some("desktop-main".to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(TEST_OWNER_PRINCIPAL_ID.to_string()),
        attached_session_id: Some(opened.id.clone()),
    }));
    let session = test_session();

    handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(39),
            method: METHOD_DAEMON_APPROVAL_DECIDE.to_string(),
            params: Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id: approval_id.clone(),
                    decision: ApprovalDecision::Approved,
                    commentary: Some("looks safe".to_string()),
                })
                .expect("params"),
            ),
        },
    )
    .expect("daemon.approval.decide should succeed");

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
                if resolution.approval_id == approval_id =>
            {
                Some(resolution)
            }
            _ => None,
        })
        .expect("resolved approval activity should exist");
}
