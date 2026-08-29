use ta_protocol::wire::{
    WorkspaceFileOpenExternalParams, WorkspaceFileOpenExternalResult, WorkspaceFileReadParams,
    WorkspaceFileReadResult, WorkspaceFileTreeParams, WorkspaceFileTreeResult,
    WorkspaceFileWriteParams, WorkspaceFileWriteResult,
};
use ta_store::PersistenceStore;

use super::{AppService, AppServiceError};
use crate::workspace::files::{
    read_workspace_file, workspace_file_tree, write_workspace_text_file,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn workspace_file_tree(
        &self,
        owner_principal_id: &str,
        params: &WorkspaceFileTreeParams,
    ) -> Result<WorkspaceFileTreeResult, AppServiceError> {
        let workspace =
            self.project_workspace(owner_principal_id, &params.project_id, &params.workspace_id)?;
        workspace_file_tree(workspace.root_realpath.as_path())
    }

    pub fn read_workspace_file(
        &self,
        owner_principal_id: &str,
        params: &WorkspaceFileReadParams,
    ) -> Result<WorkspaceFileReadResult, AppServiceError> {
        let workspace =
            self.project_workspace(owner_principal_id, &params.project_id, &params.workspace_id)?;
        let (path, _, content) = read_workspace_file(
            workspace.root_realpath.as_path(),
            &params.path,
            params.pdf_page_index,
        )?;
        Ok(WorkspaceFileReadResult { path, content })
    }

    pub fn write_workspace_file(
        &self,
        owner_principal_id: &str,
        params: &WorkspaceFileWriteParams,
    ) -> Result<WorkspaceFileWriteResult, AppServiceError> {
        if !params.user_approved {
            return Err(AppServiceError::WorkspaceFileWriteApprovalRequired);
        }
        let workspace =
            self.project_workspace(owner_principal_id, &params.project_id, &params.workspace_id)?;
        let (path, revision, byte_len) = write_workspace_text_file(
            workspace.root_realpath.as_path(),
            &params.path,
            &params.expected_revision,
            &params.text,
        )?;
        Ok(WorkspaceFileWriteResult {
            path,
            revision,
            byte_len,
        })
    }

    pub fn workspace_file_open_external(
        &self,
        owner_principal_id: &str,
        params: &WorkspaceFileOpenExternalParams,
    ) -> Result<WorkspaceFileOpenExternalResult, AppServiceError> {
        let workspace =
            self.project_workspace(owner_principal_id, &params.project_id, &params.workspace_id)?;
        let (_, canonical_path, _) =
            read_workspace_file(workspace.root_realpath.as_path(), &params.path, None)?;
        Ok(WorkspaceFileOpenExternalResult {
            path: ta_protocol::wire::WorkspacePath::new(canonical_path).map_err(|error| {
                AppServiceError::WorkspaceFileIo {
                    path: params.path.clone(),
                    action: "prepare external open".to_string(),
                    reason: error.to_string(),
                }
            })?,
        })
    }
}
