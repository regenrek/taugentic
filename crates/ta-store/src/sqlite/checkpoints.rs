use super::*;
use ta_protocol::wire::WorkspaceId;

impl CheckpointRepository for SqliteStore {
    fn checkpoints_for_run(&self, run_id: &RunId) -> Result<Vec<CheckpointRecord>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM checkpoints WHERE run_id = ? ORDER BY revision ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        let rows = stmt
            .query_map([run_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("checkpoint_record", json))
            .collect()
    }

    fn checkpoints_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<CheckpointRecord>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM checkpoints ORDER BY commit_id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        let mut records = rows
            .into_iter()
            .map(|json| Self::decode("checkpoint_record", json))
            .collect::<Result<Vec<CheckpointRecord>, _>>()?
            .into_iter()
            .filter(|record| record.workspace_id == *workspace_id)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.created_at_ms);
        Ok(records)
    }

    fn checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRecord>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM checkpoints ORDER BY commit_id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_record",
                source,
            })?;
        for json in rows {
            let record = Self::decode::<CheckpointRecord>("checkpoint_record", json)?;
            if record.checkpoint_id == checkpoint_id {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }
}
