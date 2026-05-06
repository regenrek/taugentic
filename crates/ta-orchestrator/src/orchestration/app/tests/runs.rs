use super::*;

#[test]
fn list_runs_filters_by_session_id() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session_a = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session A".to_string(),
            },
        )
        .expect("session should open");
    let session_b = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Session B".to_string(),
            },
        )
        .expect("session should open");
    service
        .start_run(
            &session_a.id,
            &StartRunCommand {
                objective: "one".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    service
        .start_run(
            &session_b.id,
            &StartRunCommand {
                objective: "two".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let runs = service.list_runs(&session_a.id).expect("runs");

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].objective, "one");
}

#[test]
fn start_run_projects_waiting_run_session_status_and_activity() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");

    let started = service
        .start_run(
            &session.id,
            &StartRunCommand {
                objective: "Ship app server hard cut".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");
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

    assert_eq!(started.body.status, crate::RunStatus::WaitingForApproval);
    assert_eq!(selected_session.status, SessionStatus::Running);
    assert_eq!(
        selected_run.summary.status,
        crate::RunStatus::WaitingForApproval
    );
    assert_eq!(approvals.items.len(), 1);
    assert!(approvals.latest_cursor.is_some());
    assert_eq!(activity.items.len(), 2);
    assert!(matches!(
        &started.deferred_records.iter().map(|event| &event.payload).collect::<Vec<_>>()[..],
        [
            DaemonEvent::Run(crate::RunEvent {
                run_id,
                status,
                detail,
                ..
            }),
            DaemonEvent::Approval(crate::ApprovalEvent::Requested { request }),
        ] if *run_id == started.body.id
            && *status == crate::RunStatus::WaitingForApproval
            && detail == "Waiting for approval"
            && request.run_id == started.body.id
    ));
}

#[test]
fn get_run_returns_summary_only_for_selected_session() {
    let service = AppService::bootstrap().expect("app service should boot");
    let selected_session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
            },
        )
        .expect("session should open");
    let other_session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Other".to_string(),
            },
        )
        .expect("session should open");
    let selected_run = service
        .start_run(
            &selected_session.id,
            &StartRunCommand {
                objective: "selected".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    let selected = service
        .get_run(
            &selected_session.id,
            &GetRunQuery {
                run_id: selected_run.body.id.clone(),
            },
        )
        .expect("run lookup should work");
    let other = service
        .get_run(
            &other_session.id,
            &GetRunQuery {
                run_id: selected_run.body.id,
            },
        )
        .expect("run lookup should work");

    assert_eq!(
        selected.expect("run should exist").summary.objective,
        "selected"
    );
    assert_eq!(other, None);
}
