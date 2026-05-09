use super::*;

#[test]
fn session_read_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("session-decode-corruption");
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
        .conn
        .execute(
            "UPDATE sessions SET data_json = ?1 WHERE id = ?2",
            params!["{", session_id.as_str()],
        )
        .expect("corrupt session json");

    let error = store
        .session(&session_id)
        .expect_err("session read must fail on corrupt json");
    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "session_projection",
            ..
        }
    ));

    let _ = std::fs::remove_file(path);
}

#[test]
fn sessions_read_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("sessions-decode-corruption");
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
        .conn
        .execute(
            "UPDATE sessions SET data_json = ?1 WHERE id = ?2",
            params!["{", session_id.as_str()],
        )
        .expect("corrupt session json");

    let error = store
        .sessions()
        .expect_err("session list read must fail on corrupt json");
    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "session_projection",
            ..
        }
    ));

    let _ = std::fs::remove_file(path);
}

#[test]
fn run_read_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("run-decode-corruption");
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
    store
        .save_run(RunProjection {
            id: run_id.clone(),
            session_id,
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Persisted".to_string(),
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
            params!["{", run_id.as_str()],
        )
        .expect("corrupt run json");

    let error = store
        .run(&run_id)
        .expect_err("run read must fail on corrupt json");
    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "run_projection",
            ..
        }
    ));

    let _ = std::fs::remove_file(path);
}

#[test]
fn runs_read_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("runs-decode-corruption");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
            id: session_id,
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
            id: run_id.clone(),
            session_id: SessionId::new("session-1").expect("session id"),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Persisted".to_string(),
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
            params!["{", run_id.as_str()],
        )
        .expect("corrupt run json");

    let error = store
        .runs()
        .expect_err("run list read must fail on corrupt json");
    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "run_projection",
            ..
        }
    ));

    let _ = std::fs::remove_file(path);
}
