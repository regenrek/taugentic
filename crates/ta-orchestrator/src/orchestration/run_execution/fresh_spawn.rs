use ta_policy::{Operation, evaluate_execution_context};
use ta_protocol::wire::{
    ApprovalScope, JoinRunRequest, JoinRunResult, RunSource, RunStatus, SpawnRunRequest,
    SpawnRunResult,
};
use ta_store::{
    CommitRunTransition, PersistenceStore, ReceiptListQuery, RunProjection, UserTurnCommit,
};
use uuid::Uuid;

use super::*;
use crate::{DelegateRecipeResolutionRequest, resolve_delegate_recipe};

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn spawn_run(
        &self,
        session_id: crate::SessionId,
        request: SpawnRunRequest,
    ) -> Result<SpawnRunResult, RunExecutionError> {
        if request.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.session_id.as_str().to_string(),
            ));
        }
        let resolved = resolve_delegate_recipe(
            &self.recipe_registry,
            DelegateRecipeResolutionRequest {
                objective: request.objective,
                output_contract: request.output_contract,
                model_id: request.selection.model_id.clone(),
                recipe_id: request.recipe_id,
            },
        )
        .map_err(map_recipe_resolution_error)?;
        let objective = resolved.objective.trim();
        if objective.is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
        }
        let parent = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let parent = store.run(&request.parent_run_id)?.ok_or_else(|| {
                RunExecutionError::RunNotFound(request.parent_run_id.as_str().to_string())
            })?;
            if parent.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    parent.id.as_str().to_string(),
                ));
            }
            parent
        };
        let selection = self
            .agent_runtime
            .validate_agent_run_selection(&request.selection)
            .map_err(map_agent_runtime_error)?;
        let runtime_profile = selection.runtime_profile().clone();
        let route = selection.route().clone();
        let execution_harness = selection.execution_harness();
        let run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start_with_policy(
                &session_id,
                run_id.clone(),
                crate::RunSchedulingPolicy::ParallelIfBusy,
            )
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error| {
            self.runtime
                .finish_scheduled_run(&session_id, &run_id, RunStatus::Failed);
            error
        };
        let prepared_context = self
            .prepare_child_execution_context(
                &session_id,
                &run_id,
                &parent.execution_context,
                ExecutionContextRequest {
                    workspace_mode: request.workspace_scope,
                    cleanup_policy: request.cleanup_policy,
                    planned_write_files: request.planned_write_files.clone(),
                },
            )
            .map_err(fail_scheduled_run)?;
        let decision = evaluate_execution_context(
            &prepared_context.execution_context,
            &Operation::new(ApprovalScope::ProcessExec, "execute fresh spawned run"),
        );
        let user_turn = UserTurnCommit::Append {
            text: objective.to_string(),
            attachments: Vec::new(),
        };
        let (mut run, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, events) = match disposition {
                crate::RunScheduleDisposition::StartNow => {
                    build_start_transition(run_id.clone(), decision, resolved.recipe_id.clone())
                }
                crate::RunScheduleDisposition::Queued { position } => {
                    build_queue_transition(run_id.clone(), position, resolved.recipe_id.clone())
                }
            };
            let run = RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective: objective.to_string(),
                status,
                harness: run_harness_kind(execution_harness),
                source: RunSource::FreshSpawn {
                    route: route.clone(),
                    parent_run_id: parent.id.clone(),
                    output_contract: resolved.output_contract,
                    model_id: resolved.model_id.clone(),
                    recipe_id: resolved.recipe_id.clone(),
                    workspace_scope: request.workspace_scope,
                    cleanup_policy: request.cleanup_policy,
                    planned_write_files: request.planned_write_files,
                },
                execution_context: prepared_context.execution_context,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: prepared_context.workspace_info,
                claimed_files: prepared_context.claimed_files,
                conflict_summary: prepared_context.conflict_summary,
            };
            let committed = store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run,
                    user_turn,
                    events,
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .map_err(|error| fail_scheduled_run(error.into()))?;
            (committed.run, committed.events)
        };
        self.publish_records(&events);
        if run.status == RunStatus::Running {
            let generation = self
                .runtime
                .claim_live_run(run.id.clone(), session_id.clone());
            let start_result = self.start_provider_execution(
                &session_id,
                &run.id,
                &runtime_profile,
                &route,
                generation,
            );
            let latest = self.load_run_projection(&run.id)?;
            if let Err(error) = start_result
                && latest.status == RunStatus::Running
            {
                self.fail_live_run_and_publish_for_generation(
                    session_id.clone(),
                    &latest.id,
                    error.to_string(),
                    generation,
                )?;
                run = self.load_run_projection(&latest.id)?;
            } else if latest.status != RunStatus::Cancelled {
                run = latest;
            }
        }
        Ok(SpawnRunResult {
            run: project_run_record(run),
        })
    }

    pub fn join_run(
        &self,
        session_id: crate::SessionId,
        request: JoinRunRequest,
    ) -> Result<JoinRunResult, RunExecutionError> {
        if request.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.session_id.as_str().to_string(),
            ));
        }
        let store = self.store.lock().expect("app store should not be poisoned");
        let run = store.run(&request.child_run_id)?.ok_or_else(|| {
            RunExecutionError::RunNotFound(request.child_run_id.as_str().to_string())
        })?;
        if run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        if !matches!(
            &run.source,
            RunSource::FreshSpawn { parent_run_id, .. } if parent_run_id == &request.parent_run_id
        ) {
            return Err(RunExecutionError::RunNotFound(
                request.child_run_id.as_str().to_string(),
            ));
        }
        let receipts = store.list(&ReceiptListQuery {
            session_id: session_id.clone(),
            run_id: Some(run.id.clone()),
            state: None,
            kind: None,
            parent_run_id: None,
            limit: None,
        })?;
        let artifacts = store
            .artifacts_for_run(&run.id)?
            .into_iter()
            .map(|artifact| ta_store::project_artifact_summary(&artifact))
            .collect();
        Ok(JoinRunResult {
            result: run.result.clone(),
            run: project_run_record(run),
            receipts,
            artifacts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::{
        app_and_execution_with_runtime, approval_actor, attach_noop_handle,
        ensure_running_run_with_profile, open_session,
    };
    use ta_protocol::wire::{
        AgentTurnRow, ArtifactId, ArtifactKind, CapsuleResult, DaemonEvent, DebugResult, RunEvent,
        RunHarnessKind, WorkspaceMode, WorktreeCleanupPolicy,
    };
    use ta_store::{
        ArtifactRecord, CommitRepository, CommitRunTransition, EventLogRepository,
        ProjectionRepository, SessionAgentTurnsPageQuery,
    };

    #[test]
    fn fresh_spawn_uses_selected_generic_harness_with_empty_child_history() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Fresh generic selection");
        let parent = ensure_running_run_with_profile(
            &app,
            &execution,
            &session.id,
            "Parent remains independent",
            "runtime-openai-safe",
        );
        let selection = crate::orchestration::test_runtime_selection(&app, "runtime-codex-safe");

        let spawned = execution
            .spawn_run(
                session.id.clone(),
                SpawnRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id.clone(),
                    objective: "Independent fresh child".to_string(),
                    selection,
                    output_contract: None,
                    recipe_id: None,
                    workspace_scope: WorkspaceMode::WorkspaceWrite,
                    cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
                    planned_write_files: Vec::new(),
                },
            )
            .expect("fresh spawn should persist before provider dispatch");

        assert_eq!(spawned.run.harness, RunHarnessKind::CodexAppServer);
        assert_eq!(spawned.run.status, RunStatus::WaitingForApproval);
        let persisted = execution
            .load_run_projection(&spawned.run.id)
            .expect("fresh child should persist");
        assert!(matches!(
            persisted.source,
            RunSource::FreshSpawn { parent_run_id, .. } if parent_run_id == parent.id
        ));
        assert!(
            execution
                .native_history_initial_state_for_run(&session.id, &spawned.run.id)
                .expect("fresh child dispatch state")
                .is_none()
        );
        let rows = execution
            .store
            .lock()
            .expect("store")
            .session_agent_turns_page(&SessionAgentTurnsPageQuery {
                session_id: session.id,
                before_sequence: None,
                limit: 20,
            })
            .expect("turn page")
            .rows;
        assert_eq!(
            rows.iter()
                .filter_map(|row| match row {
                    AgentTurnRow::User(row) if row.run_id == spawned.run.id =>
                        Some(row.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["Independent fresh child"]
        );
    }

    #[test]
    fn fresh_spawn_join_is_direct_lineage_idempotent_for_pending_child() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Fresh direct join");
        let parent = ensure_running_run_with_profile(
            &app,
            &execution,
            &session.id,
            "Parent",
            "runtime-openai-safe",
        );
        let spawned = execution
            .spawn_run(
                session.id.clone(),
                SpawnRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id.clone(),
                    objective: "Fresh child".to_string(),
                    selection: crate::orchestration::test_runtime_selection(
                        &app,
                        "runtime-codex-safe",
                    ),
                    output_contract: None,
                    recipe_id: None,
                    workspace_scope: WorkspaceMode::WorkspaceWrite,
                    cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
                    planned_write_files: Vec::new(),
                },
            )
            .expect("spawn");
        let request = JoinRunRequest {
            session_id: session.id.clone(),
            parent_run_id: parent.id,
            child_run_id: spawned.run.id,
        };

        let first = execution
            .join_run(session.id.clone(), request.clone())
            .expect("first join");
        let second = execution
            .join_run(session.id, request)
            .expect("second join");

        assert_eq!(first, second);
        assert_eq!(first.run.status, RunStatus::WaitingForApproval);
        assert!(first.result.is_none());
        assert!(first.receipts.is_empty());
        assert!(first.artifacts.is_empty());
    }

    #[test]
    fn fresh_spawn_cancellation_isolation_keeps_child_pending_when_parent_cancels() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Fresh cancellation isolation");
        let parent = ensure_running_run_with_profile(
            &app,
            &execution,
            &session.id,
            "Parent",
            "runtime-openai-safe",
        );
        attach_noop_handle(&execution, &parent.id);
        let spawned = execution
            .spawn_run(
                session.id.clone(),
                SpawnRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id.clone(),
                    objective: "Fresh child survives parent cancellation".to_string(),
                    selection: crate::orchestration::test_runtime_selection(
                        &app,
                        "runtime-openai-safe",
                    ),
                    output_contract: None,
                    recipe_id: None,
                    workspace_scope: WorkspaceMode::WorkspaceWrite,
                    cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
                    planned_write_files: Vec::new(),
                },
            )
            .expect("spawn");

        execution
            .cancel_run(session.id.clone(), approval_actor(), &parent.id, None)
            .expect("parent cancellation");
        let child = execution
            .load_run_projection(&spawned.run.id)
            .expect("child projection");
        assert_eq!(child.status, RunStatus::WaitingForApproval);
        assert!(matches!(child.source, RunSource::FreshSpawn { .. }));
    }

    #[test]
    fn fresh_spawn_join_projects_completed_result_receipt_and_canonical_artifact_name() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Fresh completed join");
        let parent = ensure_running_run_with_profile(
            &app,
            &execution,
            &session.id,
            "Parent",
            "runtime-openai-safe",
        );
        let spawned = execution
            .spawn_run(
                session.id.clone(),
                SpawnRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id.clone(),
                    objective: "Fresh native child".to_string(),
                    selection: crate::orchestration::test_runtime_selection(
                        &app,
                        "runtime-openai-safe",
                    ),
                    output_contract: None,
                    recipe_id: None,
                    workspace_scope: WorkspaceMode::WorkspaceWrite,
                    cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
                    planned_write_files: Vec::new(),
                },
            )
            .expect("spawn");
        let run_id = spawned.run.id.clone();
        {
            let mut store = execution.store.lock().expect("store");
            let pending = store.run(&run_id).expect("read").expect("fresh child");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        status: RunStatus::Running,
                        ..pending
                    },
                    user_turn: UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Run(
                        RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
                            .expect("active status"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("child should become live");
        }
        execution
            .runtime
            .claim_live_run(run_id.clone(), session.id.clone());
        attach_noop_handle(&execution, &run_id);
        execution
            .record_artifact(ArtifactRecord {
                id: ArtifactId::new("artifact-fresh-join").expect("artifact id"),
                session_id: session.id.clone(),
                run_id: run_id.clone(),
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "internal/private/fresh.patch".to_string(),
            })
            .expect("artifact should publish");
        execution
            .complete_run_with_result(
                session.id.clone(),
                &run_id,
                "Fresh child complete".to_string(),
                Some(CapsuleResult::Debug(DebugResult {
                    reproduced: false,
                    root_cause: None,
                    evidence_receipt_ids: Vec::new(),
                    patch_receipt_id: None,
                    confidence: 1.0,
                    blockers: Vec::new(),
                })),
            )
            .expect("completion");

        let joined = execution
            .join_run(
                session.id.clone(),
                JoinRunRequest {
                    session_id: session.id,
                    parent_run_id: parent.id,
                    child_run_id: run_id,
                },
            )
            .expect("join");
        assert_eq!(joined.run.status, RunStatus::Completed);
        assert!(joined.result.is_some());
        assert!(!joined.receipts.is_empty());
        assert_eq!(joined.artifacts[0].display_name, "fresh.patch");
    }
}
