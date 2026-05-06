use super::*;

impl ArtifactRepository for InMemoryStore {
    fn artifact(&self, artifact_id: &ArtifactId) -> Result<Option<ArtifactRecord>, StoreError> {
        Ok(self.artifacts.get(artifact_id).cloned())
    }

    fn artifacts_for_run(&self, run_id: &RunId) -> Result<Vec<ArtifactRecord>, StoreError> {
        Ok(self
            .artifacts
            .values()
            .filter(|artifact| artifact.run_id == *run_id)
            .cloned()
            .collect())
    }

    fn artifacts_for_session(
        &self,
        query: &SessionArtifactQuery,
    ) -> Result<Vec<ArtifactRecord>, StoreError> {
        let mut artifacts = self
            .artifacts
            .values()
            .filter(|artifact| artifact.session_id == query.session_id)
            .filter(|artifact| {
                query
                    .run_id
                    .as_ref()
                    .is_none_or(|run_id| artifact.run_id == *run_id)
            })
            .filter(|artifact| {
                query
                    .artifact_id
                    .as_ref()
                    .is_none_or(|artifact_id| artifact.id == *artifact_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(artifacts)
    }
}
