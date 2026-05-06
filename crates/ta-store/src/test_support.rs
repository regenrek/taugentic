use crate::{
    ArtifactRecord, EventRecord, PrincipalProjection, RunProjection, SessionProjection, StoreError,
};

pub trait StoreSeedRepository {
    fn append_event(&mut self, event: EventRecord) -> Result<(), StoreError>;
    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError>;
    fn save_session(&mut self, session: SessionProjection) -> Result<(), StoreError>;
    fn save_run(&mut self, run: RunProjection) -> Result<(), StoreError>;
    fn save_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), StoreError>;
}
