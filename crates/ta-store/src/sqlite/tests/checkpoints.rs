use super::*;

#[test]
fn commit_checkpoint_persist_persists_checkpoint_under_canonical_commit_boundary() {
    let path = test_db_path("commit-checkpoint");
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
            objective: "Ship checkpoint".to_string(),
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
        })
        .expect("run should persist");

    let committed = store
        .commit_checkpoint_persist(CommitCheckpointPersist {
            checkpoint: CheckpointRecord {
                run_id: run_id.clone(),
                revision: 1,
                artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect("checkpoint should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.commit.first_sequence, 0);
    assert_eq!(committed.commit.last_sequence, 0);
    assert_eq!(
        ok(store.checkpoints_for_run(&run_id)),
        vec![CheckpointRecord {
            run_id,
            revision: 1,
            artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
        }]
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn checkpoints_for_run_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("checkpoint-decode-corruption");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
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
            .expect("session");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship checkpoint".to_string(),
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
            })
            .expect("run");
        store
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: CheckpointRecord {
                    run_id: run_id.clone(),
                    revision: 1,
                    artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
                },
                occurred_at_ms: 20,
            })
            .expect("checkpoint");
    }

    let conn = Connection::open(&path).expect("sqlite should reopen directly");
    conn.execute(
        "UPDATE checkpoints SET data_json = ? WHERE run_id = ?",
        params!["{", run_id.as_str()],
    )
    .expect("corrupt checkpoint json");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should reopen");
    let error = store
        .checkpoints_for_run(&run_id)
        .expect_err("checkpoint read must fail on corrupt json");
    assert_eq!(
        error,
        StoreError::DecodeRecord {
            entity: "checkpoint_record",
            source: serde_json::Error::io(std::io::Error::other("ignored")),
        }
    );
    let _ = std::fs::remove_file(path);
}
