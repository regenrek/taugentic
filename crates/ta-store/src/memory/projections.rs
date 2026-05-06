use super::*;

impl ProjectionRepository for InMemoryStore {
    fn session(&self, session_id: &SessionId) -> Result<Option<SessionProjection>, StoreError> {
        Ok(self.sessions.get(session_id).cloned())
    }

    fn sessions(&self) -> Result<Vec<SessionProjection>, StoreError> {
        Ok(self.sessions.values().cloned().collect())
    }

    fn run(&self, run_id: &RunId) -> Result<Option<RunProjection>, StoreError> {
        Ok(self.runs.get(run_id).cloned())
    }

    fn runs(&self) -> Result<Vec<RunProjection>, StoreError> {
        Ok(self.runs.values().cloned().collect())
    }

    fn list_native_runs(
        &self,
        query: &NativeRunListQuery,
    ) -> Result<NativeRunListPage, StoreError> {
        Ok(list_native_runs_from_projections(
            self.runs.values().cloned(),
            query,
        ))
    }
}
