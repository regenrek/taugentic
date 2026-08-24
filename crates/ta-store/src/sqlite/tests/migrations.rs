use super::*;

#[test]
fn open_initializes_current_schema_and_pragmas() {
    let path = test_db_path("current-schema");
    let store = SqliteStore::open(&path).expect("store should open");

    let foreign_keys = store.pragma_i64("foreign_keys").expect("foreign keys");
    let busy_timeout = store.pragma_i64("busy_timeout").expect("busy timeout");

    store
        .validate_current_schema()
        .expect("current schema should validate");
    assert_eq!(foreign_keys, 1);
    assert_eq!(busy_timeout, SQLITE_BUSY_TIMEOUT.as_millis() as i64);

    let _ = std::fs::remove_file(path);
}

#[test]
fn open_rejects_foreign_key_violations() {
    let path = test_db_path("foreign-key-violation");
    let store = SqliteStore::open(&path).expect("store should open");
    let conn = Connection::open(store.path()).expect("sqlite should reopen directly");
    conn.pragma_update(None, "foreign_keys", "OFF")
        .expect("disable foreign keys for corruption fixture");
    conn.execute(
        "INSERT INTO runs (id, session_id, data_json, last_commit_id) VALUES (?, ?, ?, NULL)",
        params![
            "run-orphan",
            "session-missing",
            serde_json::to_string(&RunProjection {
                id: RunId::new("run-orphan").expect("run id"),
                session_id: SessionId::new("session-missing").expect("session id"),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Corrupt".to_string(),
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
            })
            .expect("run json")
        ],
    )
    .expect("corrupt orphan run");
    drop(conn);
    drop(store);

    let error = SqliteStore::open(&path).expect_err("store should reject orphan rows");
    match error {
        StoreError::ForeignKeyCheckFailed { detail, .. } => {
            assert!(detail.contains("table=runs"), "unexpected detail: {detail}");
        }
        other => panic!("expected ForeignKeyCheckFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn open_rejects_orphan_artifact_rows() {
    let path = test_db_path("foreign-key-artifact-violation");
    let store = SqliteStore::open(&path).expect("store should open");
    let conn = Connection::open(store.path()).expect("sqlite should reopen directly");
    conn.pragma_update(None, "foreign_keys", "OFF")
        .expect("disable foreign keys for corruption fixture");
    conn.execute(
        "INSERT INTO artifacts (id, session_id, run_id, data_json, last_commit_id)
             VALUES (?, ?, ?, ?, NULL)",
        params![
            "artifact-orphan",
            "session-1",
            "run-missing",
            serde_json::to_string(&ArtifactRecord {
                id: ArtifactId::new("artifact-orphan").expect("artifact id"),
                session_id: SessionId::new("session-1").expect("session id"),
                run_id: RunId::new("run-missing").expect("run id"),
                kind: ArtifactKind::Transcript,
                storage_path: "artifacts/run-missing/transcript.md".to_string(),
            })
            .expect("artifact json")
        ],
    )
    .expect("corrupt orphan artifact");
    drop(conn);
    drop(store);

    let error = SqliteStore::open(&path).expect_err("store should reject orphan artifact rows");
    match error {
        StoreError::ForeignKeyCheckFailed { detail, .. } => {
            assert!(
                detail.contains("table=artifacts"),
                "unexpected detail: {detail}"
            );
            assert!(
                detail.contains("parent=runs"),
                "unexpected detail: {detail}"
            );
        }
        other => panic!("expected ForeignKeyCheckFailed, got {other:?}"),
    }
    let _ = std::fs::remove_file(path);
}
