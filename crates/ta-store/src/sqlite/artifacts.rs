use super::*;

impl SqliteStore {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn save_seed_artifact(
        &mut self,
        artifact: ArtifactRecord,
    ) -> Result<(), StoreError> {
        artifact.validate_metadata()?;
        let changed = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO artifacts (id, session_id, run_id, data_json, last_commit_id)
                 VALUES (?, ?, ?, ?, NULL)",
                params![
                    artifact.id.as_str(),
                    artifact.session_id.as_str(),
                    artifact.run_id.as_str(),
                    Self::encode("artifact_record", &artifact)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact",
                source,
            })?;
        if changed == 0 {
            return Err(StoreError::DuplicateRecord {
                entity: "artifact",
                key: artifact.id.as_str().to_string(),
            });
        }
        Ok(())
    }
}

impl ArtifactRepository for SqliteStore {
    fn artifact(&self, artifact_id: &ArtifactId) -> Result<Option<ArtifactRecord>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM artifacts WHERE id = ?",
                [artifact_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?;
        json.map(|json| Self::decode("artifact_record", json))
            .transpose()
    }

    fn artifacts_for_run(&self, run_id: &RunId) -> Result<Vec<ArtifactRecord>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM artifacts WHERE run_id = ? ORDER BY id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?;
        let rows = stmt
            .query_map([run_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("artifact_record", json))
            .collect()
    }

    fn artifacts_for_session(
        &self,
        query: &SessionArtifactQuery,
    ) -> Result<Vec<ArtifactRecord>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM artifacts WHERE session_id = ? ORDER BY id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?;
        let mut artifacts = stmt
            .query_map([query.session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_record",
                source,
            })?
            .into_iter()
            .map(|json| Self::decode("artifact_record", json))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|artifact: &ArtifactRecord| {
                query
                    .run_id
                    .as_ref()
                    .is_none_or(|run_id| artifact.run_id == *run_id)
            })
            .filter(|artifact: &ArtifactRecord| {
                query
                    .artifact_id
                    .as_ref()
                    .is_none_or(|artifact_id| artifact.id == *artifact_id)
            })
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(artifacts)
    }
}
