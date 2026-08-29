use super::*;

impl SqliteStore {
    pub(super) fn ensure_current_store(&mut self) -> Result<(), StoreError> {
        if self.is_empty_store()? {
            self.initialize_current_schema()?;
            return Ok(());
        }
        self.validate_current_schema()
    }

    #[cfg(test)]
    pub(super) fn pragma_i64(&self, name: &'static str) -> Result<i64, StoreError> {
        self.conn
            .pragma_query_value(None, name, |row| row.get(0))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
    }

    fn is_empty_store(&self) -> Result<bool, StoreError> {
        let object_count = self
            .conn
            .query_row(
                "SELECT COUNT(1) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        Ok(object_count == 0)
    }

    fn initialize_current_schema(&self) -> Result<(), StoreError> {
        self.ensure_current_schema()?;
        self.validate_current_schema()
    }

    fn ensure_current_schema(&self) -> Result<(), StoreError> {
        self.conn
            .execute_batch(
                "
                BEGIN;
                CREATE TABLE IF NOT EXISTS workspaces (
                    id TEXT PRIMARY KEY,
                    root_realpath TEXT NOT NULL UNIQUE,
                    display_name TEXT NOT NULL,
                    trust_state TEXT NOT NULL,
                    git_repo_root TEXT,
                    created_at TEXT NOT NULL,
                    last_used_at TEXT NOT NULL,
                    data_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS navigation_states (
                    owner_principal_id TEXT PRIMARY KEY REFERENCES principals(id),
                    data_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS auth_profiles (
                    id TEXT PRIMARY KEY,
                    auth_method_id TEXT NOT NULL,
                    provider_id TEXT NOT NULL,
                    sort_order INTEGER NOT NULL,
                    is_default INTEGER NOT NULL,
                    data_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_auth_profiles_method_order
                    ON auth_profiles (provider_id, auth_method_id, sort_order, id);
                CREATE TABLE IF NOT EXISTS code_host_accounts (
                    id TEXT PRIMARY KEY,
                    owner_principal_id TEXT NOT NULL REFERENCES principals(id),
                    provider TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    data_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS plugin_installations (
                    owner_principal_id TEXT NOT NULL REFERENCES principals(id),
                    plugin_id TEXT NOT NULL,
                    version TEXT NOT NULL,
                    digest_sha256 TEXT NOT NULL,
                    data_json TEXT NOT NULL,
                    PRIMARY KEY (owner_principal_id, plugin_id, version, digest_sha256)
                );
                CREATE INDEX IF NOT EXISTS idx_code_host_accounts_owner_provider_name
                    ON code_host_accounts (owner_principal_id, provider, display_name, id);
                CREATE INDEX IF NOT EXISTS idx_workspaces_root_realpath
                    ON workspaces (root_realpath);
                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    data_json TEXT NOT NULL,
                    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
                    last_commit_id INTEGER REFERENCES commits(id)
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id
                    ON sessions (workspace_id);
                CREATE TABLE IF NOT EXISTS principals (
                    id TEXT PRIMARY KEY,
                    credential_hash TEXT NOT NULL UNIQUE,
                    data_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_principals_credential_hash
                    ON principals (credential_hash);
                CREATE TABLE IF NOT EXISTS commits (
                    id INTEGER PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    kind TEXT NOT NULL,
                    occurred_at_ms INTEGER NOT NULL,
                    first_sequence INTEGER NOT NULL,
                    last_sequence INTEGER NOT NULL,
                    CHECK(first_sequence <= last_sequence)
                );
                CREATE INDEX IF NOT EXISTS idx_commits_session_id
                    ON commits (session_id, id);
                CREATE TABLE IF NOT EXISTS runs (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    data_json TEXT NOT NULL,
                    last_commit_id INTEGER REFERENCES commits(id)
                );
                CREATE INDEX IF NOT EXISTS idx_runs_session_id
                    ON runs (session_id);
                CREATE TABLE IF NOT EXISTS scheduled_work_definitions (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    data_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS scheduled_work_occurrences (
                    id TEXT PRIMARY KEY,
                    scheduled_work_id TEXT NOT NULL REFERENCES scheduled_work_definitions(id),
                    run_id TEXT REFERENCES runs(id),
                    state TEXT NOT NULL,
                    data_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_scheduled_work_occurrences_work_state
                    ON scheduled_work_occurrences (scheduled_work_id, state);
                CREATE INDEX IF NOT EXISTS idx_run_projections_session_started_at
                    ON runs (
                        session_id,
                        CASE
                            WHEN json_valid(data_json)
                            THEN json_extract(data_json, '$.started_at_ms')
                            ELSE NULL
                        END DESC
                    );
                CREATE TABLE IF NOT EXISTS events (
                    sequence INTEGER PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    occurred_at_ms INTEGER NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_events_session_sequence
                    ON events (session_id, sequence);
                CREATE INDEX IF NOT EXISTS idx_events_session_run_seq
                    ON events (session_id, sequence);
                CREATE TABLE IF NOT EXISTS agent_turn_rows (
                    sequence INTEGER PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    data_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_agent_turn_rows_session_sequence
                    ON agent_turn_rows (session_id, sequence);
                CREATE TABLE IF NOT EXISTS thread_workspace_events (
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    sequence INTEGER NOT NULL,
                    occurred_at_ms INTEGER NOT NULL,
                    data_json TEXT NOT NULL,
                    PRIMARY KEY (session_id, sequence)
                );
                CREATE TABLE IF NOT EXISTS thread_workspaces (
                    session_id TEXT PRIMARY KEY REFERENCES sessions(id),
                    projection_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS checkpoints (
                    run_id TEXT NOT NULL REFERENCES runs(id),
                    revision INTEGER NOT NULL,
                    data_json TEXT NOT NULL,
                    commit_id INTEGER NOT NULL UNIQUE REFERENCES commits(id),
                    PRIMARY KEY (run_id, revision)
                );
                CREATE TABLE IF NOT EXISTS artifacts (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    run_id TEXT NOT NULL REFERENCES runs(id),
                    data_json TEXT NOT NULL,
                    last_commit_id INTEGER REFERENCES commits(id)
                );
                CREATE INDEX IF NOT EXISTS idx_artifacts_run_id
                    ON artifacts (run_id);
                CREATE INDEX IF NOT EXISTS idx_artifacts_session_id
                    ON artifacts (session_id);
                CREATE TABLE IF NOT EXISTS context_receipts (
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
                    last_commit_id TEXT,
                    parent_run_id TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_context_receipts_session_run
                    ON context_receipts (session_id, run_id);
                CREATE INDEX IF NOT EXISTS idx_context_receipts_session_state
                    ON context_receipts (session_id, state);
                CREATE INDEX IF NOT EXISTS idx_context_receipts_session_kind
                    ON context_receipts (session_id, kind);
                CREATE INDEX IF NOT EXISTS idx_context_receipts_session_parent_run
                    ON context_receipts (session_id, parent_run_id);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_context_receipts_artifact_unique
                    ON context_receipts (
                        session_id,
                        run_id,
                        kind,
                        json_extract(provenance_json, '$.artifactId')
                    )
                    WHERE json_extract(provenance_json, '$.artifactId') IS NOT NULL;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_context_receipts_event_turn_unique
                    ON context_receipts (
                        session_id,
                        run_id,
                        kind,
                        json_extract(provenance_json, '$.eventSeq'),
                        json_extract(provenance_json, '$.agentTurnId')
                    )
                    WHERE json_extract(provenance_json, '$.artifactId') IS NULL
                      AND json_extract(provenance_json, '$.eventSeq') IS NOT NULL
                      AND json_extract(provenance_json, '$.agentTurnId') IS NOT NULL;
                CREATE TABLE IF NOT EXISTS work_items (
                    key TEXT PRIMARY KEY,
                    source_kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    fetched_at_ms INTEGER NOT NULL,
                    data_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_work_items_source_status
                    ON work_items (source_kind, status);
                CREATE TABLE IF NOT EXISTS work_source_cursors (
                    source_key TEXT PRIMARY KEY,
                    data_json TEXT NOT NULL
                );
                COMMIT;
                ",
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
    }

    pub(super) fn validate_current_schema(&self) -> Result<(), StoreError> {
        for (kind, name) in [
            ("table", "events"),
            ("table", "principals"),
            ("table", "sessions"),
            ("table", "workspaces"),
            ("table", "navigation_states"),
            ("table", "auth_profiles"),
            ("table", "code_host_accounts"),
            ("table", "plugin_installations"),
            ("table", "runs"),
            ("table", "scheduled_work_definitions"),
            ("table", "scheduled_work_occurrences"),
            ("table", "checkpoints"),
            ("table", "artifacts"),
            ("table", "context_receipts"),
            ("table", "commits"),
            ("table", "agent_turn_rows"),
            ("table", "thread_workspace_events"),
            ("table", "thread_workspaces"),
            ("index", "idx_events_session_sequence"),
            ("index", "idx_agent_turn_rows_session_sequence"),
            ("index", "idx_principals_credential_hash"),
            ("index", "idx_workspaces_root_realpath"),
            ("index", "idx_auth_profiles_method_order"),
            ("index", "idx_code_host_accounts_owner_provider_name"),
            ("index", "idx_sessions_workspace_id"),
            ("index", "idx_runs_session_id"),
            ("index", "idx_scheduled_work_occurrences_work_state"),
            ("index", "idx_run_projections_session_started_at"),
            ("index", "idx_events_session_run_seq"),
            ("index", "idx_artifacts_run_id"),
            ("index", "idx_artifacts_session_id"),
            ("index", "idx_context_receipts_session_run"),
            ("index", "idx_context_receipts_session_state"),
            ("index", "idx_context_receipts_session_kind"),
            ("index", "idx_context_receipts_session_parent_run"),
            ("index", "idx_context_receipts_artifact_unique"),
            ("index", "idx_context_receipts_event_turn_unique"),
            ("index", "idx_commits_session_id"),
            ("table", "work_items"),
            ("table", "work_source_cursors"),
            ("index", "idx_work_items_source_status"),
        ] {
            self.require_schema_object(kind, name)?;
        }
        self.require_table_columns("principals", &["id", "credential_hash", "data_json"])?;
        self.require_table_columns(
            "workspaces",
            &[
                "id",
                "root_realpath",
                "display_name",
                "trust_state",
                "git_repo_root",
                "created_at",
                "last_used_at",
                "data_json",
            ],
        )?;
        self.require_table_columns(
            "auth_profiles",
            &[
                "id",
                "auth_method_id",
                "provider_id",
                "sort_order",
                "is_default",
                "data_json",
            ],
        )?;
        self.require_table_columns(
            "code_host_accounts",
            &[
                "id",
                "owner_principal_id",
                "provider",
                "display_name",
                "data_json",
            ],
        )?;
        self.require_table_columns(
            "plugin_installations",
            &[
                "owner_principal_id",
                "plugin_id",
                "version",
                "digest_sha256",
                "data_json",
            ],
        )?;
        self.require_table_columns(
            "sessions",
            &["id", "data_json", "workspace_id", "last_commit_id"],
        )?;
        self.require_table_columns("runs", &["id", "session_id", "data_json", "last_commit_id"])?;
        self.require_table_columns(
            "scheduled_work_definitions",
            &["id", "session_id", "data_json"],
        )?;
        self.require_table_columns(
            "scheduled_work_occurrences",
            &["id", "scheduled_work_id", "run_id", "state", "data_json"],
        )?;
        self.require_table_columns(
            "events",
            &["sequence", "session_id", "occurred_at_ms", "payload_json"],
        )?;
        self.require_table_columns("agent_turn_rows", &["sequence", "session_id", "data_json"])?;
        self.require_table_columns(
            "thread_workspace_events",
            &["session_id", "sequence", "occurred_at_ms", "data_json"],
        )?;
        self.require_table_columns("thread_workspaces", &["session_id", "projection_json"])?;
        self.require_table_columns(
            "checkpoints",
            &["run_id", "revision", "data_json", "commit_id"],
        )?;
        self.require_table_columns(
            "artifacts",
            &["id", "session_id", "run_id", "data_json", "last_commit_id"],
        )?;
        self.require_table_columns(
            "context_receipts",
            &[
                "id",
                "session_id",
                "run_id",
                "state",
                "kind",
                "provenance_json",
                "data_json",
                "created_at_ms",
                "promoted_at_ms",
                "quarantined_at_ms",
                "last_commit_id",
                "parent_run_id",
            ],
        )?;
        self.require_table_columns(
            "commits",
            &[
                "id",
                "session_id",
                "kind",
                "occurred_at_ms",
                "first_sequence",
                "last_sequence",
            ],
        )?;
        self.require_table_columns(
            "work_items",
            &["key", "source_kind", "status", "fetched_at_ms", "data_json"],
        )?;
        self.require_table_columns("work_source_cursors", &["source_key", "data_json"])?;
        self.require_foreign_keys(
            "sessions",
            &[
                ("last_commit_id", "commits", "id"),
                ("workspace_id", "workspaces", "id"),
            ],
        )?;
        self.require_foreign_keys(
            "runs",
            &[
                ("last_commit_id", "commits", "id"),
                ("session_id", "sessions", "id"),
            ],
        )?;
        self.require_foreign_keys("events", &[("session_id", "sessions", "id")])?;
        self.require_foreign_keys("agent_turn_rows", &[("session_id", "sessions", "id")])?;
        self.require_foreign_keys(
            "thread_workspace_events",
            &[("session_id", "sessions", "id")],
        )?;
        self.require_foreign_keys("thread_workspaces", &[("session_id", "sessions", "id")])?;
        self.require_foreign_keys(
            "checkpoints",
            &[("commit_id", "commits", "id"), ("run_id", "runs", "id")],
        )?;
        self.require_foreign_keys(
            "artifacts",
            &[
                ("last_commit_id", "commits", "id"),
                ("run_id", "runs", "id"),
                ("session_id", "sessions", "id"),
            ],
        )?;
        self.require_foreign_keys("commits", &[("session_id", "sessions", "id")])?;
        self.require_foreign_keys(
            "code_host_accounts",
            &[("owner_principal_id", "principals", "id")],
        )?;
        self.require_foreign_keys(
            "plugin_installations",
            &[("owner_principal_id", "principals", "id")],
        )?;
        self.require_table_sql_contains("commits", "CHECK(first_sequence <= last_sequence)")?;
        Ok(())
    }

    fn require_schema_object(
        &self,
        kind: &'static str,
        name: &'static str,
    ) -> Result<(), StoreError> {
        let exists: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(1) FROM sqlite_master WHERE type = ? AND name = ?",
                params![kind, name],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        if exists != 1 {
            return Err(StoreError::MissingSchemaObject {
                path: self.path.clone(),
                kind,
                name,
            });
        }
        Ok(())
    }

    fn require_table_columns(
        &self,
        table: &'static str,
        expected: &[&'static str],
    ) -> Result<(), StoreError> {
        let actual = self.table_column_names(table)?;
        if actual == expected {
            return Ok(());
        }
        Err(StoreError::SchemaShapeMismatch {
            path: self.path.clone(),
            table,
            detail: format!("expected columns {:?}, got {:?}", expected, actual),
        })
    }

    fn table_column_names(&self, table: &'static str) -> Result<Vec<String>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let mut actual = Vec::new();
        for row in rows {
            actual.push(row.map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?);
        }
        Ok(actual)
    }

    fn require_foreign_keys(
        &self,
        table: &'static str,
        expected: &[(&'static str, &'static str, &'static str)],
    ) -> Result<(), StoreError> {
        let actual = self.table_foreign_keys(table)?;
        let expected = expected
            .iter()
            .map(|(from, to_table, to_column)| {
                (
                    from.to_string(),
                    to_table.to_string(),
                    to_column.to_string(),
                )
            })
            .collect::<Vec<_>>();
        if actual == expected {
            return Ok(());
        }
        Err(StoreError::SchemaShapeMismatch {
            path: self.path.clone(),
            table,
            detail: format!("expected foreign keys {:?}, got {:?}", expected, actual),
        })
    }

    fn table_foreign_keys(
        &self,
        table: &'static str,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let mut actual = Vec::new();
        for row in rows {
            actual.push(row.map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?);
        }
        actual.sort();
        Ok(actual)
    }

    fn require_table_sql_contains(
        &self,
        table: &'static str,
        needle: &'static str,
    ) -> Result<(), StoreError> {
        let sql: String = self
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?",
                [table],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        if sql.contains(needle) {
            return Ok(());
        }
        Err(StoreError::SchemaShapeMismatch {
            path: self.path.clone(),
            table,
            detail: format!("expected table sql to contain {needle:?}"),
        })
    }

    pub(super) fn verify_integrity(&self) -> Result<(), StoreError> {
        let result: String = self
            .conn
            .pragma_query_value(None, "quick_check", |row| row.get(0))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        if result == "ok" {
            return self.verify_foreign_keys();
        }
        Err(StoreError::IntegrityCheckFailed {
            path: self.path.clone(),
            result,
        })
    }

    fn verify_foreign_keys(&self) -> Result<(), StoreError> {
        let mut stmt = self
            .conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let violation = stmt
            .query_row([], |row| {
                let table = row.get::<_, String>(0)?;
                let row_id = row.get::<_, i64>(1)?;
                let parent = row.get::<_, String>(2)?;
                let fk_id = row.get::<_, i64>(3)?;
                Ok((table, row_id, parent, fk_id))
            })
            .optional()
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        if let Some((table, row_id, parent, fk_id)) = violation {
            return Err(StoreError::ForeignKeyCheckFailed {
                path: self.path.clone(),
                detail: format!("table={table} rowid={row_id} parent={parent} fk={fk_id}"),
            });
        }
        Ok(())
    }
}
