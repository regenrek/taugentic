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
}
