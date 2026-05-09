use super::*;

#[test]
fn session_overview_projects_lane_activity_and_approval_attention() {
    let service = AppService::bootstrap().expect("app service should boot");
    let selected = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    service
        .open_session(
            TEST_CLIENT_NAME,
            OTHER_TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Other".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(
            &selected.id,
            &StartRunCommand {
                objective: "needs approval".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    assert_eq!(started.body.status, RunStatus::WaitingForApproval);

    let snapshot = service
        .session_overview(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionOverviewQuery {
                recent_activity_limit: 3,
            },
        )
        .expect("session overview should project");

    assert_eq!(snapshot.sessions.len(), 1);
    let selected_snapshot = &snapshot.sessions[0];
    assert_eq!(selected_snapshot.session.id, selected.id);
    assert_eq!(
        selected_snapshot.latest_run.as_ref().map(|run| run.status),
        Some(RunStatus::WaitingForApproval)
    );
    assert_eq!(
        selected_snapshot.lane_status,
        SessionOverviewLaneStatus::WaitingForApproval
    );
    assert!(selected_snapshot.is_active);
    assert_eq!(
        selected_snapshot.approval_attention,
        ApprovalAttentionState::Pending
    );
    assert_eq!(selected_snapshot.pending_approval_count, 1);
    assert_eq!(selected_snapshot.recent_activity.len(), 3);
    assert_eq!(
        selected_snapshot.last_event_preview.as_deref(),
        Some("Approval requested: execute run executes a process")
    );
    assert!(selected_snapshot.last_activity_at_ms.is_some());
}

#[test]
fn session_overview_uses_latest_run_not_latest_non_run_activity() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let running = ensure_running_run(&service, &session.id, "artifact producer");
    let queued = service
        .start_run(
            &session.id,
            &StartRunCommand {
                objective: "latest waiting".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    service
        .record_artifact(ArtifactRecord {
            id: ArtifactId::new("artifact-latest").expect("artifact id"),
            session_id: session.id.clone(),
            run_id: running.body.id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-a/patch.diff".to_string(),
        })
        .expect("artifact should record");

    let snapshot = service
        .session_overview(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionOverviewQuery {
                recent_activity_limit: 1,
            },
        )
        .expect("session overview should project");

    let selected_snapshot = &snapshot.sessions[0];
    let expected_preview = format!("Artifact patch for run {}", running.body.id.as_str());
    assert_eq!(selected_snapshot.recent_activity.len(), 1);
    assert_eq!(
        selected_snapshot
            .latest_run
            .as_ref()
            .map(|run| run.id.clone()),
        Some(queued.body.id.clone())
    );
    assert_eq!(
        selected_snapshot.latest_run.as_ref().map(|run| run.status),
        Some(queued.body.status)
    );
    assert_eq!(
        selected_snapshot.lane_status,
        SessionOverviewLaneStatus::Active
    );
    assert_eq!(
        selected_snapshot.last_event_preview.as_deref(),
        Some(expected_preview.as_str())
    );
}

#[test]
fn session_overview_caps_recent_activity_limit_server_side() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let mut latest_run = None;
    for index in 0..20 {
        latest_run = Some(
            service
                .start_run(
                    &session.id,
                    &StartRunCommand {
                        objective: format!("run-{index}"),
                        ..StartRunCommand::default()
                    },
                )
                .expect("run should start")
                .body
                .id,
        );
    }

    let snapshot = service
        .session_overview(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionOverviewQuery {
                recent_activity_limit: u32::MAX,
            },
        )
        .expect("session overview should project");

    let selected_snapshot = &snapshot.sessions[0];
    assert_eq!(
        selected_snapshot.recent_activity.len(),
        MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT as usize
    );
    assert_eq!(
        selected_snapshot
            .latest_run
            .as_ref()
            .map(|run| run.id.clone()),
        latest_run
    );
}

#[test]
fn session_overview_preserves_preview_metadata_when_recent_activity_is_zero() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    service
        .start_run(
            &session.id,
            &StartRunCommand {
                objective: "needs approval".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");

    let snapshot = service
        .session_overview(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionOverviewQuery {
                recent_activity_limit: 0,
            },
        )
        .expect("session overview should project");

    let selected_snapshot = &snapshot.sessions[0];
    assert_eq!(selected_snapshot.recent_activity.len(), 0);
    assert!(selected_snapshot.last_activity_at_ms.is_some());
    assert_eq!(
        selected_snapshot.last_event_preview.as_deref(),
        Some("Approval requested: execute run executes a process")
    );
}

#[test]
fn session_overview_excludes_agent_stream_events_from_recent_activity() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Selected".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(
            &session.id,
            &StartRunCommand {
                objective: "agent stream excluded".to_string(),
                ..StartRunCommand::default()
            },
        )
        .expect("run should start");
    append_agent_stream_tool_started_event(
        &service,
        &session.id,
        &started.body.id,
        10_000,
        999_999,
    );

    let snapshot = service
        .session_overview(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &SessionOverviewQuery {
                recent_activity_limit: 1,
            },
        )
        .expect("session overview should project");

    let selected_snapshot = &snapshot.sessions[0];
    assert_eq!(selected_snapshot.recent_activity.len(), 1);
    assert!(!matches!(
        &selected_snapshot.recent_activity[0].event,
        PublicDaemonEvent::AgentStream(_)
    ));
    assert_ne!(selected_snapshot.last_activity_at_ms, Some(999_999));
    assert_ne!(
        selected_snapshot.last_event_preview.as_deref(),
        Some("Tool call started: shell")
    );
}
