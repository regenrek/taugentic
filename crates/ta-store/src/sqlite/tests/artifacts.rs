use super::*;
use crate::StoreSeedRepository;
use ta_protocol::wire::{
    AgentRuntimeStrategyId, AgentStreamItemId, AgentStreamTurnId, ArtifactMetadata,
    ImageArtifactMetadata, ImageArtifactProvenance, ImageMediaType, RuntimeProfileId,
};

fn image_metadata() -> ArtifactMetadata {
    ArtifactMetadata::Image(ImageArtifactMetadata {
        media_type: ImageMediaType::Png,
        sha256: "sha256:fixture".to_string(),
        byte_len: 8,
        provenance: ImageArtifactProvenance {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            provider_id: AgentRuntimeStrategyId::new("codex").expect("provider id"),
            turn_id: AgentStreamTurnId::new("turn-fixture").expect("turn id"),
            item_id: AgentStreamItemId::new("item-fixture").expect("item id"),
        },
    })
}

fn metadata_mismatch(id: &str, image_kind: bool) -> ArtifactRecord {
    ArtifactRecord {
        id: ArtifactId::new(id).expect("artifact id"),
        session_id: SessionId::new("session-metadata").expect("session id"),
        run_id: RunId::new("run-metadata").expect("run id"),
        kind: if image_kind {
            ArtifactKind::Image
        } else {
            ArtifactKind::Patch
        },
        metadata: if image_kind {
            ArtifactMetadata::Standard
        } else {
            image_metadata()
        },
        storage_path: "artifact.bin".to_string(),
    }
}

fn metadata_commit_store(path: &std::path::Path) -> SqliteStore {
    let mut store = SqliteStore::open(path).expect("store should open");
    store
        .save_session(SessionProjection {
            id: SessionId::new("session-metadata").expect("session id"),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Metadata".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session");
    store
        .save_run(RunProjection {
            id: RunId::new("run-metadata").expect("run id"),
            session_id: SessionId::new("session-metadata").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Validate metadata".to_string(),
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
}

#[test]
fn artifact_metadata_mismatch_is_rejected_at_seed_and_commit_boundaries() {
    for (id, image_kind) in [
        ("artifact-seed-image", true),
        ("artifact-seed-standard", false),
    ] {
        let path = test_db_path(id);
        let mut store = SqliteStore::open(&path).expect("store should open");
        let error = store
            .save_artifact(metadata_mismatch(id, image_kind))
            .expect_err("seed must reject either metadata mismatch direction");
        assert!(matches!(error, StoreError::ArtifactMetadataMismatch { .. }));
        let _ = std::fs::remove_file(path);
    }

    for (id, image_kind) in [
        ("artifact-commit-image", true),
        ("artifact-commit-standard", false),
    ] {
        let path = test_db_path(id);
        let mut store = metadata_commit_store(&path);
        let error = store
            .commit_artifact_publish(CommitArtifactPublish {
                artifact: metadata_mismatch(id, image_kind),
                occurred_at_ms: 1,
            })
            .expect_err("commit must reject either metadata mismatch direction");
        assert!(matches!(error, StoreError::ArtifactMetadataMismatch { .. }));
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn commit_artifact_publish_persists_artifact_and_activity_atomically() {
    let path = test_db_path("commit-artifact");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let artifact_id = ArtifactId::new("artifact-1").expect("artifact id");
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
            id: run_id.clone(),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship artifact".to_string(),
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
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session_id.clone(),
                run_id,
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect("artifact should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.event.sequence, 1);
    assert_eq!(
        some(store.artifact(&artifact_id)).storage_path,
        "artifacts/run-1/patch.diff"
    );
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::Artifact],
        }))
        .records
        .len(),
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_artifact_publish_rejects_cross_session_run_projection() {
    let path = test_db_path("commit-artifact-cross-session");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let other_session_id = SessionId::new("session-2").expect("session id");
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .save_session(SessionProjection {
            id: other_session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Other".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .save_run(RunProjection {
            id: run_id.clone(),
            session_id: other_session_id,
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship artifact".to_string(),
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

    let error = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id: session_id.clone(),
                run_id,
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect_err("cross-session artifact commit must fail");

    assert_eq!(
        error,
        StoreError::CommitSessionMismatch {
            entity: "artifact",
            expected: "session-2".to_string(),
            actual: "session-1".to_string(),
        }
    );
    assert!(
        ok(store.artifacts_for_session(&crate::SessionArtifactQuery {
            session_id,
            run_id: None,
            artifact_id: None,
        }))
        .is_empty()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_artifact_publish_rejects_non_running_run_projection() {
    let path = test_db_path("commit-artifact-non-running");
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .save_run(RunProjection {
            id: run_id.clone(),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship artifact".to_string(),
            status: RunStatus::Cancelled,
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

    let error = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id: session_id.clone(),
                run_id,
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect_err("non-running artifact commit must fail");

    assert_eq!(
        error,
        StoreError::CommitRunStatusMismatch {
            entity: "artifact",
            expected: RunStatus::Running,
            actual: RunStatus::Cancelled,
        }
    );
    assert!(
        ok(store.artifacts_for_session(&crate::SessionArtifactQuery {
            session_id,
            run_id: None,
            artifact_id: None,
        }))
        .is_empty()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn artifact_read_returns_decode_record_for_corrupt_data_json() {
    let path = test_db_path("artifact-decode-corruption");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let artifact_id = ArtifactId::new("artifact-1").expect("artifact id");
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
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            })
            .expect("session");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship artifact".to_string(),
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
            .commit_artifact_publish(CommitArtifactPublish {
                artifact: ArtifactRecord {
                    id: artifact_id.clone(),
                    session_id,
                    run_id,
                    kind: ArtifactKind::Patch,
                    metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                    storage_path: "artifacts/run-1/patch.diff".to_string(),
                },
                occurred_at_ms: 20,
            })
            .expect("artifact");
    }

    let conn = Connection::open(&path).expect("sqlite should reopen directly");
    conn.execute(
        "UPDATE artifacts SET data_json = ? WHERE id = ?",
        params!["{", artifact_id.as_str()],
    )
    .expect("corrupt artifact json");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should reopen");
    let error = store
        .artifact(&artifact_id)
        .expect_err("artifact read must fail on corrupt json");
    assert_eq!(
        error,
        StoreError::DecodeRecord {
            entity: "artifact_record",
            source: serde_json::Error::io(std::io::Error::other("ignored")),
        }
    );
    let _ = std::fs::remove_file(path);
}
