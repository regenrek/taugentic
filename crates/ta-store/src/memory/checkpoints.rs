use super::*;

impl CheckpointRepository for InMemoryStore {
    fn checkpoints_for_run(&self, run_id: &RunId) -> Result<Vec<CheckpointRecord>, StoreError> {
        Ok(self
            .checkpoints
            .get(run_id)
            .into_iter()
            .flat_map(|revisions| revisions.values().cloned())
            .collect())
    }

    fn checkpoints_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<CheckpointRecord>, StoreError> {
        let mut records = self
            .checkpoints
            .values()
            .flat_map(|revisions| revisions.values())
            .filter(|record| record.workspace_id == *workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.created_at_ms);
        Ok(records)
    }

    fn checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRecord>, StoreError> {
        Ok(self
            .checkpoints
            .values()
            .flat_map(|revisions| revisions.values())
            .find(|record| record.checkpoint_id == checkpoint_id)
            .cloned())
    }
}
