use super::*;
use crate::{ThreadWorkspaceEvent, ThreadWorkspaceRepository};
use ta_protocol::wire::{ActivityCursor, AgentStreamFrame, ThreadWorkspacePin};

fn session(id: &str) -> SessionProjection {
    SessionProjection {
        id: SessionId::new(id).expect("session id"),
        owner_client_name: "memory-tests".to_string(),
        owner_principal_id: "principal-test-owner".to_string(),
        current_session_authority_hash: "session-authority-hash".to_string(),
        current_session_authority_generation: 0,
        recovery_session_authority_hash: None,
        recovery_session_authority_generation: None,
        title: "Thread workspace".to_string(),
        status: SessionStatus::Running,
        workspace_id: crate::default_test_workspace_id(),
        next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
    }
}

fn seed_durable_turns(store: &mut InMemoryStore, session_id: &SessionId, run_id: &RunId) {
    let run = RunProjection {
        id: run_id.clone(),
        session_id: session_id.clone(),
        runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-thread-workspace")
            .expect("runtime profile id"),
        objective: "thread workspace".to_string(),
        status: RunStatus::Running,
        source: crate::default_test_run_source(),
        execution_context: crate::default_test_execution_context(),
        harness: RunHarnessKind::Native,
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
                delta: "one".to_string(),
            },
        ),
        (
            12,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "two".to_string(),
            },
        ),
        (13, AgentStreamFrame::AssistantTurnCompleted),
        (14, AgentStreamFrame::AssistantTurnStarted),
        (
            15,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "three".to_string(),
            },
        ),
        (
            16,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "four".to_string(),
            },
        ),
        (17, AgentStreamFrame::AssistantTurnCompleted),
    ] {
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: run.clone(),
                user_turn: crate::UserTurnCommit::NoUserTurn,
                events: vec![agent_stream_event(run_id, frame)],
                occurred_at_ms,
                auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
            })
            .expect("durable turn should commit");
    }
}

#[test]
fn thread_workspace_memory_is_session_scoped_ordered_and_atomic() {
    let mut store = InMemoryStore::current();
    let first = session("session-thread-one");
    let second = session("session-thread-two");
    store.save_session(first.clone()).expect("first session");
    store.save_session(second.clone()).expect("second session");

    for (event, timestamp) in [
        (
            ThreadWorkspaceEvent::GoalSet {
                value: "goal".to_string(),
            },
            10,
        ),
        (
            ThreadWorkspaceEvent::PlanSet {
                value: "plan".to_string(),
            },
            20,
        ),
        (
            ThreadWorkspaceEvent::NotesSet {
                value: "notes".to_string(),
            },
            30,
        ),
        (
            ThreadWorkspaceEvent::RecapSet {
                value: "recap".to_string(),
            },
            40,
        ),
    ] {
        store
            .append_thread_workspace_event(&first.id, timestamp, event)
            .expect("event appends");
    }
    let second_result = store
        .append_thread_workspace_event(
            &second.id,
            50,
            ThreadWorkspaceEvent::GoalSet {
                value: "other".to_string(),
            },
        )
        .expect("second event appends");
    assert_eq!(second_result.work_log[0].sequence, 1);

    let before = store
        .thread_workspace(&first.id)
        .expect("workspace")
        .expect("record");
    assert_eq!(
        (
            before.goal.as_str(),
            before.plan.as_str(),
            before.notes.as_str(),
            before.recap.as_str()
        ),
        ("goal", "plan", "notes", "recap")
    );
    assert_eq!(
        before
            .work_log
            .iter()
            .map(|entry| (entry.sequence, entry.occurred_at_ms))
            .collect::<Vec<_>>(),
        vec![(1, 10), (2, 20), (3, 30), (4, 40)]
    );
    let error = store
        .append_thread_workspace_event(
            &first.id,
            50,
            ThreadWorkspaceEvent::PinRemoved {
                cursor: ta_protocol::wire::ActivityCursor { sequence: 1 },
            },
        )
        .expect_err("missing pin removal fails");
    assert!(matches!(
        error,
        StoreError::AgentTurnProjectionViolation { .. }
    ));
    assert_eq!(
        store.thread_workspace(&first.id).expect("workspace"),
        Some(before)
    );
}

#[test]
fn thread_workspace_memory_validates_durable_pins_without_consuming_failed_sequences() {
    let mut store = InMemoryStore::current();
    let session_a = session("session-thread-pin-a");
    let session_b = session("session-thread-pin-b");
    let run_a = RunId::new("run-thread-pin-a").expect("run id");
    let run_b = RunId::new("run-thread-pin-b").expect("run id");
    store.save_session(session_a.clone()).expect("session A");
    store.save_session(session_b.clone()).expect("session B");
    seed_durable_turns(&mut store, &session_a.id, &run_a);
    seed_durable_turns(&mut store, &session_b.id, &run_b);

    let earlier_pin = ThreadWorkspacePin {
        run_id: run_a.clone(),
        cursor: ActivityCursor { sequence: 4 },
    };
    let later_pin = ThreadWorkspacePin {
        run_id: run_a.clone(),
        cursor: ActivityCursor { sequence: 8 },
    };
    let failures = [
        ThreadWorkspaceEvent::PinAdded {
            pin: ThreadWorkspacePin {
                run_id: run_b.clone(),
                cursor: ActivityCursor { sequence: 1 },
            },
        },
        ThreadWorkspaceEvent::PinAdded {
            pin: ThreadWorkspacePin {
                run_id: run_a.clone(),
                cursor: ActivityCursor { sequence: 0 },
            },
        },
        ThreadWorkspaceEvent::PinAdded {
            pin: ThreadWorkspacePin {
                run_id: RunId::new("run-thread-pin-wrong").expect("run id"),
                cursor: ActivityCursor { sequence: 1 },
            },
        },
    ];
    for (index, event) in failures.into_iter().enumerate() {
        assert!(
            store
                .append_thread_workspace_event(&session_a.id, 20, event)
                .is_err()
        );
        let valid = store
            .append_thread_workspace_event(
                &session_a.id,
                21,
                ThreadWorkspaceEvent::GoalSet {
                    value: "goal".to_string(),
                },
            )
            .expect("valid event after rejection");
        assert_eq!(
            valid.work_log.last().expect("work log").sequence,
            index as u64 + 1
        );
    }
    let pinned = store
        .append_thread_workspace_event(
            &session_a.id,
            22,
            ThreadWorkspaceEvent::PinAdded {
                pin: later_pin.clone(),
            },
        )
        .expect("valid pin");
    assert_eq!(pinned.work_log.last().expect("work log").sequence, 4);
    let pinned = store
        .append_thread_workspace_event(
            &session_a.id,
            23,
            ThreadWorkspaceEvent::PinAdded {
                pin: earlier_pin.clone(),
            },
        )
        .expect("an earlier valid durable turn may be pinned after a later turn");
    assert_eq!(
        pinned
            .pins
            .iter()
            .map(|pin| pin.cursor.sequence)
            .collect::<Vec<_>>(),
        vec![4, 8]
    );
    assert_eq!(pinned.work_log.last().expect("work log").sequence, 5);
    for (index, event) in [
        ThreadWorkspaceEvent::PinAdded { pin: later_pin },
        ThreadWorkspaceEvent::PinRemoved {
            cursor: ActivityCursor { sequence: 99 },
        },
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            store
                .append_thread_workspace_event(&session_a.id, 23, event)
                .is_err()
        );
        let valid = store
            .append_thread_workspace_event(
                &session_a.id,
                24,
                ThreadWorkspaceEvent::NotesSet {
                    value: "notes".to_string(),
                },
            )
            .expect("valid event after rejection");
        assert_eq!(
            valid.work_log.last().expect("work log").sequence,
            index as u64 + 6
        );
    }
    let removed = store
        .append_thread_workspace_event(
            &session_a.id,
            25,
            ThreadWorkspaceEvent::PinRemoved {
                cursor: ActivityCursor { sequence: 8 },
            },
        )
        .expect("pin removes");
    assert_eq!(
        removed
            .pins
            .iter()
            .map(|pin| pin.cursor.sequence)
            .collect::<Vec<_>>(),
        vec![4]
    );
    assert_eq!(removed.work_log.last().expect("work log").sequence, 8);
    let removed = store
        .append_thread_workspace_event(
            &session_a.id,
            26,
            ThreadWorkspaceEvent::PinRemoved {
                cursor: earlier_pin.cursor,
            },
        )
        .expect("earlier pin removes");
    assert!(removed.pins.is_empty());
    assert_eq!(removed.work_log.last().expect("work log").sequence, 9);
}
