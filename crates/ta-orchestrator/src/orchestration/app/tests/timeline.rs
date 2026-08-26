use super::*;

#[test]
fn run_timeline_projects_lineage_and_typed_events_from_event_log() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Timeline projection".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let parent = native_run_projection("run-parent", &session.id, RunStatus::Running, 100);
    let child = child_run_projection("run-child", &session.id, &parent.id, 200);
    let sibling = child_run_projection("run-sibling", &session.id, &parent.id, 250);
    seed_run_projection(&service, parent.clone());
    seed_run_projection(&service, child.clone());
    seed_run_projection(&service, sibling.clone());
    append_timeline_events(&service, &session.id, &parent.id, &child.id, &sibling.id);

    let timeline = service
        .run_timeline(
            &session.id,
            &GetRunTimelineQuery {
                session_id: session.id.clone(),
                root_run_id: parent.id.clone(),
                after_seq: Some(101),
                limit: Some(10),
            },
        )
        .expect("timeline should project");

    assert_eq!(timeline.root_run_id, parent.id);
    assert_eq!(timeline.latest_event_seq, Some(107));
    assert_eq!(
        timeline
            .runs
            .iter()
            .map(|run| (run.run_id.as_str(), run.depth))
            .collect::<Vec<_>>(),
        vec![("run-parent", 0), ("run-child", 1), ("run-sibling", 1)]
    );
    assert_eq!(
        timeline
            .events
            .iter()
            .map(|event| (event.seq, event.run_id.as_str(), event.kind))
            .collect::<Vec<_>>(),
        vec![
            (102, "run-child", RunTimelineEventKind::RunStatus),
            (103, "run-child", RunTimelineEventKind::ApprovalRequested),
            (104, "run-sibling", RunTimelineEventKind::ClaimConflict),
            (105, "run-child", RunTimelineEventKind::TokenUsage),
            (106, "run-child", RunTimelineEventKind::ToolCall),
            (107, "run-child", RunTimelineEventKind::BudgetExceeded),
        ]
    );
}

#[test]
fn run_timeline_rejects_session_mismatch() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: "Timeline session".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let other_session_id = SessionId::new("session-other").expect("session id");

    let error = service
        .run_timeline(
            &session.id,
            &GetRunTimelineQuery {
                session_id: other_session_id,
                root_run_id: RunId::new("run-parent").expect("run id"),
                after_seq: None,
                limit: Some(10),
            },
        )
        .expect_err("mismatched request session should fail");

    assert!(matches!(error, AppServiceError::RunSessionMismatch(_)));
}

fn child_run_projection(
    run_id: &str,
    session_id: &SessionId,
    parent_run_id: &RunId,
    started_at_ms: u64,
) -> RunProjection {
    RunProjection {
        id: RunId::new(run_id).expect("run id"),
        session_id: session_id.clone(),
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        objective: format!("Objective {run_id}"),
        status: RunStatus::Running,
        harness: RunHarnessKind::Native,
        source: RunSource::NativeSubagent {
            route: ta_store::default_test_run_source().route().clone(),
            parent_run_id: parent_run_id.clone(),
            parent_turn_id: AgentStreamTurnId::new("turn-parent").expect("turn id"),
            output_contract: Some(OutputContractKind::Patch),
            model_id: None,
            recipe_id: Some("patch-agent".to_string()),
            workspace_scope: Default::default(),
            cleanup_policy: Default::default(),
            planned_write_files: vec!["apps/desktop/package.json".to_string()],
        },
        execution_context: ta_store::default_test_execution_context(),
        result: None,
        contract_violation: None,
        started_at_ms: Some(started_at_ms),
        ended_at_ms: None,
        last_event_seq: Some(started_at_ms / 10),
        workspace_info: None,
        claimed_files: vec!["apps/desktop/package.json".to_string()],
        conflict_summary: None,
    }
}

fn append_timeline_events(
    service: &AppService,
    session_id: &SessionId,
    parent_run_id: &RunId,
    child_run_id: &RunId,
    sibling_run_id: &RunId,
) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    let events = vec![
        run_event(101, session_id, parent_run_id, "parent started"),
        run_event(102, session_id, child_run_id, "child running"),
        approval_event(103, session_id, child_run_id),
        conflict_event(104, session_id, sibling_run_id, child_run_id),
        token_event(105, session_id, child_run_id),
        tool_event(106, session_id, child_run_id),
        budget_event(107, session_id, child_run_id, parent_run_id),
    ];
    for event in events {
        store
            .append_event(event)
            .expect("timeline event should seed");
    }
}

fn run_event(sequence: u64, session_id: &SessionId, run_id: &RunId, detail: &str) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Run(crate::RunEvent {
            run_id: run_id.clone(),
            status: RunStatus::Running,
            detail: detail.to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        }),
    }
}

fn approval_event(sequence: u64, session_id: &SessionId, run_id: &RunId) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Approval(crate::ApprovalEvent::Requested {
            request: ApprovalRequest::new(
                ApprovalId::new("approval-child").expect("approval id"),
                run_id.clone(),
                ApprovalScope::ProcessExec,
                sequence * 10,
                sequence * 10 + 1_000,
                ApprovalTarget::CapsuleDispatch {
                    child_run_id: Some(run_id.clone()),
                    workspace_scope: None,
                },
                "dispatch child",
            )
            .expect("approval request"),
        }),
    }
}

fn conflict_event(
    sequence: u64,
    session_id: &SessionId,
    requesting_run_id: &RunId,
    holding_run_id: &RunId,
) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Conflict(crate::ConflictEvent::Warning {
            run_id: requesting_run_id.clone(),
            warning: ConflictWarning {
                requesting_capsule: requesting_run_id.clone(),
                severity: ConflictSeverity::Warning,
                conflicts: vec![FileClaimConflict {
                    file: "apps/desktop/package.json".to_string(),
                    holding_capsule: holding_run_id.clone(),
                    holding_kind: FileClaimKind::Write,
                }],
            },
        }),
    }
}

fn token_event(sequence: u64, session_id: &SessionId, run_id: &RunId) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::TokenUsageRecorded(crate::TokenUsageRecordedEvent {
            run_id: run_id.clone(),
            capsule_id: None,
            prompt_tokens: 100,
            completion_tokens: 23,
            cached_tokens: Some(10),
            reasoning_tokens: Some(3),
            model: "gpt-test".to_string(),
            provider: "openai".to_string(),
            recorded_at_ms: sequence * 10,
        }),
    }
}

fn tool_event(sequence: u64, session_id: &SessionId, run_id: &RunId) -> EventRecord {
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::AgentStream(agent_stream_event(
            run_id.clone(),
            None,
            AgentStreamFrame::ToolCallStarted {
                tool_name: "shell".to_string(),
                input: "{}".to_string(),
            },
        )),
    }
}

fn budget_event(
    sequence: u64,
    session_id: &SessionId,
    run_id: &RunId,
    parent_run_id: &RunId,
) -> EventRecord {
    let breach = BudgetBreach {
        scope: BudgetScope::ParentAggregate,
        metric: BudgetMetric::Tokens,
        limit: 100,
        actual: 123,
    };
    EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Budget(BudgetEvent::Exceeded {
            event: BudgetExceededEvent {
                run_id: run_id.clone(),
                parent_run_id: Some(parent_run_id.clone()),
                breach: breach.clone(),
                snapshot: BudgetSnapshot {
                    run_id: run_id.clone(),
                    parent_run_id: Some(parent_run_id.clone()),
                    scope: breach.scope,
                    total_tokens: breach.actual,
                    wall_clock_ms: 0,
                    tool_calls: 0,
                },
            },
        }),
    }
}
