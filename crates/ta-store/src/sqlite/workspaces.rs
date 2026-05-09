use ta_protocol::wire::{Workspace, WorkspaceId};

use super::*;
use crate::{WorkspaceProjection, WorkspaceRepository};

impl SqliteStore {
    pub(super) fn upsert_workspace_row(
        &mut self,
        workspace: WorkspaceProjection,
    ) -> Result<WorkspaceProjection, StoreError> {
        let trust_state = Self::encode("workspace_trust_state", workspace.trust_state())?;
        let inner: &Workspace = &workspace.0;
        self.conn
            .execute(
                "INSERT INTO workspaces (
                    id,
                    root_realpath,
                    display_name,
                    trust_state,
                    git_repo_root,
                    created_at,
                    last_used_at,
                    data_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    root_realpath = excluded.root_realpath,
                    display_name = excluded.display_name,
                    trust_state = excluded.trust_state,
                    git_repo_root = excluded.git_repo_root,
                    last_used_at = excluded.last_used_at,
                    data_json = excluded.data_json",
                params![
                    inner.id.as_str(),
                    inner.root_realpath.as_str(),
                    inner.display_name.as_str(),
                    trust_state,
                    inner.git_repo_root.as_ref().map(|path| path.as_str()),
                    inner.created_at.as_str(),
                    inner.last_used_at.as_str(),
                    Self::encode("workspace_projection", &workspace)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        Ok(workspace)
    }

    pub(super) fn read_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM workspaces WHERE id = ?",
                [workspace_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        json.map(|json| Self::decode("workspace_projection", json))
            .transpose()
    }

    pub(super) fn read_workspace_by_root(
        &self,
        root_realpath: &str,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM workspaces WHERE root_realpath = ?",
                [root_realpath],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        json.map(|json| Self::decode("workspace_projection", json))
            .transpose()
    }

    pub(super) fn read_workspaces(&self) -> Result<Vec<WorkspaceProjection>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM workspaces ORDER BY last_used_at DESC, id ASC")
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        rows.into_iter()
            .map(|json| Self::decode("workspace_projection", json))
            .collect()
    }
}

impl WorkspaceRepository for SqliteStore {
    fn upsert_workspace(
        &mut self,
        workspace: WorkspaceProjection,
    ) -> Result<WorkspaceProjection, StoreError> {
        self.upsert_workspace_row(workspace)
    }

    fn workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        self.read_workspace(workspace_id)
    }

    fn workspace_by_root_realpath(
        &self,
        root_realpath: &str,
    ) -> Result<Option<WorkspaceProjection>, StoreError> {
        self.read_workspace_by_root(root_realpath)
    }

    fn workspaces(&self) -> Result<Vec<WorkspaceProjection>, StoreError> {
        self.read_workspaces()
    }
}
