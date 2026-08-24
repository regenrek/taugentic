use super::*;

#[test]
fn commit_run_transition_updates_session_and_allocates_monotonic_run_event_sequence() {
    let session_id = SessionId::new("session-a").expect("session id");
    let mut store = InMemoryStore::current();

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "memory-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session");
    store
        .append_event(EventRecord {
            sequence: 4,
            session_id: session_id.clone(),
            occurred_at_ms: 10,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Idle,
            }),
        })
        .expect("seed event");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-a").expect("run id"),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
                execution_context: crate::default_test_execution_context(),
                harness: RunHarnessKind::Unknown,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            },
            events: vec![DaemonEvent::Run(RunEvent {
                run_id: RunId::new("run-a").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect("run start event");
    let event = committed.events.first().expect("run start event");

    assert_eq!(event.sequence, 5);
    assert!(matches!(
        &event.payload,
        DaemonEvent::Run(RunEvent {
            run_id,
            status,
            detail,
            ..
        })
            if run_id.as_str() == "run-a"
                && *status == RunStatus::Running
                && detail == "Execution started"
    ));
    assert_eq!(
        some(store.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(
        some(store.run(&RunId::new("run-a").expect("run id"))).objective,
        "Ship app server hard cut"
    );
}

#[test]
fn commit_run_transition_persists_only_durable_agent_stream_frames() {
    let session_id = SessionId::new("session-lane").expect("session id");
    let run_id = RunId::new("run-lane").expect("run id");
    let mut store = InMemoryStore::current();

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "memory-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Lane".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "stream".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
                execution_context: crate::default_test_execution_context(),
                harness: RunHarnessKind::Unknown,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            },
            events: vec![
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallStarted {
                        tool_name: "shell".to_string(),
                        input: "{}".to_string(),
                    },
                ),
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallProgressed {
                        delta: "stdout".to_string(),
                    },
                ),
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallCompleted {
                        outcome: AgentToolCallOutcome::Completed,
                    },
                ),
            ],
            occurred_at_ms: 20,
        })
        .expect("run transition should commit");

    assert_eq!(
        committed
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        committed
            .persisted_events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(committed.commit.first_sequence, 1);
    assert_eq!(committed.commit.last_sequence, 3);
    assert_eq!(
        ok(store.events_for_session(&session_id))
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(
        ok(store.session_event_range(&SessionEventRangeQuery {
            session_id: session_id.clone(),
            after_sequence: None,
            up_to_sequence: None,
            kinds: vec![ta_protocol::wire::DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(
        ok(store.session_event_page(&SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![ta_protocol::wire::DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );
}
