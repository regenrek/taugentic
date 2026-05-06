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
    ) -> Result<(), RunExecutionError> {
        let completed = self.complete_live_run_transition(session_id, run_id, detail, result)?;
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
        self.complete_live_run_transition(session_id, run_id, detail, result)
    }

    fn complete_live_run_transition(
        &self,
        session_id: crate::SessionId,
        run_id: &crate::RunId,
        detail: String,
        result: Option<CapsuleResult>,
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
            let committed = self.commit_live_run_status_with_completion(
                session_id.clone(),
                run_id,
                RunStatus::Failed,
                validation_error.to_string(),
                RunCompletionProjection {
                    output_contract,
                    result: result.clone(),
                    contract_violation: Some(validation_error.clone()),
                },
            )?;
            let mut records = committed.events;
            if let Some(sequence) = completion_event_sequence(&records) {
                records.extend(self.append_completion_result_receipt(
                    &run,
                    sequence,
                    output_contract,
                    result.as_ref(),
                    CompletionReceiptState::Quarantined,
                )?);
            }
            records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Failed)?);
            self.publish_records(&records);
            return Err(RunExecutionError::OutputContractViolation(validation_error));
        }

        let committed = self.commit_live_run_status_with_completion(
            session_id.clone(),
            run_id,
            RunStatus::Completed,
            detail,
            RunCompletionProjection {
                output_contract,
                result: result.clone(),
                contract_violation: None,
            },
        )?;
        let mut records = committed.events;
        if let Some(sequence) = completion_event_sequence(&records) {
            records.extend(self.append_completion_result_receipt(
                &run,
                sequence,
                output_contract,
                result.as_ref(),
                CompletionReceiptState::Promoted,
            )?);
        }
        records.extend(self.advance_ready_queue(&session_id, run_id, RunStatus::Completed)?);
        Ok(RunMutationResult {
            run: committed.run,
            events: records,
        })
    }

    fn append_completion_result_receipt(
        &self,
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
            | RunSource::Forked { parent_run_id, .. } => Some(parent_run_id.clone()),
            RunSource::User { .. } => None,
        };
        let agent_turn_id = match &run.source {
            RunSource::NativeSubagent { parent_turn_id, .. } => Some(parent_turn_id.clone()),
            RunSource::User { .. } | RunSource::Forked { .. } => None,
        };
        let provenance = ReceiptProvenance {
            artifact_id: None,
            event_seq: agent_turn_id.as_ref().map(|_| event_seq),
            agent_turn_id,
            stream_cursor: Some(format!("run:{}:event:{event_seq}", run.id.as_str())),
        };
        let receipt = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
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
            }
        };
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            store.commit_receipt_event(CommitReceiptEvent {
                session_id: run.session_id.clone(),
                event: DaemonEvent::ContextReceipt(match state {
                    CompletionReceiptState::Promoted => ContextReceiptEvent::Promoted { receipt },
                    CompletionReceiptState::Quarantined => {
                        ContextReceiptEvent::Quarantined { receipt }
                    }
                }),
                occurred_at_ms: current_time_ms(),
            })?
        };
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
