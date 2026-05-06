use super::*;

#[test]
fn get_session_returns_projected_summary() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build foundation".to_string(),
            },
        )
        .expect("session should open");

    let session = service.get_session(&session.id).expect("session");

    assert_eq!(
        session.expect("session should exist").title,
        "Build foundation"
    );
}

#[test]
fn list_sessions_filters_by_owner_principal_id() {
    let service = AppService::bootstrap().expect("app service should boot");
    let selected = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
            },
        )
        .expect("session should open");
    service
        .open_session(
            TEST_CLIENT_NAME,
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Other".to_string(),
            },
        )
        .expect("session should open");

    let sessions = service
        .list_sessions(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &ListSessionsQuery {},
        )
        .expect("sessions should list");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, selected.id);
    assert_eq!(sessions[0].title, "Selected");
}

#[test]
fn resolve_or_issue_session_principal_reuses_known_client_credential() {
    let service = AppService::bootstrap().expect("app service should boot");

    let issued = service
        .resolve_or_issue_session_principal(TEST_CLIENT_NAME, None)
        .expect("principal should issue");
    let reused = service
        .resolve_or_issue_session_principal(TEST_CLIENT_NAME, Some(&issued.client_credential))
        .expect("principal should resolve");

    assert_eq!(reused.principal_id, issued.principal_id);
    assert_eq!(reused.client_credential, issued.client_credential);
    assert_eq!(reused.client_name, TEST_CLIENT_NAME);
}

#[test]
fn resolve_or_issue_session_principal_reuses_canonical_client_name_for_known_credential() {
    let service = AppService::bootstrap().expect("app service should boot");

    let issued = service
        .resolve_or_issue_session_principal(TEST_CLIENT_NAME, None)
        .expect("principal should issue");
    let reused = service
        .resolve_or_issue_session_principal("spoofed-client", Some(&issued.client_credential))
        .expect("principal should resolve");

    assert_eq!(reused.principal_id, issued.principal_id);
    assert_eq!(reused.client_credential, issued.client_credential);
    assert_eq!(reused.client_name, TEST_CLIENT_NAME);
}

#[test]
fn resolve_or_issue_session_principal_rotates_unknown_client_credential() {
    let service = AppService::bootstrap().expect("app service should boot");

    let resolved = service
        .resolve_or_issue_session_principal(TEST_CLIENT_NAME, Some(TEST_CLIENT_CREDENTIAL))
        .expect("principal should issue");

    assert_ne!(resolved.client_credential, TEST_CLIENT_CREDENTIAL);
    assert!(resolved.client_credential.len() >= 32);
    assert!(resolved.principal_id.starts_with("principal-"));
}

#[test]
fn attach_session_rejects_foreign_owner_projection() {
    let service = AppService::bootstrap().expect("app service should boot");
    let opened = service
        .open_session(
            TEST_CLIENT_NAME,
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
            },
        )
        .expect("session should open");

    let error = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &opened.session_authority,
        )
        .expect_err("foreign session should fail");

    assert!(matches!(error, AppServiceError::SessionNotFound(_)));
}

#[test]
fn attach_session_requires_existing_projection() {
    let service = AppService::bootstrap().expect("app service should boot");
    let error = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionId::new("session-missing").expect("session id"),
            &SessionAuthority::new("session-authority-1session-authority-1")
                .expect("session authority"),
        )
        .expect_err("missing session should fail");

    assert!(matches!(error, AppServiceError::SessionNotFound(_)));
}

#[test]
fn attach_session_rejects_wrong_session_authority() {
    let service = AppService::bootstrap().expect("app service should boot");
    let opened = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
            },
        )
        .expect("session should open");

    let error = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &SessionAuthority::new("wrong-session-authoritywrong-session-authority")
                .expect("session authority"),
        )
        .expect_err("wrong authority should fail");

    assert!(matches!(
        error,
        AppServiceError::SessionAuthorityRejected(_)
    ));
}

#[test]
fn attach_session_consumes_recovery_authority_once() {
    let service = AppService::bootstrap().expect("app service should boot");
    let opened = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
            },
        )
        .expect("session should open");

    let attached = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &opened.session_authority,
        )
        .expect("attach should succeed");

    assert_eq!(attached.id, opened.id);
    assert_ne!(attached.session_authority, opened.session_authority);

    let recovered = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &opened.session_authority,
        )
        .expect("recovery authority should recover once");
    assert_ne!(recovered.session_authority, attached.session_authority);

    let stale_attached_error = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &attached.session_authority,
        )
        .expect_err("consumed recovery flow should not leave attached authority valid");
    assert!(matches!(
        stale_attached_error,
        AppServiceError::SessionAuthorityRejected(_)
    ));

    let rotated = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &recovered.session_authority,
        )
        .expect("rotated authority should attach");
    assert_ne!(rotated.session_authority, recovered.session_authority);

    let stale_error = service
        .attach_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &opened.id,
            &opened.session_authority,
        )
        .expect_err("oldest authority should fail");
    assert!(matches!(
        stale_error,
        AppServiceError::SessionAuthorityRejected(_)
    ));
}

#[test]
fn open_session_creates_idle_projection() {
    let service = AppService::bootstrap().expect("app service should boot");

    let opened = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Build daemon app server".to_string(),
            },
        )
        .expect("session should open");

    assert!(opened.id.as_str().starts_with("session-"));
    assert_eq!(opened.title, "Build daemon app server");
    assert_eq!(opened.status, SessionStatus::Idle);

    let stored = service
        .get_session(&opened.id)
        .expect("stored session query should work");
    assert_eq!(stored, Some(opened.session.clone()));
    assert_eq!(
        service
            .latest_event_cursor_for_session(&opened.id)
            .expect("latest event cursor should work")
            .map(|cursor| (
                cursor.daemon_instance_id,
                cursor.session_id,
                cursor.sequence
            )),
        Some((service.daemon_instance_id.clone(), opened.id.clone(), 1))
    );
    assert!(opened.session_authority.as_str().len() >= 32);
}

#[test]
fn completing_all_runs_marks_session_completed() {
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

    commit_session_status_test_run(
        &service,
        &session.id,
        "run-completed-a",
        RunStatus::Completed,
    );
    commit_session_status_test_run(
        &service,
        &session.id,
        "run-completed-b",
        RunStatus::Completed,
    );

    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(selected_session.status, SessionStatus::Completed);
}

#[test]
fn mixed_terminal_states_resolves_to_failed_when_any_failed() {
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

    commit_session_status_test_run(&service, &session.id, "run-completed", RunStatus::Completed);
    commit_session_status_test_run(&service, &session.id, "run-failed", RunStatus::Failed);

    let selected_session = service
        .get_session(&session.id)
        .expect("session lookup")
        .expect("session should exist");

    assert_eq!(selected_session.status, SessionStatus::Failed);
}

fn commit_session_status_test_run(
    service: &AppService,
    session_id: &SessionId,
    run_id: &str,
    status: RunStatus,
) {
    let run = native_run_projection(run_id, session_id, status, 100);
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: run.clone(),
            events: vec![DaemonEvent::Run(crate::RunEvent {
                run_id: run.id,
                status,
                detail: format!("Session status test transition to {status:?}"),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 100,
        })
        .expect("run transition should commit");
}

#[test]
fn open_session_rejects_blank_title() {
    let service = AppService::bootstrap().expect("app service should boot");

    let error = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "   ".to_string(),
            },
        )
        .expect_err("blank title should fail");

    assert!(matches!(error, AppServiceError::EmptySessionTitle));
}
