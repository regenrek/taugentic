use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use ta_protocol::wire::{
    GIT_COMMIT_MESSAGE_MAX_BYTES, GitCheckpointApplyRevertParams, GitCheckpointListParams,
    GitCheckpointListResult, GitCheckpointPrepareRevertParams, GitCheckpointPrepareRevertResult,
    GitCheckpointSummary, GitCommitParams, GitDiffParams, GitDiffResult, GitDiffScope,
    GitMutationResult, GitPathsMutationParams, GitRepositorySnapshotParams,
    GitRepositorySnapshotResult, RunStatus,
};
use ta_store::{CheckpointRecord, PersistenceStore};
use uuid::Uuid;

use super::{AppService, AppServiceError};
use crate::workspace::git::{GitPatch, GitRepository, GitRepositoryError};

const REVERT_TOKEN_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Clone, Default)]
pub(crate) struct GitRevertRuntime {
    prepared: Arc<Mutex<BTreeMap<String, PreparedRevert>>>,
}

#[derive(Clone)]
struct PreparedRevert {
    owner_principal_id: String,
    project_id: ta_protocol::wire::ProjectId,
    workspace_id: ta_protocol::wire::WorkspaceId,
    repository_root: PathBuf,
    checkpoint: CheckpointRecord,
    expected_fingerprint: String,
    expires_at_ms: u64,
}

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn git_repository_snapshot(
        &self,
        owner_principal_id: &str,
        params: &GitRepositorySnapshotParams,
    ) -> Result<GitRepositorySnapshotResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        Ok(GitRepositorySnapshotResult {
            snapshot: repository.snapshot().map_err(map_git_error)?,
        })
    }

    pub fn git_diff(
        &self,
        owner_principal_id: &str,
        params: &GitDiffParams,
    ) -> Result<GitDiffResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let patch = match &params.scope {
            GitDiffScope::Staged => repository.staged_patch().map_err(map_git_error)?,
            GitDiffScope::Unstaged => repository.unstaged_patch().map_err(map_git_error)?,
            GitDiffScope::LastTurn => {
                let (before, after) = self.last_turn_checkpoint_pair(&params.workspace_id)?;
                repository
                    .patch_between(&before.full_commit, &after.full_commit)
                    .map_err(map_git_error)?
            }
            GitDiffScope::Checkpoint { checkpoint_id } => {
                let checkpoint = self.workspace_checkpoint(&params.workspace_id, checkpoint_id)?;
                let current = repository.capture_objects().map_err(map_git_error)?;
                repository
                    .patch_between(&checkpoint.full_commit, &current.full_commit)
                    .map_err(map_git_error)?
            }
        };
        let fingerprint = repository.current_fingerprint().map_err(map_git_error)?;
        Ok(diff_result(patch, fingerprint))
    }

    pub fn git_stage_paths(
        &self,
        owner_principal_id: &str,
        params: &GitPathsMutationParams,
    ) -> Result<GitMutationResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        repository
            .stage_paths(&params.paths)
            .map_err(map_git_error)?;
        Ok(GitMutationResult {
            snapshot: repository.snapshot().map_err(map_git_error)?,
            commit: None,
        })
    }

    pub fn git_unstage_paths(
        &self,
        owner_principal_id: &str,
        params: &GitPathsMutationParams,
    ) -> Result<GitMutationResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        repository
            .unstage_paths(&params.paths)
            .map_err(map_git_error)?;
        Ok(GitMutationResult {
            snapshot: repository.snapshot().map_err(map_git_error)?,
            commit: None,
        })
    }

    pub fn git_commit(
        &self,
        owner_principal_id: &str,
        params: &GitCommitParams,
    ) -> Result<GitMutationResult, AppServiceError> {
        if params.message.trim().is_empty()
            || params.message.as_bytes().len() > GIT_COMMIT_MESSAGE_MAX_BYTES
        {
            return Err(AppServiceError::GitCommitMessageInvalid);
        }
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let commit = repository.commit(&params.message).map_err(map_git_error)?;
        Ok(GitMutationResult {
            snapshot: repository.snapshot().map_err(map_git_error)?,
            commit: Some(commit),
        })
    }

    pub fn git_checkpoints(
        &self,
        owner_principal_id: &str,
        params: &GitCheckpointListParams,
    ) -> Result<GitCheckpointListResult, AppServiceError> {
        self.project_git_repository(owner_principal_id, &params.project_id, &params.workspace_id)?;
        let store = self.store.lock().expect("app store should not be poisoned");
        let checkpoints = store
            .checkpoints_for_workspace(&params.workspace_id)?
            .into_iter()
            .map(checkpoint_summary)
            .collect();
        Ok(GitCheckpointListResult { checkpoints })
    }

    pub fn git_prepare_checkpoint_revert(
        &self,
        owner_principal_id: &str,
        params: &GitCheckpointPrepareRevertParams,
    ) -> Result<GitCheckpointPrepareRevertResult, AppServiceError> {
        let repository = self.project_git_repository(
            owner_principal_id,
            &params.project_id,
            &params.workspace_id,
        )?;
        let checkpoint = self.workspace_checkpoint(&params.workspace_id, &params.checkpoint_id)?;
        let current = repository.capture_objects().map_err(map_git_error)?;
        let patch = repository
            .patch_between(&current.full_commit, &checkpoint.full_commit)
            .map_err(map_git_error)?;
        if patch.truncated {
            return Err(AppServiceError::GitOutputTooLarge);
        }
        let token = format!("git-revert-{}", Uuid::new_v4().simple());
        let prepared = PreparedRevert {
            owner_principal_id: owner_principal_id.to_string(),
            project_id: params.project_id.clone(),
            workspace_id: params.workspace_id.clone(),
            repository_root: repository.root().to_path_buf(),
            checkpoint: checkpoint.clone(),
            expected_fingerprint: current.fingerprint.clone(),
            expires_at_ms: current_time_ms().saturating_add(REVERT_TOKEN_TTL_MS),
        };
        self.git_reverts
            .prepared
            .lock()
            .expect("git revert runtime should not be poisoned")
            .insert(token.clone(), prepared);
        Ok(GitCheckpointPrepareRevertResult {
            token,
            patch: patch.patch,
            checkpoint: checkpoint_summary(checkpoint),
            current_fingerprint: current.fingerprint,
        })
    }

    pub fn git_apply_checkpoint_revert(
        &self,
        owner_principal_id: &str,
        params: &GitCheckpointApplyRevertParams,
    ) -> Result<GitMutationResult, AppServiceError> {
        let prepared = self
            .git_reverts
            .prepared
            .lock()
            .expect("git revert runtime should not be poisoned")
            .remove(&params.token)
            .ok_or(AppServiceError::GitRevertTokenInvalid)?;
        if prepared.owner_principal_id != owner_principal_id
            || current_time_ms() > prepared.expires_at_ms
        {
            return Err(AppServiceError::GitRevertTokenInvalid);
        }
        self.project_git_repository(
            owner_principal_id,
            &prepared.project_id,
            &prepared.workspace_id,
        )?;
        if self.workspace_has_active_run(&prepared.workspace_id)? {
            return Err(AppServiceError::GitWorkspaceRunActive);
        }
        let repository = GitRepository::open(&prepared.repository_root).map_err(map_git_error)?;
        if repository.current_fingerprint().map_err(map_git_error)? != prepared.expected_fingerprint
        {
            return Err(AppServiceError::GitRevertStateChanged);
        }
        let snapshot = repository
            .restore_checkpoint(
                &prepared.checkpoint.staged_commit,
                &prepared.checkpoint.full_commit,
            )
            .map_err(map_git_error)?;
        Ok(GitMutationResult {
            snapshot,
            commit: None,
        })
    }

    pub(super) fn project_git_repository(
        &self,
        owner_principal_id: &str,
        project_id: &ta_protocol::wire::ProjectId,
        workspace_id: &ta_protocol::wire::WorkspaceId,
    ) -> Result<GitRepository, AppServiceError> {
        let workspace = self.project_workspace(owner_principal_id, project_id, workspace_id)?;
        let root = workspace
            .git_repo_root
            .ok_or(AppServiceError::GitRepositoryRequired)?;
        GitRepository::open(root.as_path()).map_err(map_git_error)
    }

    fn workspace_checkpoint(
        &self,
        workspace_id: &ta_protocol::wire::WorkspaceId,
        checkpoint_id: &str,
    ) -> Result<CheckpointRecord, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        store
            .checkpoint(checkpoint_id)?
            .filter(|checkpoint| checkpoint.workspace_id == *workspace_id)
            .ok_or_else(|| AppServiceError::GitCheckpointNotFound(checkpoint_id.to_string()))
    }

    fn last_turn_checkpoint_pair(
        &self,
        workspace_id: &ta_protocol::wire::WorkspaceId,
    ) -> Result<(CheckpointRecord, CheckpointRecord), AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let checkpoints = store.checkpoints_for_workspace(workspace_id)?;
        for after in checkpoints.iter().rev().filter(|checkpoint| {
            matches!(
                checkpoint.phase,
                ta_protocol::wire::GitCheckpointPhase::AfterTurn
            )
        }) {
            if let Some(before) = checkpoints.iter().find(|candidate| {
                candidate.run_id == after.run_id
                    && matches!(
                        candidate.phase,
                        ta_protocol::wire::GitCheckpointPhase::BeforeTurn
                    )
            }) {
                return Ok((before.clone(), after.clone()));
            }
        }
        Err(AppServiceError::GitLastTurnUnavailable)
    }

    pub(super) fn workspace_has_active_run(
        &self,
        workspace_id: &ta_protocol::wire::WorkspaceId,
    ) -> Result<bool, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        Ok(store.runs()?.into_iter().any(|run| {
            run.execution_context.workspace_id == *workspace_id
                && matches!(
                    run.status,
                    RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval
                )
        }))
    }
}

fn diff_result(patch: GitPatch, fingerprint: String) -> GitDiffResult {
    GitDiffResult {
        patch: patch.patch,
        truncated: patch.truncated,
        fingerprint,
    }
}

fn checkpoint_summary(checkpoint: CheckpointRecord) -> GitCheckpointSummary {
    GitCheckpointSummary {
        checkpoint_id: checkpoint.checkpoint_id,
        workspace_id: checkpoint.workspace_id,
        run_id: checkpoint.run_id,
        revision: checkpoint.revision,
        phase: checkpoint.phase,
        created_at_ms: checkpoint.created_at_ms,
    }
}

pub(super) fn map_git_error(error: GitRepositoryError) -> AppServiceError {
    match error {
        GitRepositoryError::NotARepository => AppServiceError::GitRepositoryRequired,
        GitRepositoryError::InvalidPath(path) => AppServiceError::GitPathInvalid(path),
        GitRepositoryError::OutputTooLarge | GitRepositoryError::TooManyStatusEntries(_) => {
            AppServiceError::GitOutputTooLarge
        }
        GitRepositoryError::CommandFailed { context, .. } => {
            AppServiceError::GitOperationFailed(context)
        }
        GitRepositoryError::Io(_) => {
            AppServiceError::GitOperationFailed("git process unavailable".to_string())
        }
        error => AppServiceError::GitOperationFailed(error.to_string()),
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_millis() as u64
}
