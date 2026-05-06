use super::*;

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
}
