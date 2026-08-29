use ta_protocol::wire::{
    CapsuleResult, ContextReceiptEvent, DaemonEvent, OutputContractKind, ReceiptProvenance,
    RunSource, RunStatus,
};
use ta_store::{CommitReceiptEvent, CreateReceipt, EventRecord, PersistenceStore, RunProjection};

use super::super::{
    RunExecutionError, RunExecutionService, RunMutationResult,
    completion_result::{
        CompletionReceiptState, OutputContractKindLabel, completion_event_sequence,
        receipt_kind_for_contract, receipt_kind_for_result, validate_completion_payload,
    },
    current_time_ms, output_contract_for_run,
};
use super::RunCompletionProjection;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn complete_live_run_and_publish(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        detail: String,
        result: Option<CapsuleResult>,
        generation: u64,
    ) -> Result<(), RunExecutionError> {
        let completed =
            self.complete_live_run_transition(session_id, run_id, detail, result, generation)?;
        self.publish_records(&completed.events);
        Ok(())
    }

    pub(crate) fn complete_run_with_result(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        detail: String,
        result: Option<CapsuleResult>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        self.complete_live_run_transition_current(session_id, run_id, detail, result)
    }

    fn complete_live_run_transition(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        detail: String,
        result: Option<CapsuleResult>,
        generation: u64,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let run = self.load_run_projection(run_id)?;
        let output_contract = output_contract_for_run(&run);
        if let Err(validation_error) = validate_completion_payload(output_contract, result.as_ref())
        {
            tracing::warn!(
                run_id = run_id.as_str(),
                output_contract = ?output_contract,
                detail = %validation_error,
                "output contract validation failed"
            );
            let event = crate::RunEvent::terminal(
                run_id.clone(),
                RunStatus::Failed,
                crate::RunStatusReason::new(validation_error.to_string()).map_err(|error| {
                    RunExecutionError::ProviderExecutionFailed(error.to_string())
                })?,
                output_contract,
                super::recipe_id_for_run(&run),
                result.clone(),
            )
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
            let committed = self.commit_terminal_with_completion_receipt(
                session_id.clone(),
                run_id,
                event,
                RunCompletionProjection {
                    output_contract,
                    result: result.clone(),
                    contract_violation: Some(validation_error.clone()),
                },
                generation,
                &run,
                CompletionReceiptState::Quarantined,
                None,
            )?;
            let mut records = committed.events;
            records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Failed)?);
            self.publish_records(&records);
            return Err(RunExecutionError::OutputContractViolation(validation_error));
        }

        let completion = RunCompletionProjection {
            output_contract,
            result: result.clone(),
            contract_violation: None,
        };
        let (prepared_checkpoint, event) = match self.prepare_after_user_turn_checkpoint(run_id) {
            Ok(prepared) => (
                prepared,
                crate::RunEvent::terminal(
                    run_id.clone(),
                    RunStatus::Completed,
                    crate::RunStatusReason::new(detail).map_err(|error| {
                        RunExecutionError::ProviderExecutionFailed(error.to_string())
                    })?,
                    completion.output_contract.clone(),
                    super::recipe_id_for_run(&run),
                    completion.result.clone(),
                )
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?,
            ),
            Err(error) => {
                tracing::error!(
                    run_id = run_id.as_str(),
                    error = %error,
                    "after-turn checkpoint capture failed; failing terminal run"
                );
                (None, crate::RunEvent::terminal(
                    run_id.clone(),
                    RunStatus::Failed,
                    crate::RunStatusReason::new(
                        "Run failed because the after-turn Git checkpoint could not be captured",
                    )
                    .map_err(|error| {
                        RunExecutionError::ProviderExecutionFailed(error.to_string())
                    })?,
                    None,
                    super::recipe_id_for_run(&run),
                    None,
                )
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?)
            }
        };
        let completion = if matches!(
            event,
            crate::RunEvent::Status(ref event) if event.status() == RunStatus::Failed
        ) {
            RunCompletionProjection::default()
        } else {
            completion
        };
        let receipt_state = (matches!(
            event,
            crate::RunEvent::Status(ref event) if event.status() == RunStatus::Completed
        ))
        .then_some(CompletionReceiptState::Promoted);
        let committed = match receipt_state {
            Some(state) => self.commit_terminal_with_completion_receipt(
                session_id.clone(),
                run_id,
                event,
                completion,
                generation,
                &run,
                state,
                prepared_checkpoint,
            )?,
            None => self.commit_live_run_event(
                session_id.clone(),
                run_id,
                event,
                completion,
                generation,
                prepared_checkpoint,
                ta_store::AuthProfileCommitMutation::Unchanged,
            )?,
        };
        let mut records = committed.events;
        let terminal_status = committed.run.status;
        records.extend(self.advance_ready_queue(&session_id, run_id, terminal_status)?);
        Ok(RunMutationResult {
            run: committed.run,
            events: records,
        })
    }

    fn complete_live_run_transition_current(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        detail: String,
        result: Option<CapsuleResult>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        // The external command is the sole path permitted to select the live
        // generation. Its selection and durable terminal commit are one
        // owner-held operation; no provider callback may use this entry.
        let run = self.load_run_projection(run_id)?;
        let output_contract = output_contract_for_run(&run);
        let (mut event, mut completion, mut receipt_state, mut semantic_error) =
            match validate_completion_payload(output_contract.clone(), result.as_ref()) {
                Ok(()) => (
                    crate::RunEvent::terminal(
                        run_id.clone(),
                        RunStatus::Completed,
                        crate::RunStatusReason::new(detail).map_err(|error| {
                            RunExecutionError::ProviderExecutionFailed(error.to_string())
                        })?,
                        output_contract.clone(),
                        super::recipe_id_for_run(&run),
                        result.clone(),
                    )
                    .map_err(|error| {
                        RunExecutionError::ProviderExecutionFailed(error.to_string())
                    })?,
                    RunCompletionProjection {
                        output_contract,
                        result,
                        contract_violation: None,
                    },
                    CompletionReceiptState::Promoted,
                    None,
                ),
                Err(error) => (
                    crate::RunEvent::terminal(
                        run_id.clone(),
                        RunStatus::Failed,
                        crate::RunStatusReason::new(error.to_string()).map_err(|error| {
                            RunExecutionError::ProviderExecutionFailed(error.to_string())
                        })?,
                        output_contract.clone(),
                        super::recipe_id_for_run(&run),
                        result.clone(),
                    )
                    .map_err(|error| {
                        RunExecutionError::ProviderExecutionFailed(error.to_string())
                    })?,
                    RunCompletionProjection {
                        output_contract,
                        result,
                        contract_violation: Some(error.clone()),
                    },
                    CompletionReceiptState::Quarantined,
                    Some(error),
                ),
            };
        let (prepared_checkpoint, checkpoint_error) =
            match self.prepare_after_user_turn_checkpoint(run_id) {
                Ok(prepared_checkpoint) => (prepared_checkpoint, None),
                Err(error) => {
                    tracing::error!(
                        run_id = run_id.as_str(),
                        error = %error,
                        "after-turn checkpoint capture failed; failing terminal run"
                    );
                    event = crate::RunEvent::terminal(
                    run_id.clone(),
                    RunStatus::Failed,
                    crate::RunStatusReason::new(
                        "Run failed because the after-turn Git checkpoint could not be captured",
                    )
                    .map_err(|error| {
                        RunExecutionError::ProviderExecutionFailed(error.to_string())
                    })?,
                    None,
                    super::recipe_id_for_run(&run),
                    None,
                )
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
                    completion = RunCompletionProjection::default();
                    receipt_state = CompletionReceiptState::Quarantined;
                    semantic_error = None;
                    (None, Some(error))
                }
            };
        let terminal = self
            .runtime
            .with_current_terminal_live_generation_lease_and_take_handle(
                run_id,
                &session_id,
                |_generation| {
                    let mut store = self.store.lock().expect("app store should not be poisoned");
                    self.commit_terminal_body(
                        &mut *store,
                        session_id.clone(),
                        run_id,
                        event,
                        completion,
                        ta_store::AuthProfileCommitMutation::Unchanged,
                        Some(receipt_state),
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
        let mut records = committed.events;
        records.extend(self.advance_ready_queue(&session_id, run_id, committed.run.status)?);
        if let Some(error) = checkpoint_error {
            return Err(error);
        }
        if let Some(error) = semantic_error {
            self.publish_records(&records);
            return Err(RunExecutionError::OutputContractViolation(error));
        }
        Ok(RunMutationResult {
            run: committed.run,
            events: records,
        })
    }

    fn commit_terminal_with_completion_receipt(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        event: crate::RunEvent,
        completion: RunCompletionProjection,
        generation: u64,
        run: &RunProjection,
        state: CompletionReceiptState,
        prepared_checkpoint: Option<super::super::checkpoints::PreparedAfterUserTurnCheckpoint>,
    ) -> Result<RunMutationResult, RunExecutionError> {
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
                        session_id.clone(),
                        run_id,
                        event,
                        completion,
                        ta_store::AuthProfileCommitMutation::Unchanged,
                        Some(state),
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

    pub(super) fn commit_terminal_body(
        &self,
        store: &mut S,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        event: crate::RunEvent,
        completion: RunCompletionProjection,
        auth_profile_mutation: ta_store::AuthProfileCommitMutation,
        receipt_state: Option<CompletionReceiptState>,
        prepared_checkpoint: Option<&super::super::checkpoints::PreparedAfterUserTurnCheckpoint>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let Some(run) = store.run(run_id)? else {
            return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
        };
        if run.session_id != session_id || run.status != RunStatus::Running {
            return Err(RunExecutionError::RunNotLiveOwned(
                run_id.as_str().to_string(),
            ));
        }
        let receipt_result = completion.result.clone();
        let committed = super::commit_live_run_event_in_store(
            store,
            session_id,
            run_id,
            event,
            completion,
            auth_profile_mutation,
        )?;
        let mut events = committed.events;
        if let Some(state) = receipt_state
            && let Some(sequence) = completion_event_sequence(&events)
        {
            events.extend(self.append_completion_result_receipt_in_store(
                store,
                &run,
                sequence,
                output_contract_for_run(&run),
                receipt_result.as_ref(),
                state,
            )?);
        }
        if let Some(prepared_checkpoint) = prepared_checkpoint {
            self.persist_prepared_after_user_turn_checkpoint(store, prepared_checkpoint)?;
        }
        Ok(RunMutationResult {
            run: committed.run,
            events,
        })
    }

    fn append_completion_result_receipt_in_store(
        &self,
        store: &mut S,
        run: &RunProjection,
        event_seq: u64,
        output_contract: Option<OutputContractKind>,
        result: Option<&CapsuleResult>,
        state: CompletionReceiptState,
    ) -> Result<Vec<EventRecord>, RunExecutionError> {
        let Some(receipt_kind) = result
            .map(receipt_kind_for_result)
            .or_else(|| output_contract.map(receipt_kind_for_contract))
        else {
            return Ok(Vec::new());
        };
        let Some(label) = output_contract
            .or_else(|| result.map(CapsuleResult::contract_kind))
            .map(OutputContractKindLabel::completion_label)
        else {
            return Ok(Vec::new());
        };
        let parent_run_id = match &run.source {
            RunSource::NativeSubagent { parent_run_id, .. }
            | RunSource::Forked { parent_run_id, .. }
            | RunSource::AccountSwitchedContinuation { parent_run_id, .. } => {
                Some(parent_run_id.clone())
            }
            RunSource::ScheduledWork { .. }
            | RunSource::User { .. }
            | RunSource::FreshSpawn { .. } => None,
        };
        let agent_turn_id = match &run.source {
            RunSource::NativeSubagent { parent_turn_id, .. } => Some(parent_turn_id.clone()),
            RunSource::ScheduledWork { .. }
            | RunSource::User { .. }
            | RunSource::Forked { .. }
            | RunSource::AccountSwitchedContinuation { .. }
            | RunSource::FreshSpawn { .. } => None,
        };
        let provenance = ReceiptProvenance {
            artifact_id: None,
            event_seq: agent_turn_id.as_ref().map(|_| event_seq),
            agent_turn_id,
            stream_cursor: Some(format!("run:{}:event:{event_seq}", run.id.as_str())),
        };
        let receipt = store.create(CreateReceipt {
            session_id: run.session_id.clone(),
            run_id: run.id.clone(),
            parent_run_id,
            kind: receipt_kind,
            provenance,
            title: Some(format!("{label} result")),
            summary: Some(completion_receipt_summary(label, result, state)),
        })?;
        match state {
            CompletionReceiptState::Promoted => store.promote(&receipt.id)?,
            CompletionReceiptState::Quarantined => store.quarantine(&receipt.id)?,
        };
        let committed = store.commit_receipt_event(CommitReceiptEvent {
            session_id: run.session_id.clone(),
            event: DaemonEvent::ContextReceipt(match state {
                CompletionReceiptState::Promoted => ContextReceiptEvent::Promoted { receipt },
                CompletionReceiptState::Quarantined => ContextReceiptEvent::Quarantined { receipt },
            }),
            occurred_at_ms: current_time_ms(),
        })?;
        Ok(vec![committed.event])
    }
}

fn completion_receipt_summary(
    label: &str,
    result: Option<&CapsuleResult>,
    state: CompletionReceiptState,
) -> String {
    match (state, result) {
        (CompletionReceiptState::Promoted, Some(result)) => format!(
            "{} CapsuleResult returned from native run",
            result.contract_kind().completion_label()
        ),
        (CompletionReceiptState::Quarantined, Some(result)) => format!(
            "{} CapsuleResult quarantined after daemon validation",
            result.contract_kind().completion_label()
        ),
        (CompletionReceiptState::Quarantined, None) => {
            format!("{label} CapsuleResult missing from native run")
        }
        (CompletionReceiptState::Promoted, None) => format!("{label} completion result"),
    }
}
