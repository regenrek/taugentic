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
fn open_backfills_additive_schema_objects_for_existing_store() {
    let path = test_db_path("backfill-additive-schema");
    let conn = Connection::open(&path).expect("sqlite should open directly");
    conn.execute_batch(
        "
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                data_json TEXT NOT NULL,
                last_commit_id INTEGER REFERENCES commits(id)
            );
            CREATE TABLE commits (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                kind TEXT NOT NULL,
                occurred_at_ms INTEGER NOT NULL,
                first_sequence INTEGER NOT NULL,
                last_sequence INTEGER NOT NULL,
                CHECK(first_sequence <= last_sequence)
            );
            CREATE INDEX idx_commits_session_id
                ON commits (session_id, id);
            CREATE TABLE runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                data_json TEXT NOT NULL,
                last_commit_id INTEGER REFERENCES commits(id)
            );
            CREATE INDEX idx_runs_session_id
                ON runs (session_id);
            CREATE TABLE events (
                sequence INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                occurred_at_ms INTEGER NOT NULL,
                payload_json TEXT NOT NULL
            );
            CREATE INDEX idx_events_session_sequence
                ON events (session_id, sequence);
            CREATE TABLE checkpoints (
                run_id TEXT NOT NULL REFERENCES runs(id),
                revision INTEGER NOT NULL,
                data_json TEXT NOT NULL,
                commit_id INTEGER NOT NULL UNIQUE REFERENCES commits(id),
                PRIMARY KEY (run_id, revision)
            );
            CREATE TABLE artifacts (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                run_id TEXT NOT NULL REFERENCES runs(id),
                data_json TEXT NOT NULL,
                last_commit_id INTEGER REFERENCES commits(id)
            );
            CREATE INDEX idx_artifacts_run_id
                ON artifacts (run_id);
            CREATE INDEX idx_artifacts_session_id
                ON artifacts (session_id);
            ",
    )
    .expect("legacy schema fixture should write");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should backfill additive schema");
    store
        .validate_current_schema()
        .expect("backfilled schema should validate");

    let conn = Connection::open(store.path()).expect("sqlite should reopen directly");
    let principal_table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type = 'table' AND name = 'principals'",
            [],
            |row| row.get(0),
        )
        .expect("principal table query");
    let principal_index_exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM sqlite_master WHERE type = 'index' AND name = 'idx_principals_credential_hash'",
                [],
                |row| row.get(0),
            )
            .expect("principal index query");
    let run_started_index_exists: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type = 'index' AND name = 'idx_run_projections_session_started_at'",
            [],
            |row| row.get(0),
        )
        .expect("run started index query");
    let event_replay_index_exists: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type = 'index' AND name = 'idx_events_session_run_seq'",
            [],
            |row| row.get(0),
        )
        .expect("event replay index query");
    assert_eq!(principal_table_exists, 1);
    assert_eq!(principal_index_exists, 1);
    assert_eq!(run_started_index_exists, 1);
    assert_eq!(event_replay_index_exists, 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn open_backfills_context_receipt_parent_run_id_column() {
    let path = test_db_path("backfill-receipt-parent-run-id");
    let store = SqliteStore::open(&path).expect("store should open");
    drop(store);

    let conn = Connection::open(&path).expect("sqlite should reopen directly");
    conn.execute_batch(
        r#"
            DROP INDEX idx_context_receipts_session_run;
            DROP INDEX idx_context_receipts_session_state;
            DROP INDEX idx_context_receipts_session_kind;
            DROP INDEX idx_context_receipts_session_parent_run;
            DROP INDEX idx_context_receipts_artifact_unique;
            DROP INDEX idx_context_receipts_event_turn_unique;
            CREATE TABLE context_receipts_legacy (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                state TEXT NOT NULL,
                kind TEXT NOT NULL,
                provenance_json TEXT NOT NULL,
                data_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                promoted_at_ms INTEGER,
                quarantined_at_ms INTEGER,
                last_commit_id TEXT
            );
            INSERT INTO context_receipts_legacy (
                id,
                session_id,
                run_id,
                state,
                kind,
                provenance_json,
                data_json,
                created_at_ms,
                promoted_at_ms,
                quarantined_at_ms,
                last_commit_id
            ) VALUES (
                'receipt-legacy',
                'session-legacy',
                'run-legacy',
                'returned',
                'patch',
                '{"artifactId":"artifact-legacy"}',
                '{"id":"receipt-legacy","sessionId":"session-legacy","runId":"run-legacy","parentRunId":"parent-legacy","kind":"patch","provenance":{"artifactId":"artifact-legacy"},"state":"returned","createdAtMs":"1"}',
                1,
                NULL,
                NULL,
                NULL
            );
            DROP TABLE context_receipts;
            ALTER TABLE context_receipts_legacy RENAME TO context_receipts;
            "#,
    )
    .expect("legacy receipt schema fixture should write");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should migrate receipt parent_run_id");
    store
        .validate_current_schema()
        .expect("migrated schema should validate");
    let conn = Connection::open(store.path()).expect("sqlite should reopen directly");
    let parent_run_id: String = conn
        .query_row(
            "SELECT parent_run_id FROM context_receipts WHERE id = 'receipt-legacy'",
            [],
            |row| row.get(0),
        )
        .expect("parent run id should backfill");
    assert_eq!(parent_run_id, "parent-legacy");

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
