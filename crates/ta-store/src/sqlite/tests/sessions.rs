use super::*;

#[test]
fn rotate_session_authority_consumes_recovery_slot_once() {
    let path = test_db_path("rotate-session-authority");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "authority-a0".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Selected".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

    let rotated_once = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a1",
        )
        .expect("rotation should read")
        .expect("current authority should rotate");
    assert_eq!(rotated_once.current_session_authority_hash, "authority-a1");
    assert_eq!(rotated_once.current_session_authority_generation, 1);
    assert_eq!(
        rotated_once.recovery_session_authority_hash.as_deref(),
        Some("authority-a0")
    );
    assert_eq!(rotated_once.recovery_session_authority_generation, Some(0));

    let recovered = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a2",
        )
        .expect("rotation should read")
        .expect("recovery authority should recover once");
    assert_eq!(recovered.current_session_authority_hash, "authority-a2");
    assert_eq!(recovered.current_session_authority_generation, 2);
    assert_eq!(recovered.recovery_session_authority_hash, None);
    assert_eq!(recovered.recovery_session_authority_generation, None);

    assert!(
        store
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a0",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );
    assert!(
        store
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a1",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );
    let rotated_again = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a2",
            "authority-a3",
        )
        .expect("rotation should read")
        .expect("current authority should rotate again");
    assert_eq!(rotated_again.current_session_authority_hash, "authority-a3");
    assert_eq!(rotated_again.current_session_authority_generation, 3);
    assert_eq!(
        rotated_again.recovery_session_authority_hash.as_deref(),
        Some("authority-a2")
    );
    assert_eq!(rotated_again.recovery_session_authority_generation, Some(2));

    let _ = std::fs::remove_file(path);
}

#[test]
fn rotate_session_authority_persists_recovery_slot_across_reopen() {
    let path = test_db_path("rotate-session-authority-reopen");
    let session_id = SessionId::new("session-1").expect("session id");

    let mut store = SqliteStore::open(&path).expect("store should open");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "authority-a0".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Selected".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    let rotated_once = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a1",
        )
        .expect("rotation should read")
        .expect("current authority should rotate");
    assert_eq!(rotated_once.current_session_authority_hash, "authority-a1");
    assert_eq!(rotated_once.current_session_authority_generation, 1);
    assert_eq!(
        rotated_once.recovery_session_authority_hash.as_deref(),
        Some("authority-a0")
    );
    assert_eq!(rotated_once.recovery_session_authority_generation, Some(0));
    drop(store);

    let mut reopened = SqliteStore::open(&path).expect("store should reopen");
    let persisted = some(reopened.session(&session_id));
    assert_eq!(persisted.current_session_authority_hash, "authority-a1");
    assert_eq!(persisted.current_session_authority_generation, 1);
    assert_eq!(
        persisted.recovery_session_authority_hash.as_deref(),
        Some("authority-a0")
    );
    assert_eq!(persisted.recovery_session_authority_generation, Some(0));

    let recovered = reopened
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a2",
        )
        .expect("rotation should read")
        .expect("recovery authority should recover once");
    assert_eq!(recovered.current_session_authority_hash, "authority-a2");
    assert_eq!(recovered.current_session_authority_generation, 2);
    assert_eq!(recovered.recovery_session_authority_hash, None);
    assert_eq!(recovered.recovery_session_authority_generation, None);
    assert!(
        reopened
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a0",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );
    assert!(
        reopened
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a1",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn open_persists_and_reloads_existing_rows() {
    let path = test_db_path("reload");
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .save_run(RunProjection {
            id: RunId::new("run-1").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Persist checkpoint".to_string(),
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
    store
        .commit_checkpoint_persist(CommitCheckpointPersist {
            checkpoint: crate::test_checkpoint_record(RunId::new("run-1").expect("run id"), 1),
            occurred_at_ms: 20,
        })
        .expect("checkpoint should persist");

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    assert_eq!(some(reopened.session(&session_id)).title, "Persisted");
    assert_eq!(
        ok(reopened.checkpoints_for_run(&RunId::new("run-1").expect("run id"))).len(),
        1
    );
    let _ = std::fs::remove_file(path);
}
