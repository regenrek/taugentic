use ta_protocol::wire::{
    AgentStreamEvent, ApprovalRequest, ApprovalResolution, ArtifactId, ArtifactKind, CapsuleResult,
    TokenUsageRecordedEvent, ValidationError,
};
use ta_provider_llm::client::LlmTokenUsage;
use ta_store::{ArtifactRecord, CommitRunTransition};
use taugentic_agent::{
    ExecutionError, ExecutionSink, NativeChildRunRequest, NativeChildRunResult, StreamEmission,
};
use uuid::Uuid;

use super::*;

mod completion;

pub(super) struct ProviderRunExecutionSink<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) service: RunExecutionService<S>,
    pub(super) session_id: crate::SessionId,
    pub(super) run_id: RunId,
}

#[derive(Debug, Default)]
struct RunCompletionProjection {
    output_contract: Option<OutputContractKind>,
    result: Option<CapsuleResult>,
    contract_violation: Option<ValidationError>,
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn emit_live_run_detail_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        detail: String,
    ) -> Result<(), RunExecutionError> {
        let committed =
            self.commit_live_run_status(session_id, run_id, RunStatus::Running, detail)?;
        self.publish_records(&committed.events);
        Ok(())
    }

    fn emit_agent_stream_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        frame: StreamEmission,
    ) -> Result<(), RunExecutionError> {
        let committed = self.commit_live_agent_stream(session_id, run_id, frame)?;
        self.publish_records(&committed.events);
        Ok(())
    }

    fn record_token_usage_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        usage: LlmTokenUsage,
    ) -> Result<(), RunExecutionError> {
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if existing_run.status != RunStatus::Running
                || !self.runtime.is_live_run_running(run_id, &session_id)
            {
                return Err(RunExecutionError::RunNotLiveOwned(
                    existing_run.id.as_str().to_string(),
                ));
            }
            let recorded_at_ms = current_time_ms();
            store.commit_run_transition(CommitRunTransition {
                session_id,
                run: existing_run,
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
            })?
        };
        self.publish_records(&committed.events);
        Ok(())
    }

    pub(super) fn fail_live_run_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        detail: String,
    ) -> Result<(), RunExecutionError> {
        let committed =
            self.commit_live_run_status(session_id.clone(), run_id, RunStatus::Failed, detail)?;
        let mut records = committed.events;
        records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Failed)?);
        self.publish_records(&records);
        Ok(())
    }

    pub(super) fn fail_live_run_without_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        detail: String,
    ) -> Result<(RunProjection, Vec<EventRecord>), RunExecutionError> {
        let committed =
            self.commit_live_run_status(session_id, run_id, RunStatus::Failed, detail)?;
        Ok((self.load_run_projection(run_id)?, committed.events))
    }

    fn commit_live_run_status(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        status: RunStatus,
        detail: String,
    ) -> Result<RunMutationResult, RunExecutionError> {
        self.commit_live_run_status_with_completion(
            session_id,
            run_id,
            status,
            detail,
            RunCompletionProjection::default(),
        )
    }

    fn commit_live_run_status_with_completion(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        status: RunStatus,
        detail: String,
        completion: RunCompletionProjection,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let Some(existing_run) = store.run(run_id)? else {
            return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
        };
        if existing_run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                existing_run.id.as_str().to_string(),
            ));
        }
        if existing_run.status != RunStatus::Running
            || !self.runtime.is_live_run_running(run_id, &session_id)
        {
            return Err(RunExecutionError::RunNotLiveOwned(
                existing_run.id.as_str().to_string(),
            ));
        }
        let recipe_id = recipe_id_for_run(&existing_run);
        let projected_result = completion
            .contract_violation
            .is_none()
            .then(|| completion.result.clone())
            .flatten();

        let committed = store.commit_run_transition(CommitRunTransition {
            session_id,
            run: RunProjection {
                status,
                result: projected_result,
                contract_violation: completion.contract_violation,
                ..existing_run
            },
            events: vec![DaemonEvent::Run(crate::RunEvent {
                run_id: run_id.clone(),
                status,
                detail,
                output_contract: completion.output_contract,
                recipe_id,
                result: completion.result,
            })],
            occurred_at_ms: current_time_ms(),
        })?;
        Ok(RunMutationResult {
            run: project_run_summary(committed.run),
            events: committed.events,
        })
    }

    fn commit_live_agent_stream(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        frame: StreamEmission,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let Some(existing_run) = store.run(run_id)? else {
            return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
        };
        if existing_run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                existing_run.id.as_str().to_string(),
            ));
        }
        if existing_run.status != RunStatus::Running
            || !self.runtime.is_live_run_running(run_id, &session_id)
        {
            return Err(RunExecutionError::RunNotLiveOwned(
                existing_run.id.as_str().to_string(),
            ));
        }

        let committed = store.commit_run_transition(CommitRunTransition {
            session_id,
            run: existing_run,
            events: vec![DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: run_id.clone(),
                emission: frame,
            })],
            occurred_at_ms: current_time_ms(),
        })?;
        Ok(RunMutationResult {
            run: project_run_summary(committed.run),
            events: committed.events,
        })
    }

    fn record_artifact_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: RunId,
        kind: ArtifactKind,
        storage_path: String,
    ) -> Result<(), RunExecutionError> {
        let artifact = self.record_artifact(ArtifactRecord {
            id: ArtifactId::new(format!("artifact-{}", Uuid::new_v4().simple()))
                .expect("generated artifact id should be valid"),
            session_id,
            run_id,
            kind,
            storage_path,
        })?;
        self.publish_records(&artifact.events);
        Ok(())
    }

    fn request_approval_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        request: ApprovalRequest,
    ) -> Result<(), RunExecutionError> {
        if request.run_id != *run_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.run_id.as_str().to_string(),
            ));
        }
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if existing_run.status != RunStatus::Running
                || !self.runtime.is_live_run_running(run_id, &session_id)
            {
                return Err(RunExecutionError::RunNotLiveOwned(
                    existing_run.id.as_str().to_string(),
                ));
            }
            store.commit_run_transition(CommitRunTransition {
                session_id,
                run: existing_run,
                events: vec![DaemonEvent::Approval(ApprovalEvent::Requested { request })],
                occurred_at_ms: current_time_ms(),
            })?
        };
        self.publish_records(&committed.events);
        Ok(())
    }

    fn resolve_approval_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
        resolution: ApprovalResolution,
    ) -> Result<(), RunExecutionError> {
        if resolution.run_id != *run_id {
            return Err(RunExecutionError::RunSessionMismatch(
                resolution.run_id.as_str().to_string(),
            ));
        }
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if existing_run.status != RunStatus::Running
                || !self.runtime.is_live_run_running(run_id, &session_id)
            {
                return Err(RunExecutionError::RunNotLiveOwned(
                    existing_run.id.as_str().to_string(),
                ));
            }
            store.commit_run_transition(CommitRunTransition {
                session_id,
                run: existing_run,
                events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution,
                })],
                occurred_at_ms: current_time_ms(),
            })?
        };
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
            .emit_agent_stream_and_publish(self.session_id.clone(), &self.run_id, frame)
            .and_then(|()| {
                self.service
                    .enforce_budget_after_stream(&self.session_id, &self.run_id)
            })
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn record_token_usage(&self, usage: LlmTokenUsage) -> Result<(), ExecutionError> {
        self.service
            .record_token_usage_and_publish(self.session_id.clone(), &self.run_id, usage)
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn push_activity(&self, detail: &str) -> Result<(), ExecutionError> {
        self.service
            .emit_live_run_detail_and_publish(
                self.session_id.clone(),
                &self.run_id,
                detail.to_string(),
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn push_provider_session_id(&self, _: String) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn request_approval(&self, request: ApprovalRequest) -> Result<(), ExecutionError> {
        self.service
            .request_approval_and_publish(self.session_id.clone(), &self.run_id, request)
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        self.service
            .resolve_approval_and_publish(self.session_id.clone(), &self.run_id, resolution)
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
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
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
            .start_native_child_run(self.session_id.clone(), request)
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
            )
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }

    fn fail(&self, error: ExecutionError) -> Result<(), ExecutionError> {
        self.service
            .fail_live_run_and_publish(self.session_id.clone(), &self.run_id, error.to_string())
            .map_err(|error| ExecutionError::Unsupported(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{ListReceiptsRequest, ReceiptState};
    use ta_protocol::wire::{
        DebugResult, OutputContractKind, PatchResult, ReceiptKind, ValidationError,
    };
    use ta_provider_llm::client::LlmTokenUsage;
    use ta_store::{CommitRepository, EventLogRepository, ProjectionRepository};

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
                            parent_run_id: parent_run_id.clone(),
                            parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new(
                                "turn-parent-contract",
                            )
                            .expect("turn id"),
                            output_contract,
                            model_id: None,
                            sandbox_profile: None,
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
                    events: vec![DaemonEvent::Run(crate::RunEvent {
                        run_id: run_id.clone(),
                        status: RunStatus::Running,
                        detail: "seed child".to_string(),
                        output_contract: None,
                        recipe_id: None,
                        result: None,
                    })],
                    occurred_at_ms: current_time_ms(),
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
                DaemonEvent::Run(crate::RunEvent {
                    run_id: event_run_id,
                    status: event_status,
                    result: event_result,
                    ..
                }) if *event_run_id == *run_id
                    && *event_status == status
                    && event_result.as_ref() == result.as_ref()
            )
        }));
    }
}
