use ta_protocol::wire::{GitCheckpointPhase, RunId, RunSource};
use ta_store::{CheckpointRecord, CommitCheckpointPersist, PersistenceStore};

use super::{RunExecutionError, RunExecutionService, current_time_ms};
use crate::workspace::git::{GitRepository, GitRepositoryError};

pub(super) struct PreparedAfterUserTurnCheckpoint {
    repository: GitRepository,
    checkpoint: CheckpointRecord,
}

impl PreparedAfterUserTurnCheckpoint {
    pub(super) fn cleanup_unpersisted(self) -> Result<(), RunExecutionError> {
        self.repository
            .delete_checkpoint_ref(&self.checkpoint.checkpoint_id)
            .map_err(|error| RunExecutionError::CheckpointFailed(error.to_string()))
    }
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn capture_before_user_turn(&self, run_id: &RunId) -> Result<(), RunExecutionError> {
        let Some(prepared) =
            self.prepare_user_turn_checkpoint(run_id, GitCheckpointPhase::BeforeTurn, 0)?
        else {
            return Ok(());
        };
        let persisted = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: prepared.checkpoint.clone(),
                occurred_at_ms: prepared.checkpoint.created_at_ms,
            });
        if let Err(error) = persisted {
            prepared.cleanup_unpersisted().map_err(|cleanup| {
                RunExecutionError::CheckpointFailed(format!(
                    "checkpoint persistence failed ({error}); ref cleanup failed ({cleanup})"
                ))
            })?;
            return Err(error.into());
        }
        Ok(())
    }

    pub(super) fn prepare_after_user_turn_checkpoint(
        &self,
        run_id: &RunId,
    ) -> Result<Option<PreparedAfterUserTurnCheckpoint>, RunExecutionError> {
        let has_before = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .checkpoints_for_run(run_id)?
                .iter()
                .any(|checkpoint| checkpoint.phase == GitCheckpointPhase::BeforeTurn)
        };
        if !has_before {
            return Ok(None);
        }
        self.prepare_user_turn_checkpoint(run_id, GitCheckpointPhase::AfterTurn, 1)
    }

    fn prepare_user_turn_checkpoint(
        &self,
        run_id: &RunId,
        phase: GitCheckpointPhase,
        revision: u64,
    ) -> Result<Option<PreparedAfterUserTurnCheckpoint>, RunExecutionError> {
        let run = self.load_run_projection(run_id)?;
        if !matches!(run.source, RunSource::User { .. }) {
            return Ok(None);
        }
        {
            let store = self.store.lock().expect("app store should not be poisoned");
            if store
                .checkpoints_for_run(run_id)?
                .iter()
                .any(|checkpoint| checkpoint.revision == revision)
            {
                return Ok(None);
            }
        }
        let repository = match GitRepository::open(run.execution_context.effective_cwd.as_path()) {
            Ok(repository) => repository,
            Err(GitRepositoryError::NotARepository) if phase == GitCheckpointPhase::BeforeTurn => {
                return Ok(None);
            }
            Err(GitRepositoryError::NotARepository) => {
                return Err(RunExecutionError::CheckpointFailed(
                    "the workspace is no longer a Git repository".to_string(),
                ));
            }
            Err(error) => return Err(RunExecutionError::CheckpointFailed(error.to_string())),
        };
        let phase_label = match phase {
            GitCheckpointPhase::BeforeTurn => "before",
            GitCheckpointPhase::AfterTurn => "after",
        };
        let checkpoint_id = format!("checkpoint-{}-{phase_label}", run.id.as_str());
        let captured = repository
            .capture_checkpoint(&checkpoint_id)
            .map_err(|error| RunExecutionError::CheckpointFailed(error.to_string()))?;
        let created_at_ms = current_time_ms();
        let checkpoint = CheckpointRecord {
            checkpoint_id: checkpoint_id.clone(),
            workspace_id: run.execution_context.workspace_id,
            run_id: run.id,
            revision,
            phase,
            base_head: captured.base_head,
            staged_commit: captured.staged_commit,
            full_commit: captured.full_commit,
            fingerprint: captured.fingerprint,
            created_at_ms,
        };
        Ok(Some(PreparedAfterUserTurnCheckpoint {
            repository,
            checkpoint,
        }))
    }

    pub(super) fn persist_prepared_after_user_turn_checkpoint(
        &self,
        store: &mut S,
        prepared: &PreparedAfterUserTurnCheckpoint,
    ) -> Result<(), RunExecutionError> {
        store
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: prepared.checkpoint.clone(),
                occurred_at_ms: prepared.checkpoint.created_at_ms,
            })
            .map(|_| ())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use ta_store::CheckpointRepository;
    use taugentic_agent::ExecutionSink;

    #[test]
    fn user_turn_checkpoints_capture_exact_before_and_after_git_state_once() {
        let repository_root = init_dispatch_repo();
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        set_default_test_workspace_root(&app, repository_root.path());
        let session = open_session(&app, "Checkpoint lifecycle");
        let run = ensure_running_run(
            &app,
            &execution,
            &session.id,
            "Capture a complete user turn",
        );

        execution
            .capture_before_user_turn(&run.id)
            .expect("before checkpoint should capture");
        execution
            .capture_before_user_turn(&run.id)
            .expect("duplicate before checkpoint should be idempotent");
        std::fs::write(
            repository_root.path().join("src/lib.rs"),
            "pub fn fixture() { println!(\"changed\"); }\n",
        )
        .expect("turn change");
        std::fs::write(repository_root.path().join("new.txt"), "new turn file\n")
            .expect("turn untracked file");
        provider_sink(&execution, &session.id, &run.id)
            .complete("turn completed")
            .expect("after checkpoint should persist with terminal completion");

        let checkpoints = execution
            .store
            .lock()
            .expect("checkpoint store")
            .checkpoints_for_run(&run.id)
            .expect("checkpoints should load");
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].revision, 0);
        assert_eq!(checkpoints[0].phase, GitCheckpointPhase::BeforeTurn);
        assert_eq!(checkpoints[1].revision, 1);
        assert_eq!(checkpoints[1].phase, GitCheckpointPhase::AfterTurn);

        let repository = GitRepository::open(repository_root.path()).expect("git repository");
        let patch = repository
            .patch_between(&checkpoints[0].full_commit, &checkpoints[1].full_commit)
            .expect("turn diff should render");
        assert!(patch.patch.contains("println!"));
        assert!(patch.patch.contains("new.txt"));
    }
}
