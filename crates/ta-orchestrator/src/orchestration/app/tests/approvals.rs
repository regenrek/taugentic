use super::*;

#[test]
fn list_approvals_filters_by_session_and_returns_latest_first() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session_a = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session A".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_b = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session B".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let first = service
        .start_run(&session_a.id, &start_run_command(&service, "one"))
        .expect("run should start");
    service
        .start_run(&session_b.id, &start_run_command(&service, "other session"))
        .expect("run should start");
    let latest = service
        .start_run(&session_a.id, &start_run_command(&service, "latest"))
        .expect("run should start");
    let first_approval_id = first
        .requested_approval_id()
        .expect("expected approval request event");

    let approvals = service
        .list_approvals(
            &session_a.id,
            &ListApprovalsQuery {
                run_id: None,
                approval_id: None,
            },
        )
        .expect("approvals");

    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].run_id, first.body.id);
    assert_eq!(approvals.items[0].reason, "execute run executes a process");
    assert_eq!(latest.body.status, crate::RunStatus::Queued);
    assert!(approvals.latest_cursor.is_some());

    let selected = service
        .list_approvals(
            &session_a.id,
            &ListApprovalsQuery {
                run_id: None,
                approval_id: Some(first_approval_id),
            },
        )
        .expect("filtered approvals");

    assert_eq!(selected.items.len(), 1);
    assert_eq!(selected.items[0].run_id, first.body.id);
    assert!(selected.latest_cursor.is_some());
}

#[test]
fn approving_pending_run_resumes_run_and_clears_approval() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(
            &session.id,
            &start_run_command(&service, "Ship app server hard cut"),
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");

    let decided = service
        .decide_approval(
            &session.id,
            &approval_actor(),
            &DaemonApprovalDecideParams {
                approval_id: approval_id.clone(),
                decision: crate::ApprovalDecision::Approved,
                commentary: Some("looks safe".to_string()),
            },
        )
        .expect("approval should decide");

    let selected_run = service
        .get_run(
            &session.id,
            &GetRunQuery {
                run_id: started.body.id.clone(),
            },
        )
        .expect("run lookup")
        .expect("run should exist");
    let approvals = service
        .list_approvals(
            &session.id,
            &ListApprovalsQuery {
                run_id: Some(started.body.id.clone()),
                approval_id: None,
            },
        )
        .expect("approvals");
    let activity = service
        .activity_page(
            &session.id,
            &ActivityPageQuery {
                limit: 10,
                before: None,
                kinds: vec![DaemonEventKind::Run, DaemonEventKind::Approval],
            },
        )
        .expect("activity");
    service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    // Approval must release the run from the approval gate. On hosts without a
    // usable runtime, the resumed run can immediately advance to Failed.
    assert_ne!(decided.body.status, crate::RunStatus::WaitingForApproval);
    assert_ne!(
        selected_run.summary.status,
        crate::RunStatus::WaitingForApproval
    );
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());
    assert!(activity.items.len() >= 4);
    assert!(decided.deferred_records.iter().any(|event| matches!(
        &event.payload,
        DaemonEvent::Approval(crate::ApprovalEvent::Resolved { resolution })
            if resolution.approval_id == approval_id
            && resolution.run_id == started.body.id
            && resolution.decision == crate::ApprovalDecision::Approved
            && resolution.actor.as_ref().map(|actor| actor.principal_id.as_str())
                == Some(TEST_OWNER_PRINCIPAL_ID)
            && resolution.commentary.as_deref() == Some("looks safe")
    )));
    assert!(decided.deferred_records.iter().any(|event| matches!(
        &event.payload,
        DaemonEvent::Run(crate::RunEvent {
            run_id,
            status,
            detail,
            ..
        }) if *run_id == started.body.id
            && *status == crate::RunStatus::Running
            && detail == "Approval granted"
    )));
    let _activity_resolution = activity
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

#[test]
fn rejecting_one_run_keeps_session_running_when_another_run_is_still_active() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");

    let first = service
        .start_run(
            &session.id,
            &StartRunCommand::new(
                "Run A",
                crate::orchestration::test_runtime_selection(&service, "runtime-codex-deny"),
            ),
        )
        .expect("first run should start");
    assert_eq!(first.body.status, crate::RunStatus::Failed);

    let second = ensure_running_run(&service, &session.id, "Run B");
    assert_eq!(second.body.status, crate::RunStatus::Running);

    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(selected_session.status, SessionStatus::Running);
}

#[test]
fn rejecting_only_run_marks_session_failed() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(
            &session.id,
            &start_run_command(&service, "Reject pending run"),
        )
        .expect("run should start");
    let approval_id = started
        .requested_approval_id()
        .expect("expected approval request event");

    let decided = service
        .decide_approval(
            &session.id,
            &approval_actor(),
            &DaemonApprovalDecideParams {
                approval_id,
                decision: crate::ApprovalDecision::Rejected,
                commentary: Some("not safe".to_string()),
            },
        )
        .expect("approval should reject");
    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(decided.body.status, crate::RunStatus::Failed);
    assert_eq!(selected_session.status, SessionStatus::Failed);
}

#[test]
fn cancelling_one_run_keeps_session_running_when_another_runs_active() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let _running = ensure_running_run(&service, &session.id, "Keep running");
    let queued = service
        .start_run(
            &session.id,
            &start_run_command(&service, "Cancel queued run"),
        )
        .expect("queued run should start");
    assert_eq!(queued.body.status, crate::RunStatus::Queued);

    let cancelled = service
        .cancel_run(
            &session.id,
            &approval_actor(),
            &queued.body.id,
            Some("operator stopped queued run".to_string()),
        )
        .expect("queued run should cancel");
    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(cancelled.body.status, crate::RunStatus::Cancelled);
    assert_eq!(selected_session.status, SessionStatus::Running);
}

#[test]
fn cancel_run_projects_cancelled_run_and_clears_pending_approval() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(
            &session.id,
            &start_run_command(&service, "Ship app server hard cut"),
        )
        .expect("run should start");

    let cancelled = service
        .cancel_run(
            &session.id,
            &approval_actor(),
            &started.body.id,
            Some("operator stopped run".to_string()),
        )
        .expect("waiting run should cancel");

    let selected_run = service
        .get_run(
            &session.id,
            &GetRunQuery {
                run_id: started.body.id.clone(),
            },
        )
        .expect("run lookup")
        .expect("run should exist");
    let approvals = service
        .list_approvals(
            &session.id,
            &ListApprovalsQuery {
                run_id: Some(started.body.id.clone()),
                approval_id: None,
            },
        )
        .expect("approvals");
    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(cancelled.body.status, crate::RunStatus::Cancelled);
    assert_eq!(selected_run.summary.status, crate::RunStatus::Cancelled);
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());
    assert_eq!(selected_session.status, SessionStatus::Idle);
    assert!(matches!(
        &cancelled.deferred_records.iter().map(|event| &event.payload).collect::<Vec<_>>()[..],
        [
            DaemonEvent::Approval(crate::ApprovalEvent::Resolved { resolution }),
            DaemonEvent::Run(crate::RunEvent {
                run_id,
                status,
                detail,
                ..
            }),
        ] if resolution.run_id == started.body.id
            && resolution.decision == crate::ApprovalDecision::Rejected
            && resolution.commentary.as_deref() == Some("operator stopped run")
            && *run_id == started.body.id
            && *status == crate::RunStatus::Cancelled
            && detail == "operator stopped run"
    ));
}
