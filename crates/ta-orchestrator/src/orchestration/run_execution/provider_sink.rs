use ta_protocol::wire::{
    AgentStreamEvent, AgentStreamItemId, AgentStreamTurnId, ApprovalRequest, ApprovalResolution,
    ArtifactId, ArtifactKind, AuthProfileExhaustion, CapsuleResult, TokenUsageRecordedEvent,
    ValidationError,
};
use ta_provider_llm::client::LlmTokenUsage;
use ta_store::{ArtifactRecord, CommitRunTransition};
use taugentic_agent::{
    ExecutionError, ExecutionSink, NativeChildRunRequest, NativeChildRunResult, StreamEmission,
};
use uuid::Uuid;

use super::*;

mod completion;

pub(crate) struct ProviderRunExecutionSink<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) service: RunExecutionService<S>,
    pub(crate) session_id: crate::SessionId,
    pub(crate) run_id: RunId,
    /// Ephemeral capability issued by ActiveExecutionOwner. Never persisted.
    pub(crate) generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RunCompletionProjection {
    output_contract: Option<OutputContractKind>,
    result: Option<CapsuleResult>,
    contract_violation: Option<ValidationError>,
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    /// The provider callback capability is retained from owner validation
    /// through the entire store mutation. Publishing occurs after this method
    /// returns, so no provider code runs while the owner lease is held.
    fn with_live_generation_store_mutation<T>(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        generation: u64,
        action: impl FnOnce(&mut S) -> Result<T, RunExecutionError>,
    ) -> Result<T, RunExecutionError> {
        self.runtime
            .with_live_generation_lease(run_id, session_id, generation, || {
                let mut store = self.store.lock().expect("app store should not be poisoned");
                action(&mut store)
            })
    }

    fn emit_live_run_detail_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        _detail: String,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let committed =
            self.with_live_generation_store_mutation(&session_id, run_id, generation, |store| {
                let Some(run) = store.run(run_id)? else {
                    return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
                };
                let event = crate::RunEvent::active(
                    run_id.clone(),
                    RunStatus::Running,
                    None,
                    recipe_id_for_run(&run),
                    None,
                )
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
                commit_live_run_event_in_store(
                    store,
                    session_id.clone(),
                    run_id,
                    event,
                    RunCompletionProjection::default(),
                    ta_store::AuthProfileCommitMutation::Unchanged,
                )
            })?;
        self.publish_records(&committed.events);
        Ok(())
    }

    fn emit_agent_stream_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        frame: StreamEmission,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let committed = self.commit_live_agent_stream(session_id, run_id, frame, generation)?;
        self.publish_records(&committed.events);
        Ok(())
    }

    fn record_token_usage_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        usage: LlmTokenUsage,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let commit_session_id = session_id.clone();
        let committed =
            self.with_live_generation_store_mutation(&session_id, run_id, generation, |store| {
                let Some(existing_run) = store.run(run_id)? else {
                    return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
                };
                if existing_run.session_id != session_id {
                    return Err(RunExecutionError::RunSessionMismatch(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                if existing_run.status != RunStatus::Running {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                let recorded_at_ms = current_time_ms();
                Ok(store.commit_run_transition(CommitRunTransition {
                    session_id: commit_session_id,
                    run: existing_run,
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::TokenUsageRecorded(TokenUsageRecordedEvent {
                        run_id: run_id.clone(),
                        capsule_id: None,
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        cached_tokens: usage.cached_tokens,
                        reasoning_tokens: usage.reasoning_tokens,
                        model: usage.model,
                        provider: usage.provider,
                        recorded_at_ms,
                    })],
                    occurred_at_ms: recorded_at_ms,
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?)
            })?;
        self.publish_records(&committed.events);
        Ok(())
    }

    pub(super) fn fail_live_run_and_publish_for_generation(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        detail: String,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let committed = self.commit_failed_live_run_for_generation(
            session_id.clone(),
            run_id,
            detail,
            RunCompletionProjection::default(),
            generation,
        )?;
        let mut records = committed.events;
        records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Failed)?);
        self.publish_records(&records);
        Ok(())
    }

    pub(super) fn commit_failed_live_run_for_generation(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        detail: String,
        completion: RunCompletionProjection,
        generation: u64,
    ) -> Result<RunMutationResult, RunExecutionError> {
        self.commit_failed_live_run_for_generation_with_exhaustion(
            session_id, run_id, detail, completion, generation, None,
        )
    }

    fn fail_typed_exhaustion_and_publish_for_generation(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        exhaustion: AuthProfileExhaustion,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let detail = match exhaustion {
            AuthProfileExhaustion::RateLimited => "The selected account is rate limited.",
            AuthProfileExhaustion::CreditsExhausted => {
                "The selected account has exhausted its credits."
            }
        };
        let committed = self.commit_failed_live_run_for_generation_with_exhaustion(
            session_id.clone(),
            run_id,
            detail.to_string(),
            RunCompletionProjection::default(),
            generation,
            Some(exhaustion),
        )?;
        let mut records = committed.events;
        records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Failed)?);
        self.publish_records(&records);
        Ok(())
    }

    fn commit_failed_live_run_for_generation_with_exhaustion(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        mut detail: String,
        mut completion: RunCompletionProjection,
        generation: u64,
        exhaustion: Option<AuthProfileExhaustion>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let prepared_checkpoint = match self.prepare_after_user_turn_checkpoint(run_id) {
            Ok(prepared) => prepared,
            Err(error) => {
                tracing::error!(
                    run_id = run_id.as_str(),
                    error = %error,
                    "after-turn checkpoint capture failed; failing terminal run"
                );
                if exhaustion.is_none() {
                    detail =
                        format!("{detail}; the after-turn Git checkpoint could not be captured");
                }
                completion = RunCompletionProjection::default();
                None
            }
        };
        let run = self.load_run_projection(run_id)?;
        let auth_profile_mutation = match exhaustion {
            Some(exhaustion) => match run.source.route().auth_profile_id.clone() {
                Some(auth_profile_id) => ta_store::AuthProfileCommitMutation::SetExhausted {
                    auth_profile_id,
                    exhaustion,
                },
                None => ta_store::AuthProfileCommitMutation::Unchanged,
            },
            None => ta_store::AuthProfileCommitMutation::Unchanged,
        };
        let reason = crate::RunStatusReason::new(detail)
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        let event = match exhaustion {
            Some(exhaustion) => crate::RunEvent::terminal_with_auth_profile_exhaustion(
                run_id.clone(),
                reason,
                exhaustion,
            ),
            None => crate::RunEvent::terminal(
                run_id.clone(),
                RunStatus::Failed,
                reason,
                completion.output_contract.clone(),
                recipe_id_for_run(&run),
                completion.result.clone(),
            ),
        }
        .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        self.commit_live_run_event(
            session_id,
            run_id,
            event,
            completion,
            generation,
            prepared_checkpoint,
            auth_profile_mutation,
        )
    }

    pub(super) fn commit_live_run_event(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        event: crate::RunEvent,
        completion: RunCompletionProjection,
        generation: u64,
        prepared_checkpoint: Option<super::checkpoints::PreparedAfterUserTurnCheckpoint>,
        auth_profile_mutation: ta_store::AuthProfileCommitMutation,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let commit_session_id = session_id.clone();
        let terminal = self
            .runtime
            .with_terminal_live_generation_lease_and_take_handle(
                run_id,
                &session_id,
                generation,
                || {
                    let mut store = self.store.lock().expect("app store should not be poisoned");
                    self.commit_terminal_body(
                        &mut *store,
                        commit_session_id,
                        run_id,
                        event,
                        completion,
                        auth_profile_mutation,
                        None,
                        prepared_checkpoint.as_ref(),
                    )
                },
            );
        let (committed, handle) = match terminal {
            Ok(value) => value,
            Err(error) => {
                if let Some(prepared_checkpoint) = prepared_checkpoint {
                    prepared_checkpoint.cleanup_unpersisted()?;
                }
                return Err(error);
            }
        };
        if let Some(handle) = handle {
            handle
                .cancel()
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        }
        Ok(committed)
    }

    fn commit_live_agent_stream(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        frame: StreamEmission,
        generation: u64,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let commit_session_id = session_id.clone();
        self.with_live_generation_store_mutation(&session_id, run_id, generation, |store| {
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if existing_run.status != RunStatus::Running {
                return Err(RunExecutionError::RunNotLiveOwned(
                    existing_run.id.as_str().to_string(),
                ));
            }
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: commit_session_id,
                run: existing_run,
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::AgentStream(AgentStreamEvent {
                    run_id: run_id.clone(),
                    emission: frame,
                })],
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })?;
            Ok(RunMutationResult {
                run: project_run_summary(committed.run),
                events: committed.events,
            })
        })
    }

    fn record_artifact_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: RunId,
        kind: ArtifactKind,
        storage_path: String,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let artifact_session_id = session_id.clone();
        let artifact_run_id = run_id.clone();
        let artifact =
            self.runtime
                .with_live_generation_lease(&run_id, &session_id, generation, || {
                    self.record_artifact_for_leased_run(ArtifactRecord {
                        id: ArtifactId::new(format!("artifact-{}", Uuid::new_v4().simple()))
                            .expect("generated artifact id should be valid"),
                        session_id: artifact_session_id,
                        run_id: artifact_run_id,
                        kind,
                        metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                        storage_path,
                    })
                })?;
        self.publish_records(&artifact.events);
        Ok(())
    }

    fn request_approval_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        request: ApprovalRequest,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        if request.run_id != *run_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.run_id.as_str().to_string(),
            ));
        }
        let commit_session_id = session_id.clone();
        let committed =
            self.with_live_generation_store_mutation(&session_id, run_id, generation, |store| {
                let Some(existing_run) = store.run(run_id)? else {
                    return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
                };
                if existing_run.session_id != session_id {
                    return Err(RunExecutionError::RunSessionMismatch(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                if existing_run.status != RunStatus::Running {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                Ok(store.commit_run_transition(CommitRunTransition {
                    session_id: commit_session_id,
                    run: existing_run,
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Approval(ApprovalEvent::Requested { request })],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?)
            })?;
        self.publish_records(&committed.events);
        Ok(())
    }

    fn resolve_approval_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        resolution: ApprovalResolution,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        if resolution.run_id != *run_id {
            return Err(RunExecutionError::RunSessionMismatch(
                resolution.run_id.as_str().to_string(),
            ));
        }
        let commit_session_id = session_id.clone();
        let committed =
            self.with_live_generation_store_mutation(&session_id, run_id, generation, |store| {
                let Some(existing_run) = store.run(run_id)? else {
                    return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
                };
                if existing_run.session_id != session_id {
                    return Err(RunExecutionError::RunSessionMismatch(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                if existing_run.status != RunStatus::Running {
                    return Err(RunExecutionError::RunNotLiveOwned(
                        existing_run.id.as_str().to_string(),
                    ));
                }
                Ok(store.commit_run_transition(CommitRunTransition {
                    session_id: commit_session_id,
                    run: existing_run,
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                        resolution,
                    })],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?)
            })?;
        self.publish_records(&committed.events);
        Ok(())
    }

    pub(super) fn publish_records(&self, records: &[EventRecord]) {
        for record in records {
            self.runtime.publish_record(record);
        }
    }
}

impl<S> ExecutionSink for ProviderRunExecutionSink<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn push_stream(&self, frame: StreamEmission) -> Result<(), ExecutionError> {
        self.service
            .emit_agent_stream_and_publish(
                self.session_id.clone(),
                &self.run_id,
                frame,
                self.generation,
            )
            .and_then(|()| {
                self.service.enforce_budget_after_stream(
                    &self.session_id,
                    &self.run_id,
                    self.generation,
                )
            })
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn record_token_usage(&self, usage: LlmTokenUsage) -> Result<(), ExecutionError> {
        self.service
            .record_token_usage_and_publish(
                self.session_id.clone(),
                &self.run_id,
                usage,
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn push_activity(&self, detail: &str) -> Result<(), ExecutionError> {
        self.service
            .emit_live_run_detail_and_publish(
                self.session_id.clone(),
                &self.run_id,
                detail.to_string(),
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn push_provider_session_id(&self, _: String) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn request_approval(&self, request: ApprovalRequest) -> Result<(), ExecutionError> {
        self.service
            .request_approval_and_publish(
                self.session_id.clone(),
                &self.run_id,
                request,
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        self.service
            .resolve_approval_and_publish(
                self.session_id.clone(),
                &self.run_id,
                resolution,
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn record_artifact(
        &self,
        kind: ArtifactKind,
        storage_path: &str,
    ) -> Result<(), ExecutionError> {
        self.service
            .record_artifact_and_publish(
                self.session_id.clone(),
                self.run_id.clone(),
                kind,
                storage_path.to_string(),
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn record_image_artifact(
        &self,
        turn_id: AgentStreamTurnId,
        item_id: AgentStreamItemId,
        data_base64: &str,
    ) -> Result<(), ExecutionError> {
        let artifact = self
            .service
            .record_generated_image_for_leased_run(
                self.session_id.clone(),
                self.run_id.clone(),
                self.generation,
                turn_id,
                item_id,
                data_base64,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))?;
        self.service.publish_records(&artifact.events);
        Ok(())
    }

    fn start_native_child_run(
        &self,
        request: NativeChildRunRequest,
    ) -> Result<NativeChildRunResult, ExecutionError> {
        if request.parent_run_id != self.run_id {
            return Err(ExecutionError::Unsupported(format!(
                "native child run parent {} does not match active run {}",
                request.parent_run_id.as_str(),
                self.run_id.as_str()
            )));
        }
        self.service
            .start_native_child_run_from_generation(
                self.session_id.clone(),
                request,
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn complete(&self, detail: &str) -> Result<(), ExecutionError> {
        self.complete_with_result(detail, None)
    }

    fn complete_with_result(
        &self,
        detail: &str,
        result: Option<CapsuleResult>,
    ) -> Result<(), ExecutionError> {
        self.service
            .complete_live_run_and_publish(
                self.session_id.clone(),
                &self.run_id,
                detail.to_string(),
                result,
                self.generation,
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn fail(&self, error: ExecutionError) -> Result<(), ExecutionError> {
        let result = match error {
            ExecutionError::RateLimited { .. } => self
                .service
                .fail_typed_exhaustion_and_publish_for_generation(
                    self.session_id.clone(),
                    &self.run_id,
                    AuthProfileExhaustion::RateLimited,
                    self.generation,
                ),
            ExecutionError::CreditsExhausted(_) => self
                .service
                .fail_typed_exhaustion_and_publish_for_generation(
                    self.session_id.clone(),
                    &self.run_id,
                    AuthProfileExhaustion::CreditsExhausted,
                    self.generation,
                ),
            error => self.service.fail_live_run_and_publish_for_generation(
                self.session_id.clone(),
                &self.run_id,
                error.to_string(),
                self.generation,
            ),
        };
        result.map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }
}

fn commit_live_run_event_in_store<S>(
    store: &mut S,
    session_id: crate::SessionId,
    run_id: &RunId,
    event: crate::RunEvent,
    completion: RunCompletionProjection,
    auth_profile_mutation: ta_store::AuthProfileCommitMutation,
) -> Result<RunMutationResult, RunExecutionError>
where
    S: PersistenceStore,
{
    let Some(existing_run) = store.run(run_id)? else {
        return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
    };
    if existing_run.session_id != session_id {
        return Err(RunExecutionError::RunSessionMismatch(
            existing_run.id.as_str().to_string(),
        ));
    }
    if existing_run.status != RunStatus::Running {
        return Err(RunExecutionError::RunNotLiveOwned(
            existing_run.id.as_str().to_string(),
        ));
    }
    let projected_result = completion
        .contract_violation
        .is_none()
        .then(|| completion.result.clone())
        .flatten();
    let crate::RunEvent::Status(status_event) = &event else {
        return Err(RunExecutionError::ProviderExecutionFailed(
            "invalid provider status event".to_string(),
        ));
    };
    let committed = store.commit_run_transition(CommitRunTransition {
        session_id,
        run: RunProjection {
            status: status_event.status(),
            result: projected_result,
            contract_violation: completion.contract_violation,
            ..existing_run
        },
        user_turn: ta_store::UserTurnCommit::NoUserTurn,
        events: vec![DaemonEvent::Run(event)],
        occurred_at_ms: current_time_ms(),
        auth_profile_mutation,
    })?;
    Ok(RunMutationResult {
        run: project_run_summary(committed.run),
        events: committed.events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{ListReceiptsRequest, ReceiptState};
    use ta_protocol::wire::{
        DebugResult, GitCheckpointPhase, OutputContractKind, PatchResult, ReceiptKind,
        ValidationError,
    };
    use ta_provider_llm::client::LlmTokenUsage;
    use ta_store::{
        AuthProfileRepository, CheckpointRepository, CommitRepository, EventLogRepository,
        ProjectionRepository,
    };

    #[test]
    fn complete_with_valid_capsule_result_promotes_parent_receipt() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Contract child");
        let parent_run_id = RunId::new("run-parent-contract").expect("parent run id");
        let child_run_id = seed_native_child_run(
            &execution,
            &session.id,
            &parent_run_id,
            Some(OutputContractKind::Patch),
        );
        let sink = provider_sink(&execution, &session.id, &child_run_id);
        let result = CapsuleResult::Patch(PatchResult {
            patch_receipt_ids: vec!["receipt_patch".to_string()],
            touched_files: vec!["crates/example.rs".to_string()],
            tests_run_receipt_ids: vec!["receipt_tests".to_string()],
            passing: true,
            blockers: Vec::new(),
        });

        sink.complete_with_result("patch complete", Some(result.clone()))
            .expect("valid result should complete");

        let run = execution
            .store
            .lock()
            .expect("store lock")
            .run(&child_run_id)
            .expect("run lookup")
            .expect("run should exist");
        let receipts = app
            .list_receipts(
                &session.id,
                &ListReceiptsRequest {
                    session_id: session.id.clone(),
                    run_id: Some(child_run_id.clone()),
                    parent_run_id: Some(parent_run_id),
                    state: Some(ReceiptState::Promoted),
                    kind: Some(ReceiptKind::Patch),
                    limit: None,
                },
            )
            .expect("receipts should list");

        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(receipts.receipts.len(), 1);
        assert_completion_event(
            &execution,
            &session.id,
            &child_run_id,
            RunStatus::Completed,
            Some(result),
        );
    }

    #[test]
    fn complete_with_invalid_capsule_result_fails_and_quarantines() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Contract drift");
        let parent_run_id = RunId::new("run-parent-drift").expect("parent run id");
        let child_run_id = seed_native_child_run(
            &execution,
            &session.id,
            &parent_run_id,
            Some(OutputContractKind::Debug),
        );
        let sink = provider_sink(&execution, &session.id, &child_run_id);
        let result = CapsuleResult::Debug(DebugResult {
            reproduced: false,
            root_cause: None,
            evidence_receipt_ids: Vec::new(),
            patch_receipt_id: None,
            confidence: 1.5,
            blockers: Vec::new(),
        });

        let error = sink
            .complete_with_result("debug complete", Some(result.clone()))
            .expect_err("invalid result should fail");

        let run = execution
            .store
            .lock()
            .expect("store lock")
            .run(&child_run_id)
            .expect("run lookup")
            .expect("run should exist");
        let receipts = app
            .list_receipts(
                &session.id,
                &ListReceiptsRequest {
                    session_id: session.id.clone(),
                    run_id: Some(child_run_id.clone()),
                    parent_run_id: Some(parent_run_id),
                    state: Some(ReceiptState::Quarantined),
                    kind: Some(ReceiptKind::Summary),
                    limit: None,
                },
            )
            .expect("receipts should list");

        assert!(error.to_string().contains("output contract violation"));
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(receipts.receipts.len(), 1);
        assert_completion_event(
            &execution,
            &session.id,
            &child_run_id,
            RunStatus::Failed,
            Some(result),
        );
    }

    #[test]
    fn missing_contract_result_fails_with_structured_error_and_quarantines() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Contract missing result");
        let parent_run_id = RunId::new("run-parent-missing-result").expect("parent run id");
        let child_run_id = seed_native_child_run(
            &execution,
            &session.id,
            &parent_run_id,
            Some(OutputContractKind::Debug),
        );

        let error = execution
            .complete_run_with_result(
                session.id.clone(),
                &child_run_id,
                "normal end".to_string(),
                None,
            )
            .expect_err("missing contract result should fail through daemon validation");

        let run = execution
            .store
            .lock()
            .expect("store lock")
            .run(&child_run_id)
            .expect("run lookup")
            .expect("run should exist");
        let receipts = app
            .list_receipts(
                &session.id,
                &ListReceiptsRequest {
                    session_id: session.id.clone(),
                    run_id: Some(child_run_id.clone()),
                    parent_run_id: Some(parent_run_id),
                    state: Some(ReceiptState::Quarantined),
                    kind: Some(ReceiptKind::Summary),
                    limit: None,
                },
            )
            .expect("receipts should list");

        assert!(matches!(
            error,
            RunExecutionError::OutputContractViolation(ValidationError::Custom(message))
                if message.contains("requires a matching CapsuleResult")
        ));
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(receipts.receipts.len(), 1);
        assert_completion_event(
            &execution,
            &session.id,
            &child_run_id,
            RunStatus::Failed,
            None,
        );
    }

    #[test]
    fn legacy_completion_without_contract_still_completes() {
        let runtime = crate::RuntimeService::bootstrap();
        let (_app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&_app, "Legacy child");
        let child_run_id = seed_native_child_run(
            &execution,
            &session.id,
            &RunId::new("run-parent-legacy").expect("parent run id"),
            None,
        );
        let sink = provider_sink(&execution, &session.id, &child_run_id);

        sink.complete("normal end")
            .expect("legacy completion should complete");

        let run = execution
            .store
            .lock()
            .expect("store lock")
            .run(&child_run_id)
            .expect("run lookup")
            .expect("run should exist");
        assert_eq!(run.status, RunStatus::Completed);
        assert_completion_event(
            &execution,
            &session.id,
            &child_run_id,
            RunStatus::Completed,
            None,
        );
    }

    #[test]
    fn missing_after_turn_checkpoint_fails_and_releases_user_run() {
        let repository_root = init_dispatch_repo();
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        set_default_test_workspace_root(&app, repository_root.path());
        let session = open_session(&app, "Checkpoint terminal failure");
        let run = ensure_running_run(
            &app,
            &execution,
            &session.id,
            "Fail closed when the repository disappears",
        );
        execution
            .capture_before_user_turn(&run.id)
            .expect("before checkpoint should capture");
        std::fs::remove_dir_all(repository_root.path()).expect("remove isolated test repository");
        let sink = provider_sink(&execution, &session.id, &run.id);

        sink.complete("provider completed")
            .expect("terminal failure should be committed and published");

        let stored_run = execution
            .store
            .lock()
            .expect("store lock")
            .run(&run.id)
            .expect("run lookup")
            .expect("run should exist");
        let checkpoints = execution
            .store
            .lock()
            .expect("store lock")
            .checkpoints_for_run(&run.id)
            .expect("checkpoint lookup");
        assert_eq!(stored_run.status, RunStatus::Failed);
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].phase, GitCheckpointPhase::BeforeTurn);
        assert_eq!(execution.active_run_count(), 0);
        assert!(!execution.is_live_run_running(&run.id, &session.id));
        assert_completion_event(&execution, &session.id, &run.id, RunStatus::Failed, None);
    }

    #[test]
    fn typed_credit_exhaustion_marks_only_the_selected_route_profile() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Typed exhaustion");
        let run = ensure_running_run(&app, &execution, &session.id, "Fail once");
        let selected_profile = execution
            .store
            .lock()
            .expect("store lock")
            .run(&run.id)
            .expect("run lookup")
            .expect("run")
            .source
            .route()
            .auth_profile_id
            .clone()
            .expect("selected profile");
        let other_profile =
            ta_protocol::wire::AuthProfileId::new("profile-other-test").expect("other profile");
        execution
            .store
            .lock()
            .expect("store lock")
            .save_auth_profile(ta_store::connected_test_auth_profile(
                other_profile.as_str(),
                "method-test",
                "provider-test",
            ))
            .expect("other profile");

        provider_sink(&execution, &session.id, &run.id)
            .fail(ExecutionError::CreditsExhausted(
                "provider payload must not be persisted".to_string(),
            ))
            .expect("typed failure should commit");

        let store = execution.store.lock().expect("store lock");
        let events = store.events_for_session(&session.id).expect("events");
        assert!(events.iter().any(|record| matches!(
            &record.payload,
            DaemonEvent::Run(crate::RunEvent::Status(event))
                if event.run_id() == &run.id
                    && event.status() == RunStatus::Failed
                    && event.auth_profile_exhaustion() == Some(AuthProfileExhaustion::CreditsExhausted)
                    && event.reason().is_some_and(|reason| reason.as_str() == "The selected account has exhausted its credits.")
        )));
        assert_eq!(
            store
                .auth_profile(&selected_profile)
                .expect("selected profile lookup")
                .expect("selected profile")
                .profile
                .exhaustion,
            Some(AuthProfileExhaustion::CreditsExhausted)
        );
        assert_eq!(
            store
                .auth_profile(&other_profile)
                .expect("other profile lookup")
                .expect("other profile")
                .profile
                .exhaustion,
            None
        );
        assert_eq!(execution.active_run_count(), 0);
    }

    #[test]
    fn record_token_usage_persists_canonical_event() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Token usage");
        let child_run_id = seed_native_child_run(
            &execution,
            &session.id,
            &RunId::new("run-parent-token").expect("parent run id"),
            None,
        );
        let sink = provider_sink(&execution, &session.id, &child_run_id);

        sink.record_token_usage(LlmTokenUsage {
            prompt_tokens: 11,
            completion_tokens: 7,
            cached_tokens: Some(3),
            reasoning_tokens: Some(2),
            model: "gpt-test".to_string(),
            provider: "openai".to_string(),
        })
        .expect("token usage should persist");

        let events = execution
            .store
            .lock()
            .expect("store lock")
            .read_run_events(&ta_store::RunEventRangeQuery {
                session_id: session.id.clone(),
                run_id: child_run_id.clone(),
                after_sequence: None,
                limit: 20,
            })
            .expect("run events");
        assert!(events.records.iter().any(|record| matches!(
            &record.payload,
            DaemonEvent::TokenUsageRecorded(event)
                if event.run_id == child_run_id
                    && event.prompt_tokens == 11
                    && event.completion_tokens == 7
                    && event.cached_tokens == Some(3)
                    && event.reasoning_tokens == Some(2)
                    && event.model == "gpt-test"
                    && event.provider == "openai"
        )));
    }

    fn seed_native_child_run(
        execution: &RunExecutionService,
        session_id: &crate::SessionId,
        parent_run_id: &RunId,
        output_contract: Option<OutputContractKind>,
    ) -> RunId {
        let run_id = RunId::new(format!("run-{}", uuid::Uuid::new_v4().simple())).expect("run id");
        {
            let mut store = execution.store.lock().expect("store lock");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: RunProjection {
                        id: run_id.clone(),
                        session_id: session_id.clone(),
                        runtime_profile_id: crate::RuntimeProfileId::new("runtime-openai-safe")
                            .expect("runtime profile id"),
                        objective: "Child contract run".to_string(),
                        status: RunStatus::Running,
                        harness: RunHarnessKind::Native,
                        source: RunSource::NativeSubagent {
                            route: ta_store::default_test_run_source().route().clone(),
                            parent_run_id: parent_run_id.clone(),
                            parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new(
                                "turn-parent-contract",
                            )
                            .expect("turn id"),
                            output_contract,
                            model_id: None,
                            recipe_id: None,
                            workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
                            cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
                            planned_write_files: Vec::new(),
                        },
                        execution_context: ta_store::default_test_execution_context(),
                        result: None,
                        contract_violation: None,
                        started_at_ms: None,
                        ended_at_ms: None,
                        last_event_seq: None,
                        workspace_info: None,
                        claimed_files: Vec::new(),
                        conflict_summary: None,
                    },
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Run(
                        crate::RunEvent::active(
                            run_id.clone(),
                            RunStatus::Running,
                            None,
                            None,
                            None,
                        )
                        .expect("seed child status should be active"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("run should seed");
        }
        execution
            .runtime
            .claim_live_run(run_id.clone(), session_id.clone());
        run_id
    }

    fn assert_completion_event(
        execution: &RunExecutionService,
        session_id: &crate::SessionId,
        run_id: &RunId,
        status: RunStatus,
        result: Option<CapsuleResult>,
    ) {
        let events = execution
            .store
            .lock()
            .expect("store lock")
            .events_for_session(session_id)
            .expect("events should load");
        assert!(events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent::Status(event)) if event.run_id() == run_id
                    && event.status() == status
                    && event.result() == result.as_ref()
            )
        }));
    }
}
