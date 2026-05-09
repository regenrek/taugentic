use super::*;

#[test]
fn commit_run_transition_persists_run_session_status_and_activity_atomically() {
    let path = test_db_path("commit-run");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                run_id: run_id.clone(),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect("run should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.commit.first_sequence, 1);
    assert_eq!(committed.commit.last_sequence, 1);
    assert_eq!(committed.session.status, SessionStatus::Running);
    assert_eq!(some(store.run(&run_id)).status, RunStatus::Running);
    assert_eq!(
        some(store.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::Run],
        }))
        .records
        .len(),
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_persists_only_durable_agent_stream_frames() {
    let path = test_db_path("commit-run-agent-stream-durable");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

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
        .expect("run should commit");

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
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(
        ok(store.session_event_range(&crate::SessionEventRangeQuery {
            session_id: session_id.clone(),
            after_sequence: None,
            up_to_sequence: None,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![1, 3]
    );

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    assert_eq!(
        ok(reopened.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rolls_back_when_existing_run_projection_is_corrupt() {
    let path = test_db_path("commit-run-existing-corrupt");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");
    store
        .save_run(RunProjection {
            id: RunId::new("run-corrupt").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Corrupt me".to_string(),
            status: RunStatus::Running,
            source: RunSource::default(),
            harness: RunHarnessKind::Unknown,
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        })
        .expect("run should persist");
    store
        .conn
        .execute(
            "UPDATE runs SET data_json = ?1 WHERE id = ?2",
            params!["{", "run-corrupt"],
        )
        .expect("corrupt existing run json");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-new").expect("run id"),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Should roll back".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                run_id: RunId::new("run-new").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect_err("commit should fail on corrupt existing run projection");

    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "run_projection",
            ..
        }
    ));
    assert!(ok(store.run(&RunId::new("run-new").expect("run id"))).is_none());
    assert_eq!(some(store.session(&session_id)).status, SessionStatus::Idle);
    let event_count: i64 = store
        .conn
        .query_row("SELECT COUNT(1) FROM events", [], |row| row.get(0))
        .expect("event count");
    let commit_count: i64 = store
        .conn
        .query_row("SELECT COUNT(1) FROM commits", [], |row| row.get(0))
        .expect("commit count");
    assert_eq!(event_count, 0);
    assert_eq!(commit_count, 0);

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_cross_session_run_projection() {
    let path = test_db_path("commit-run-cross-session");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id: SessionId::new("session-2").expect("session id"),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                run_id: RunId::new("run-1").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect_err("cross-session run commit must fail");

    assert_eq!(
        error,
        StoreError::CommitSessionMismatch {
            entity: "run",
            expected: "session-1".to_string(),
            actual: "session-2".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_orphan_approval_resolution() {
    let path = test_db_path("commit-run-orphan-approval-resolution");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                resolution: ta_protocol::wire::ApprovalResolution::new(
                    ApprovalId::new("approval-1").expect("approval id"),
                    run_id,
                    ApprovalDecision::Approved,
                    ta_protocol::wire::ApprovalResolutionReason::User,
                    ta_protocol::wire::ApprovalActor::new("principal-sqlite-tests")
                        .expect("approval actor"),
                    None,
                ),
            })],
            occurred_at_ms: 20,
        })
        .expect_err("orphan approval resolution must fail");

    assert_eq!(
        error,
        StoreError::ApprovalLifecycleViolation {
            approval_id: "approval-1".to_string(),
            detail: "approval resolution does not match a pending request".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_mismatched_run_event_run_id() {
    let path = test_db_path("commit-run-mismatched-event-run-id");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Persisted".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                run_id: RunId::new("run-2").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect_err("mismatched run event run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-1".to_string(),
            actual: "run-2".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_mismatched_agent_stream_run_id() {
    let path = test_db_path("commit-run-transition-agent-stream-mismatch");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
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

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
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
            events: vec![DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: RunId::new("run-2").expect("run id"),
                emission: ta_protocol::wire::StreamEmission {
                    turn_id: None,
                    item_id: None,
                    fragment_sequence: None,
                    frame: AgentStreamFrame::AssistantTurnStarted,
                },
            })],
            occurred_at_ms: 20,
        })
        .expect_err("mismatched agent stream run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-1".to_string(),
            actual: "run-2".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_session_open_persists_session_and_allocates_event_sequence() {
    let path = test_db_path("commit-session-open");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");

    crate::WorkspaceRepository::upsert_workspace(&mut store, crate::default_test_workspace())
        .expect("seed workspace");
    let committed = store
        .commit_session_open(CommitSessionOpen {
            session: SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Persisted".to_string(),
                status: SessionStatus::Idle,
                workspace_id: crate::default_test_workspace_id(),
            },
            occurred_at_ms: 20,
        })
        .expect("session should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.event.sequence, 1);
    assert_eq!(some(store.session(&session_id)).title, "Persisted");
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::Session],
        }))
        .records
        .len(),
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn reopen_preserves_committed_run_transition_and_continues_event_sequence() {
    let path = test_db_path("reopen-committed-run");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        let session_id = SessionId::new("session-1").expect("session id");
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Persisted".to_string(),
                status: SessionStatus::Idle,
                workspace_id: crate::default_test_workspace_id(),
            })
            .expect("session should persist");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    id: RunId::new("run-1").expect("run id"),
                    session_id,
                    runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new(
                        "runtime-codex-safe",
                    )
                    .expect("runtime profile id"),
                    objective: "Ship store boundary".to_string(),
                    status: RunStatus::Running,
                    source: RunSource::default(),
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
                events: vec![DaemonEvent::Run(ta_protocol::wire::RunEvent {
                    run_id: RunId::new("run-1").expect("run id"),
                    status: RunStatus::Running,
                    detail: "Execution started".to_string(),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                })],
                occurred_at_ms: 20,
            })
            .expect("run should commit");
    }

    let mut reopened = SqliteStore::open(&path).expect("store should reopen");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let second = reopened
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id: session_id.clone(),
                run_id,
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 30,
        })
        .expect("artifact should commit");

    assert_eq!(
        some(reopened.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(second.event.sequence, 2);

    let _ = std::fs::remove_file(path);
}

#[test]
fn reopen_preserves_committed_checkpoint_and_next_event_sequence() {
    let path = test_db_path("reopen-committed-checkpoint");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        let session_id = SessionId::new("session-1").expect("session id");
        let run_id = RunId::new("run-1").expect("run id");
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Persisted".to_string(),
                status: SessionStatus::Running,
                workspace_id: crate::default_test_workspace_id(),
            })
            .expect("session should persist");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Persist checkpoint".to_string(),
                status: RunStatus::Running,
                source: RunSource::default(),
                harness: RunHarnessKind::Unknown,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            })
            .expect("run should persist");
        store
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: CheckpointRecord {
                    run_id,
                    revision: 1,
                    artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
                },
                occurred_at_ms: 20,
            })
            .expect("checkpoint should commit");
    }

    let mut reopened = SqliteStore::open(&path).expect("store should reopen");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let artifact = reopened
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id,
                run_id: run_id.clone(),
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 30,
        })
        .expect("artifact should commit");

    assert_eq!(artifact.commit.id, 2);
    assert_eq!(artifact.event.sequence, 1);
    assert_eq!(
        ok(reopened.checkpoints_for_run(&run_id)),
        vec![CheckpointRecord {
            run_id,
            revision: 1,
            artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
        }]
    );

    let _ = std::fs::remove_file(path);
}
