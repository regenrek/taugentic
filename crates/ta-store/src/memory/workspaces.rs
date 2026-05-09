use super::*;
use crate::{WorkspaceProjection, WorkspaceRepository};
use ta_protocol::wire::WorkspaceId;

impl WorkspaceRepository for InMemoryStore {
    fn upsert_workspace(
        &mut self,
        workspace: WorkspaceProjection,
    ) -> Result<WorkspaceProjection, StoreError> {
        if let Some(existing) = self
            .workspaces
            .values()
            .find(|candidate| {
                candidate.root_realpath().as_str() == workspace.root_realpath().as_str()
                    && candidate.id() != workspace.id()
            })
            .map(|candidate| candidate.id().as_str().to_string())
        {
            return Err(StoreError::DuplicateRecord {
                entity: "workspace_root_realpath",
                key: existing,
            });
        }
        self.workspaces
            .insert(workspace.id().clone(), workspace.clone());
        Ok(workspace)
    }

    fn workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        Ok(self.workspaces.get(workspace_id).cloned())
    }

    fn workspace_by_root_realpath(
        &self,
        root_realpath: &str,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        Ok(self
            .workspaces
            .values()
            .find(|candidate| candidate.root_realpath().as_str() == root_realpath)
            .cloned())
    }

    fn workspaces(&self) -> Result<Vec<WorkspaceProjection>, StoreError> {
        Ok(self.workspaces.values().cloned().collect())
    }
}
