use super::*;

#[test]
fn logged_out_explicit_selection_fails_before_store_or_scheduler_mutation() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Current auth validation");
    let selection = crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");
    let auth_profile_id = selection
        .auth_profile_id
        .as_ref()
        .expect("test selection should include an auth profile");
    let mut profile = service
        .store
        .lock()
        .expect("app store should not be poisoned")
        .auth_profile(auth_profile_id)
        .expect("auth profile lookup should succeed")
        .expect("auth profile should exist");
    profile.profile.connection_state = crate::AuthProfileConnectionState::LoggedOut;
    service
        .store
        .lock()
        .expect("app store should not be poisoned")
        .save_auth_profile(profile.clone())
        .expect("logged-out auth profile should persist");

    let error = service
        .start_run(
            &session.id,
            &StartRunCommand::new("Reject stale auth", selection.clone()),
        )
        .expect_err("logged-out auth must fail before scheduling");

    assert!(error.to_string().contains("not connected"));
    assert!(
        service
            .list_runs(&session.id)
            .expect("runs should list")
            .is_empty()
    );

    profile.profile.connection_state = crate::AuthProfileConnectionState::Connected;
    service
        .store
        .lock()
        .expect("app store should not be poisoned")
        .save_auth_profile(profile)
        .expect("connected auth profile should persist");
    let started = service
        .start_run(
            &session.id,
            &StartRunCommand::new("Use current auth", selection),
        )
        .expect("same explicit command should validate after reconnect");

    assert_ne!(started.body.status, RunStatus::Queued);
}

#[test]
fn independent_explicit_selections_freeze_routes_without_prior_selection_call() {
    let service = AppService::bootstrap().expect("app service should boot");
    let first_session = open_test_session(&service, "First explicit route");
    let second_session = open_test_session(&service, "Second explicit route");
    let first_selection =
        crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");
    let second_selection =
        crate::orchestration::test_runtime_selection(&service, "runtime-codex-safe");

    let first = service
        .start_run(
            &first_session.id,
            &StartRunCommand::new("First route", first_selection.clone()),
        )
        .expect("first explicit selection should start");
    let second = service
        .start_run(
            &second_session.id,
            &StartRunCommand::new("Second route", second_selection.clone()),
        )
        .expect("second explicit selection should start");

    let store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    let first_route = store
        .run(&first.body.id)
        .expect("first run lookup should succeed")
        .expect("first run should exist")
        .source
        .route()
        .clone();
    let second_route = store
        .run(&second.body.id)
        .expect("second run lookup should succeed")
        .expect("second run should exist")
        .source
        .route()
        .clone();

    assert_eq!(
        first_route.runtime_profile_id,
        first_selection.runtime_profile_id
    );
    assert_eq!(first_route.model_id, first_selection.model_id);
    assert_eq!(first_route.auth_profile_id, first_selection.auth_profile_id);
    assert_eq!(
        second_route.runtime_profile_id,
        second_selection.runtime_profile_id
    );
    assert_eq!(second_route.model_id, second_selection.model_id);
    assert_eq!(
        second_route.auth_profile_id,
        second_selection.auth_profile_id
    );
    assert_ne!(first_route.provider_id, second_route.provider_id);
}

#[test]
fn list_runs_filters_by_session_id() {
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
    let selection = crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");
    service
        .start_run(
            &session_a.id,
            &StartRunCommand::new("one", selection.clone()),
        )
        .expect("run should start");
    service
        .start_run(&session_b.id, &StartRunCommand::new("two", selection))
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
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let selection = crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");

    let started = service
        .start_run(
            &session.id,
            &StartRunCommand::new("Ship app server hard cut", selection),
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
            DaemonEvent::Run(crate::RunEvent::Status(event)),
            DaemonEvent::Approval(crate::ApprovalEvent::Requested { request }),
        ] if event.run_id() == &started.body.id
            && event.status() == crate::RunStatus::WaitingForApproval
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
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let selection = crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");
    let other_session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Other".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let selected_run = service
        .start_run(
            &selected_session.id,
            &StartRunCommand::new("selected", selection),
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
