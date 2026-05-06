use super::*;

#[cfg(any(test, feature = "test-support"))]
impl StoreSeedRepository for InMemoryStore {
    fn append_event(&mut self, event: EventRecord) -> Result<(), StoreError> {
        self.append_seed_event(event)
    }

    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError> {
        self.save_seed_principal(principal)
    }

    fn save_session(&mut self, session: SessionProjection) -> Result<(), StoreError> {
        self.save_seed_session(session)
    }

    fn save_run(&mut self, run: RunProjection) -> Result<(), StoreError> {
        self.save_seed_run(run)
    }

    fn save_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), StoreError> {
        self.save_seed_artifact(artifact)
    }
}
