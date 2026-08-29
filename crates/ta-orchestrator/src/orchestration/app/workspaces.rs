use std::{
    fs::{self, OpenOptions},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use ta_protocol::wire::{
    NavigationProject, NavigationSnapshot, ProjectId, TrustState, Workspace, WorkspaceId,
    WorkspacePath,
};
use ta_store::{PersistenceStore, WorkspaceProjection};
use uuid::Uuid;

use super::{
    AppService, AppServiceError, OpenWorkspaceRequest, sanitize_session_owner_principal_id,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn open_workspace(
        &self,
        request: &OpenWorkspaceRequest,
    ) -> Result<Workspace, AppServiceError> {
        let input_path = request.path.as_path();
        if !input_path.exists() {
            return Err(AppServiceError::WorkspaceNotFound(display_path(input_path)));
        }

        let root_realpath = WorkspacePath::canonicalize_existing(input_path).map_err(|error| {
            AppServiceError::WorkspaceCanonicalizeFailed {
                path: display_path(input_path),
                reason: error.to_string(),
            }
        })?;

        if !root_realpath.as_path().is_dir() {
            return Err(AppServiceError::WorkspaceNotADirectory(
                root_realpath.as_str().to_string(),
            ));
        }

        probe_workspace_permissions(root_realpath.as_path())?;
        let git_repo_root = detect_git_repo_root(root_realpath.as_path())?;
        let now = current_time_marker();

        let mut store = self.store.lock().expect("app store should not be poisoned");
        let existing = store.workspace_by_root_realpath(root_realpath.as_str())?;

        if !request.trust_acknowledged
            && !existing.as_ref().is_some_and(|workspace| {
                matches!(workspace.trust_state(), TrustState::UserConfirmed { .. })
            })
        {
            return Err(AppServiceError::WorkspaceTrustRequired(
                root_realpath.as_str().to_string(),
            ));
        }

        let workspace = match existing {
            Some(existing) => {
                let mut workspace = existing.into_inner();
                workspace.last_used_at = now.clone();
                if request.trust_acknowledged {
                    workspace.trust_state = TrustState::UserConfirmed {
                        confirmed_at: now.clone(),
                    };
                }
                workspace.git_repo_root = git_repo_root;
                workspace
            }
            None => Workspace {
                id: WorkspaceId::new(format!("workspace-{}", Uuid::new_v4().simple()))
                    .expect("generated workspace id should be valid"),
                root_realpath,
                display_name: workspace_display_name(input_path),
                trust_state: TrustState::UserConfirmed {
                    confirmed_at: now.clone(),
                },
                git_repo_root,
                created_at: now.clone(),
                last_used_at: now,
            },
        };

        Ok(store
            .upsert_workspace(WorkspaceProjection::new(workspace))?
            .into_inner())
    }

    pub fn open_project(
        &self,
        owner_principal_id: &str,
        path: WorkspacePath,
        trust_acknowledged: bool,
    ) -> Result<(ProjectId, NavigationSnapshot), AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let workspace = self.open_workspace(&OpenWorkspaceRequest {
            path,
            trust_acknowledged,
        })?;
        let workspace_id = workspace.id.clone();

        let mut store = self.store.lock().expect("app store should not be poisoned");
        let mut state = store.navigation_state(&owner_principal_id)?;
        let project_id = state
            .projects
            .iter()
            .filter(|project| project.workspace_ids.contains(&workspace_id))
            .map(|project| project.id.clone())
            .min_by(|left, right| left.as_str().cmp(right.as_str()))
            .unwrap_or_else(|| {
                let project_id = ProjectId::new(format!("project-{}", Uuid::new_v4().simple()))
                    .expect("generated project id should be valid");
                state.projects.push(NavigationProject {
                    id: project_id.clone(),
                    space_id: None,
                    title: workspace.display_name.clone(),
                    workspace_ids: vec![workspace_id.clone()],
                });
                project_id
            });

        for project in &mut state.projects {
            if project.id == project_id {
                if !project.workspace_ids.contains(&workspace_id) {
                    project.workspace_ids.push(workspace_id.clone());
                }
            } else {
                project.workspace_ids.retain(|id| id != &workspace_id);
            }
        }
        store.save_navigation_state(&owner_principal_id, state)?;
        drop(store);

        let snapshot = self.navigation_snapshot(&owner_principal_id, None)?;
        Ok((project_id, snapshot))
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store
            .workspaces()?
            .into_iter()
            .map(WorkspaceProjection::into_inner)
            .collect())
    }

    pub fn get_workspace(&self, workspace_id: &WorkspaceId) -> Result<Workspace, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        store
            .workspace(workspace_id)?
            .map(WorkspaceProjection::into_inner)
            .ok_or_else(|| AppServiceError::WorkspaceNotFound(workspace_id.as_str().to_string()))
    }

    pub(super) fn project_workspace(
        &self,
        owner_principal_id: &str,
        project_id: &ProjectId,
        workspace_id: &WorkspaceId,
    ) -> Result<Workspace, AppServiceError> {
        let owner_principal_id = sanitize_session_owner_principal_id(owner_principal_id)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        let navigation = store.navigation_state(&owner_principal_id)?;
        let project = navigation
            .projects
            .iter()
            .find(|project| project.id == *project_id)
            .ok_or_else(|| AppServiceError::ProjectNotFound(project_id.as_str().to_string()))?;
        if !project.workspace_ids.contains(workspace_id) {
            return Err(AppServiceError::WorkspaceNotFound(
                workspace_id.as_str().to_string(),
            ));
        }
        let workspace = store
            .workspace(workspace_id)?
            .map(WorkspaceProjection::into_inner)
            .ok_or_else(|| AppServiceError::WorkspaceNotFound(workspace_id.as_str().to_string()))?;
        if !matches!(workspace.trust_state, TrustState::UserConfirmed { .. }) {
            return Err(AppServiceError::WorkspaceTrustRequired(
                workspace_id.as_str().to_string(),
            ));
        }
        Ok(workspace)
    }
}

fn probe_workspace_permissions(root: &Path) -> Result<(), AppServiceError> {
    fs::read_dir(root).map_err(|error| AppServiceError::WorkspacePermissionDenied {
        path: display_path(root),
        reason: error.to_string(),
    })?;

    let probe_path = root.join(format!(
        ".taugentic-permission-probe-{}",
        Uuid::new_v4().simple()
    ));
    let create_result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path);

    match create_result {
        Ok(_) => fs::remove_file(&probe_path).map_err(|error| {
            AppServiceError::WorkspacePermissionDenied {
                path: display_path(&probe_path),
                reason: error.to_string(),
            }
        }),
        Err(error) => Err(AppServiceError::WorkspacePermissionDenied {
            path: display_path(root),
            reason: error.to_string(),
        }),
    }
}

fn detect_git_repo_root(root: &Path) -> Result<Option<WorkspacePath>, AppServiceError> {
    for ancestor in root.ancestors() {
        if ancestor.join(".git").exists() {
            return WorkspacePath::from_canonical_wire_value(display_path(ancestor))
                .map(Some)
                .map_err(|error| AppServiceError::WorkspaceCanonicalizeFailed {
                    path: display_path(ancestor),
                    reason: error.to_string(),
                });
        }
    }
    Ok(None)
}

fn workspace_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

fn current_time_marker() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_millis();
    format!("unix-ms:{millis}")
}
