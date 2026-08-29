use ta_protocol::wire::{
    AgentStreamEvent, AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, DaemonEvent,
    ForkRunRequest, RunId, RunStatus, StreamEmission, WorkspaceFileAttachment,
    WorkspaceFileAttachmentRequest, WorkspaceFileKind,
};
use ta_store::{CommitRepository, CommitRunTransition, InMemoryStore, ProjectionRepository};

use super::test_support::*;
use super::*;
use crate::SessionId;

fn fork_request(
    session_id: &SessionId,
    parent_run_id: &RunId,
    parent_event_seq: u64,
) -> ForkRunRequest {
    ForkRunRequest {
        session_id: session_id.clone(),
        parent_run_id: parent_run_id.clone(),
        parent_event_seq,
        objective: Some("Forked objective".to_string()),
    }
}

fn last_event_seq(execution: &RunExecutionService<InMemoryStore>, run_id: &RunId) -> u64 {
    execution
        .store
        .lock()
        .expect("store should not poison")
        .run(run_id)
        .expect("run lookup should work")
        .expect("run should exist")
        .last_event_seq
        .expect("run should have durable event")
}

fn append_parent_events(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    parent_run_id: &RunId,
    events: Vec<DaemonEvent>,
) -> Vec<ta_store::EventRecord> {
    let mut store = execution.store.lock().expect("store should not poison");
    let existing = store
        .run(parent_run_id)
        .expect("run lookup should work")
        .expect("parent should exist");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: existing,
            user_turn: ta_store::UserTurnCommit::NoUserTurn,
            events,
            occurred_at_ms: current_time_ms(),
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
        })
        .expect("parent events should persist")
        .events
}

fn append_parent_user_turn(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    parent_run_id: &RunId,
) {
    let mut store = execution.store.lock().expect("store should not poison");
    let existing = store
        .run(parent_run_id)
        .expect("run lookup should work")
        .expect("parent should exist");
    let text = existing.objective.clone();
    let status = existing.status;
    let event = match status {
        RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval => {
            crate::RunEvent::active(parent_run_id.clone(), status, None, None, None)
                .expect("parent user-turn status should be active")
        }
        RunStatus::Completed
        | RunStatus::Failed
        | RunStatus::BudgetExceeded
        | RunStatus::Cancelled => crate::RunEvent::terminal(
            parent_run_id.clone(),
            status,
            crate::RunStatusReason::new("parent user turn")
                .expect("parent user-turn terminal reason should be valid"),
            None,
            None,
            None,
        )
        .expect("parent user-turn status should be terminal"),
    };
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: existing,
            user_turn: ta_store::UserTurnCommit::Append {
                text,
                attachments: Vec::new(),
            },
            events: vec![DaemonEvent::Run(event)],
            occurred_at_ms: current_time_ms(),
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
        })
        .expect("parent user turn should persist");
}

fn assistant_turn_events(run_id: &RunId, turn: &str, text: &str) -> Vec<DaemonEvent> {
    [
        AgentStreamFrame::AssistantTurnStarted,
        AgentStreamFrame::AssistantMessageDelta {
            delta: text.to_string(),
        },
        AgentStreamFrame::AssistantTurnCompleted,
    ]
    .into_iter()
    .map(|frame| agent_stream_event(run_id, turn, None, frame))
    .collect()
}

fn agent_stream_event(
    run_id: &RunId,
    turn: &str,
    item: Option<&str>,
    frame: AgentStreamFrame,
) -> DaemonEvent {
    DaemonEvent::AgentStream(AgentStreamEvent {
        run_id: run_id.clone(),
        emission: StreamEmission {
            turn_id: Some(AgentStreamTurnId::new(turn).expect("turn id")),
            item_id: item.map(|id| AgentStreamItemId::new(id).expect("item id")),
            fragment_sequence: None,
            frame,
        },
    })
}

fn set_parent_status(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    parent_run_id: &RunId,
    status: RunStatus,
) {
    let mut store = execution.store.lock().expect("store should not poison");
    let existing = store
        .run(parent_run_id)
        .expect("run lookup should work")
        .expect("parent should exist");
    let event = match status {
        RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval => {
            crate::RunEvent::active(parent_run_id.clone(), status, None, None, None)
                .expect("parent status should be active")
        }
        RunStatus::Completed
        | RunStatus::Failed
        | RunStatus::BudgetExceeded
        | RunStatus::Cancelled => crate::RunEvent::terminal(
            parent_run_id.clone(),
            status,
            crate::RunStatusReason::new(format!("parent {status:?}"))
                .expect("parent terminal status reason should be valid"),
            None,
            None,
            None,
        )
        .expect("parent status should be terminal"),
    };
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection { status, ..existing },
            user_turn: ta_store::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(event)],
            occurred_at_ms: current_time_ms(),
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
        })
        .expect("status update should persist");
}

#[test]
fn fork_run_replays_parent_state_through_turn_boundary() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let selection = validated_runtime_selection(&app, "runtime-openai-safe");
    let session = open_session(&app, "Active fork replay");
    let parent = execution
        .seed_running_run_for_tests(
            session.id.clone(),
            "Parent objective".to_string(),
            selection,
        )
        .expect("parent should seed");
    append_parent_user_turn(&execution, &session.id, &parent.run.id);
    let turn_one = append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        assistant_turn_events(&parent.run.id, "turn-1", "first answer"),
    );
    let turn_one_boundary = turn_one.last().expect("turn one event").sequence;
    append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        assistant_turn_events(&parent.run.id, "turn-2", "second answer"),
    );

    let fork = execution
        .fork_run(
            session.id.clone(),
            fork_request(&session.id, &parent.run.id, turn_one_boundary),
        )
        .expect("turn-boundary fork should pass");
    let initial_state = execution
        .fork_ancestor_history_for_run(&session.id, &fork.run.id)
        .expect("fork state should build")
        .expect("fork state should exist");

    assert_eq!(initial_state.messages.len(), 2);
    assert_eq!(initial_state.messages[0].content, "Parent objective");
    assert_eq!(initial_state.messages[1].content, "first answer");
}

#[test]
fn fork_run_replays_the_parent_attachment_manifest() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = open_session(&app, "Attachment fork replay");
    let mut command = start_run_command(&app, "Parent with context", "runtime-openai-safe");
    command.attachments = vec![WorkspaceFileAttachmentRequest {
        path: "docs/context.md".to_string(),
        expected_revision: "sha256:context".to_string(),
    }];
    let parent = execution
        .start_run_with_validated_attachments(
            session.id.clone(),
            command,
            vec![WorkspaceFileAttachment {
                path: "docs/context.md".to_string(),
                revision: "sha256:context".to_string(),
                kind: WorkspaceFileKind::Text,
                byte_len: 128,
            }],
        )
        .expect("validated parent should start");
    let events = append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        assistant_turn_events(&parent.run.id, "turn-1", "context accepted"),
    );
    let boundary = events.last().expect("turn boundary").sequence;

    let fork = execution
        .fork_run(
            session.id.clone(),
            fork_request(&session.id, &parent.run.id, boundary),
        )
        .expect("attachment parent should fork");
    let initial_state = execution
        .fork_ancestor_history_for_run(&session.id, &fork.run.id)
        .expect("fork state should build")
        .expect("fork state should exist");

    assert_eq!(initial_state.messages.len(), 2);
    assert!(
        initial_state.messages[0]
            .content
            .starts_with("Parent with context\n\n<taugentic_workspace_attachments>")
    );
    assert!(
        initial_state.messages[0]
            .content
            .contains("docs/context.md")
    );
    assert!(initial_state.messages[0].content.contains("sha256:context"));
    assert_eq!(initial_state.messages[1].content, "context accepted");
}

#[test]
fn fork_run_rejects_mid_turn_tool_boundary() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let selection = validated_runtime_selection(&app, "runtime-openai-safe");
    let session = open_session(&app, "Mid-turn fork replay");
    let parent = execution
        .seed_running_run_for_tests(
            session.id.clone(),
            "Parent objective".to_string(),
            selection,
        )
        .expect("parent should seed");
    let events = append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        vec![
            agent_stream_event(
                &parent.run.id,
                "turn-1",
                None,
                AgentStreamFrame::AssistantTurnStarted,
            ),
            agent_stream_event(
                &parent.run.id,
                "turn-1",
                None,
                AgentStreamFrame::AssistantTurnCompleted,
            ),
            agent_stream_event(
                &parent.run.id,
                "turn-1",
                Some("call-1"),
                AgentStreamFrame::ToolCallStarted {
                    tool_name: "shell".to_string(),
                    input: r#"{"cmd":"echo hi"}"#.to_string(),
                },
            ),
        ],
    );
    let mid_turn_seq = events.last().expect("tool start").sequence;

    let error = execution
        .fork_run(
            session.id.clone(),
            fork_request(&session.id, &parent.run.id, mid_turn_seq),
        )
        .expect_err("mid-turn fork point must fail closed");

    assert!(matches!(
        error,
        RunExecutionError::RunForkPointNotTurnBoundary(_)
    ));
}

#[test]
fn fork_run_replays_completed_parent_to_last_event() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let selection = validated_runtime_selection(&app, "runtime-openai-safe");
    let session = open_session(&app, "Completed fork replay");
    let parent = execution
        .seed_running_run_for_tests(
            session.id.clone(),
            "Parent objective".to_string(),
            selection,
        )
        .expect("parent should seed");
    append_parent_user_turn(&execution, &session.id, &parent.run.id);
    append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        assistant_turn_events(&parent.run.id, "turn-1", "first answer"),
    );
    append_parent_events(
        &execution,
        &session.id,
        &parent.run.id,
        assistant_turn_events(&parent.run.id, "turn-2", "second answer"),
    );
    set_parent_status(
        &execution,
        &session.id,
        &parent.run.id,
        RunStatus::Completed,
    );
    let last_event_seq = last_event_seq(&execution, &parent.run.id);

    let fork = execution
        .fork_run(
            session.id.clone(),
            fork_request(&session.id, &parent.run.id, last_event_seq),
        )
        .expect("completed parent should fork");
    let initial_state = execution
        .fork_ancestor_history_for_run(&session.id, &fork.run.id)
        .expect("fork state should build")
        .expect("fork state should exist");

    assert_eq!(initial_state.messages.len(), 3);
    assert_eq!(initial_state.messages[1].content, "first answer");
    assert_eq!(initial_state.messages[2].content, "second answer");
}
