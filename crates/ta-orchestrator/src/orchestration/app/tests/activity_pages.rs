use super::*;

#[test]
fn activity_page_filters_by_session_kind_and_cursor() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session_a = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "A".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let session_b = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "B".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    service
        .start_run(&session_a.id, &start_run_command(&service, "run-a"))
        .expect("run should start");
    service
        .start_run(&session_b.id, &start_run_command(&service, "run-b"))
        .expect("run should start");

    let page = service
        .activity_page(
            &session_a.id,
            &ActivityPageQuery {
                limit: 2,
                before: None,
                kinds: vec![],
            },
        )
        .expect("activity page");

    assert_eq!(
        page.latest_activity_cursor,
        Some(ActivityCursor { sequence: 4 })
    );
    assert_eq!(page.next_before, Some(ActivityCursor { sequence: 3 }));
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.cursor.sequence)
            .collect::<Vec<_>>(),
        vec![4, 3]
    );

    let older_run_only = service
        .activity_page(
            &session_a.id,
            &ActivityPageQuery {
                limit: 10,
                before: Some(ActivityCursor { sequence: 3 }),
                kinds: vec![DaemonEventKind::Run],
            },
        )
        .expect("filtered page");

    assert_eq!(
        older_run_only.latest_activity_cursor,
        Some(ActivityCursor { sequence: 3 })
    );
    assert_eq!(older_run_only.next_before, None);
    assert_eq!(
        older_run_only
            .items
            .iter()
            .map(|item| item.cursor.sequence)
            .collect::<Vec<_>>(),
        Vec::<u64>::new()
    );
}

#[test]
fn activity_page_rejects_zero_limit() {
    let service = AppService::bootstrap().expect("app service should boot");
    let error = service
        .activity_page(
            &SessionId::new("session-a").expect("session id"),
            &ActivityPageQuery {
                limit: 0,
                before: None,
                kinds: vec![],
            },
        )
        .expect_err("zero limit must fail");

    assert!(matches!(error, AppServiceError::InvalidActivityPageLimit));
}

#[test]
fn activity_page_agent_stream_surface_excludes_transient_lane_frames() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "A".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = ensure_running_run(&service, &session.id, "run-a");

    commit_agent_stream_events(
        &service,
        &session.id,
        &started.body.id,
        500,
        vec![
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::ToolCallStarted {
                    tool_name: "shell".to_string(),
                    input: "{}".to_string(),
                },
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::AssistantTurnStarted,
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                Some(1),
                AgentStreamFrame::AssistantMessageDelta {
                    delta: "partial".to_string(),
                },
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::ToolCallCompleted {
                    outcome: crate::AgentToolCallOutcome::Completed,
                },
            )),
        ],
    );

    let page = service
        .activity_page(
            &session.id,
            &ActivityPageQuery {
                limit: 10,
                before: None,
                kinds: vec![DaemonEventKind::AgentStream],
            },
        )
        .expect("activity page");

    assert_eq!(
        page.items
            .iter()
            .map(|item| item.cursor.sequence)
            .collect::<Vec<_>>(),
        vec![6, 4, 3]
    );
    assert_eq!(
        page.latest_activity_cursor,
        Some(ActivityCursor { sequence: 6 })
    );
    assert!(page.items.iter().all(|item| {
        matches!(
            item.event,
            PublicDaemonEvent::AgentStream(AgentStreamEvent {
                emission: StreamEmission {
                    frame: AgentStreamFrame::AssistantTurnStarted
                        | AgentStreamFrame::ToolCallStarted { .. }
                        | AgentStreamFrame::ToolCallCompleted { .. },
                    ..
                },
                ..
            })
        )
    }));
}

#[test]
fn agent_turns_page_materializes_committed_rows_from_stream_frames() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Agent turns".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = ensure_running_run(&service, &session.id, "run-a");

    commit_agent_stream_events_with_user_turn(
        &service,
        &session.id,
        &started.body.id,
        500,
        ta_store::UserTurnCommit::Append {
            text: "run-a".to_string(),
            attachments: Vec::new(),
        },
        vec![
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::AssistantTurnStarted,
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                Some(1),
                AgentStreamFrame::AssistantMessageDelta {
                    delta: "hello ".to_string(),
                },
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                Some(2),
                AgentStreamFrame::AssistantMessageDelta {
                    delta: "world".to_string(),
                },
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::AssistantTurnCompleted,
            )),
        ],
    );

    let page = service
        .agent_turns_page(
            &session.id,
            &AgentTurnsPageQuery {
                limit: 10,
                before: None,
            },
        )
        .expect("agent turns page");

    assert_eq!(
        page.latest_cursor,
        Some(crate::DaemonEventCursor {
            daemon_instance_id: service.daemon_instance_id.clone(),
            session_id: session.id.clone(),
            sequence: 6,
        })
    );
    assert_eq!(page.next_before, None);
    assert_eq!(page.items.len(), 2);
    assert!(matches!(
        &page.items[0],
        crate::AgentTurnRow::Assistant(row)
            if row.text == "hello world" && row.started_at_ms == 500 && row.completed_at_ms == 500
    ));
    assert!(matches!(
        &page.items[1],
        crate::AgentTurnRow::User(row)
            if row.text == "run-a"
    ));
}

#[test]
fn latest_event_cursor_for_session_ignores_live_only_agent_stream_frames() {
    let service = AppService::bootstrap().expect("app service should boot");
    let opened = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Cursor".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = service
        .start_run(&opened.id, &start_run_command(&service, "stream"))
        .expect("run should start");
    commit_agent_stream_events(
        &service,
        &opened.id,
        &started.body.id,
        500,
        vec![
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::AssistantTurnStarted,
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                None,
                AgentStreamFrame::ToolCallStarted {
                    tool_name: "shell".to_string(),
                    input: "{}".to_string(),
                },
            )),
        ],
    );

    let latest_before_transient = service
        .latest_event_cursor_for_session(&opened.id)
        .expect("latest event cursor should load before transient commit")
        .map(|cursor| cursor.sequence);

    commit_agent_stream_events(
        &service,
        &opened.id,
        &started.body.id,
        500,
        vec![
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                Some(1),
                AgentStreamFrame::AssistantMessageDelta {
                    delta: "partial".to_string(),
                },
            )),
            DaemonEvent::AgentStream(agent_stream_event(
                started.body.id.clone(),
                Some(2),
                AgentStreamFrame::ToolCallProgressed {
                    delta: "stdout".to_string(),
                },
            )),
        ],
    );

    assert_eq!(
        service
            .latest_event_cursor_for_session(&opened.id)
            .expect("latest event cursor should load")
            .map(|cursor| cursor.sequence),
        latest_before_transient
    );
}
