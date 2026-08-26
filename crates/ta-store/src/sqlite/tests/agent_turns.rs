use super::*;

#[test]
fn session_agent_turns_page_persists_committed_rows_across_reopen() {
    let path = test_db_path("agent-turns-page");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-agent-turns").expect("session id");
    let run_id = RunId::new("run-agent-turns").expect("run id");

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Agent turns".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session");

    let run = RunProjection {
        id: run_id.clone(),
        session_id: session_id.clone(),
        runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
            .expect("runtime profile id"),
        objective: "stream".to_string(),
        status: RunStatus::Running,
        source: crate::default_test_run_source(),
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
    };

    for (occurred_at_ms, frame) in [
        (10, AgentStreamFrame::AssistantTurnStarted),
        (
            11,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "hello ".to_string(),
            },
        ),
        (
            12,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "world".to_string(),
            },
        ),
        (13, AgentStreamFrame::AssistantTurnCompleted),
        (
            20,
            AgentStreamFrame::ToolCallStarted {
                tool_name: "shell".to_string(),
                input: r#"{"cmd":"echo hi"}"#.to_string(),
            },
        ),
        (
            21,
            AgentStreamFrame::ToolCallProgressed {
                delta: "echo hi".to_string(),
            },
        ),
        (
            22,
            AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed,
            },
        ),
    ] {
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: run.clone(),
                events: vec![agent_stream_event(&run_id, frame)],
                occurred_at_ms,
            })
            .expect("agent frame should commit");
    }

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    let page = ok(
        reopened.session_agent_turns_page(&crate::SessionAgentTurnsPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
        }),
    );

    assert_eq!(page.latest_activity_sequence, Some(7));
    assert_eq!(page.rows.len(), 2);
    assert!(matches!(
        &page.rows[0],
        ta_protocol::wire::AgentTurnRow::ToolCall(row)
            if row.tool_name == "shell" && row.output == "echo hi"
    ));
    assert!(matches!(
        &page.rows[1],
        ta_protocol::wire::AgentTurnRow::Assistant(row)
            if row.text == "hello world" && row.started_at_ms == 10 && row.completed_at_ms == 13
    ));

    let _ = std::fs::remove_file(path);
}
