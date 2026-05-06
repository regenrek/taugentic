use super::*;

impl SqliteStore {
    pub(super) fn session_runs_tx(
        tx: &rusqlite::Transaction<'_>,
        session_id: &SessionId,
    ) -> Result<Vec<RunProjection>, StoreError> {
        let mut stmt = tx
            .prepare("SELECT data_json FROM runs WHERE session_id = ?")
            .map_err(|source| StoreError::QueryStore {
                entity: "session_status",
                source,
            })?;
        let runs = stmt
            .query_map([session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "session_status",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "session_status",
                source,
            })?;
        let runs = runs
            .into_iter()
            .map(|json| Self::decode("run_projection", json))
            .collect::<Result<Vec<RunProjection>, _>>()?;
        Ok(runs)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn save_seed_session(
        &mut self,
        session: SessionProjection,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO sessions (id, data_json, last_commit_id) VALUES (?, ?, NULL)
                 ON CONFLICT(id) DO UPDATE SET data_json = excluded.data_json",
                params![
                    session.id.as_str(),
                    Self::encode("session_projection", &session)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?;
        Ok(())
    }

    pub(super) fn read_session_projection(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionProjection>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM sessions WHERE id = ?",
                [session_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "session_projection",
                source,
            })?;
        json.map(|json| Self::decode("session_projection", json))
            .transpose()
    }

    pub(super) fn read_session_projections(&self) -> Result<Vec<SessionProjection>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM sessions ORDER BY id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "session_projection",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "session_projection",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "session_projection",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("session_projection", json))
            .collect()
    }
}

impl SessionAuthorityRepository for SqliteStore {
    fn rotate_session_authority(
        &mut self,
        session_id: &SessionId,
        owner_principal_id: &str,
        presented_authority_hash: &str,
        next_authority_hash: &str,
    ) -> Result<Option<SessionProjection>, StoreError> {
        let Some(existing) = self.session(session_id)? else {
            return Ok(None);
        };
        if existing.owner_principal_id != owner_principal_id {
            return Ok(None);
        }
        let matches_current = existing.current_session_authority_hash == presented_authority_hash;
        let matches_recovery = existing
            .recovery_session_authority_hash
            .as_deref()
            .is_some_and(|hash| hash == presented_authority_hash);
        if !matches_current && !matches_recovery {
            return Ok(None);
        }
        let next_generation = existing
            .current_session_authority_generation
            .saturating_add(1);
        let recovery_session_authority_hash = if matches_current {
            Some(existing.current_session_authority_hash.clone())
        } else {
            None
        };
        let recovery_session_authority_generation = if matches_current {
            Some(existing.current_session_authority_generation)
        } else {
            None
        };

        let updated = SessionProjection {
            current_session_authority_hash: next_authority_hash.to_string(),
            current_session_authority_generation: next_generation,
            recovery_session_authority_hash,
            recovery_session_authority_generation,
            ..existing
        };
        self.conn
            .execute(
                "UPDATE sessions SET data_json = ? WHERE id = ?",
                params![
                    Self::encode("session_projection", &updated)?,
                    updated.id.as_str(),
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?;
        Ok(Some(updated))
    }
}
