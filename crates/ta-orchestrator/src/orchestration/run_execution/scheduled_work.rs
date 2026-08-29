use ta_protocol::wire::{
    ApprovalActor, RunId, RunSource, RunStatus, ScheduledWorkId, ScheduledWorkOccurrenceId,
    ScheduledWorkPreparationTerminal,
};
use ta_store::{
    ClaimScheduledWorkOccurrence, PersistenceStore, ReserveScheduledWorkOccurrence, RunProjection,
};

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    /// Enters the established per-session scheduler only after preparation has
    /// atomically published a queued run.  Keeping this separate from
    /// preparation is the boundary that prevents an unpublished occurrence
    /// from ever reaching a provider.
    pub(crate) fn schedule_published_scheduled_work(
        &self,
        run: &RunProjection,
    ) -> Result<(), RunExecutionError> {
        let disposition = match self
            .runtime
            .schedule_run_start(&run.session_id, run.id.clone())
        {
            Ok(disposition) => disposition,
            Err(crate::RunSchedulerError::QueueFull(_)) => {
                // The occurrence is already atomically linked to this queued
                // run. Converge both projections through the established run
                // terminal transition instead of leaving an unregistered run.
                self.cancel_run(
                    run.session_id.clone(),
                    ApprovalActor::new("taugentic-daemon")
                        .expect("daemon actor id should be valid"),
                    &run.id,
                    Some("Scheduled work queue is full".to_string()),
                )?;
                return Ok(());
            }
        };
        if matches!(disposition, crate::RunScheduleDisposition::StartNow) {
            self.promote_queued_run(run.session_id.clone(), &run.id)?;
        }
        Ok(())
    }

    /// Reserves one pending occurrence before creating any run-scoped
    /// resource, then publishes its fully prepared queued projection through
    /// the repository's single atomic claim operation.  This deliberately
    /// does not enter the scheduler or dispatch a provider.
    pub(crate) fn prepare_and_publish_scheduled_work(
        &self,
        scheduled_work_id: ScheduledWorkId,
        occurrence_id: ScheduledWorkOccurrenceId,
        run_id: RunId,
    ) -> Result<RunProjection, RunExecutionError> {
        let definition = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let definition = store.scheduled_work(&scheduled_work_id)?.ok_or_else(|| {
                ta_store::StoreError::MissingRecord {
                    entity: "scheduled work",
                    key: scheduled_work_id.as_str().to_string(),
                }
            })?;
            store.reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                scheduled_work_id: scheduled_work_id.clone(),
                occurrence_id: occurrence_id.clone(),
                run_id: run_id.clone(),
            })?;
            definition
        };

        let prepared = match self.prepare_scheduled_execution_context(
            &definition.session_id,
            &run_id,
            &definition.execution_request,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.terminalize_unpublished_scheduled_work(
                    &definition,
                    &occurrence_id,
                    &run_id,
                    error.to_string(),
                )?;
                return Err(error);
            }
        };
        let run = RunProjection {
            id: run_id.clone(),
            session_id: definition.session_id.clone(),
            runtime_profile_id: definition.route.runtime_profile_id.clone(),
            objective: definition.objective.clone(),
            status: RunStatus::Queued,
            harness: definition.route.harness,
            source: RunSource::ScheduledWork {
                route: definition.route.clone(),
                scheduled_work_id: scheduled_work_id.clone(),
                occurrence_id: occurrence_id.clone(),
            },
            execution_context: prepared.execution_context,
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: prepared.workspace_info,
            claimed_files: prepared.claimed_files,
            conflict_summary: prepared.conflict_summary,
        };
        let published = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                scheduled_work_id,
                occurrence_id: occurrence_id.clone(),
                run: run.clone(),
            });
        match published {
            Ok(claim) => Ok(claim.run),
            Err(error) => {
                self.terminalize_unpublished_scheduled_work(
                    &definition,
                    &occurrence_id,
                    &run_id,
                    error.to_string(),
                )?;
                Err(error.into())
            }
        }
    }

    fn terminalize_unpublished_scheduled_work(
        &self,
        definition: &ta_protocol::wire::ScheduledWorkDefinition,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
        detail: String,
    ) -> Result<(), RunExecutionError> {
        let resource = self.unpublished_scheduled_resource(
            run_id,
            definition.execution_request.repo_root.as_path(),
            definition.execution_request.cleanup_policy,
        )?;
        let cleanup = self
            .discard_unpublished_scheduled_resources(
                run_id,
                definition.execution_request.repo_root.as_path(),
            )
            .map_err(|error| error.to_string());
        let cleanup_failure = cleanup.as_ref().err().cloned();
        self.store
            .lock()
            .expect("app store should not be poisoned")
            // The repository derives the terminal intent from its exact
            // durable pre-publication state. A cancellation recorded while
            // preparation was running must win over this failure path.
            .finalize_preparing_scheduled_work_cleanup(
                occurrence_id,
                run_id,
                ScheduledWorkPreparationTerminal::Failed,
                resource,
                detail,
                cleanup,
            )?;
        if let Some(cleanup_detail) = cleanup_failure {
            return Err(RunExecutionError::ProviderExecutionFailed(format!(
                "scheduled preparation cleanup requires intervention: {cleanup_detail}"
            )));
        }
        Ok(())
    }

    pub(crate) fn request_scheduled_work_cancellation(
        &self,
        scheduled_work_id: &ScheduledWorkId,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
    ) -> Result<(), RunExecutionError> {
        let definition = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work(scheduled_work_id)?
            .ok_or_else(|| ta_store::StoreError::MissingRecord {
                entity: "scheduled work",
                key: scheduled_work_id.as_str().to_string(),
            })?;
        let resource = self.unpublished_scheduled_resource(
            run_id,
            definition.execution_request.repo_root.as_path(),
            definition.execution_request.cleanup_policy,
        )?;
        self.store
            .lock()
            .expect("app store should not be poisoned")
            .request_preparing_scheduled_work_cancellation(occurrence_id, run_id, resource)?;
        Ok(())
    }

    pub(crate) fn reconcile_unpublished_scheduled_work(
        &self,
        definition: &ta_protocol::wire::ScheduledWorkDefinition,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
        intended_terminal: ScheduledWorkPreparationTerminal,
        detail: String,
    ) -> Result<(), RunExecutionError> {
        let resource = self.unpublished_scheduled_resource(
            run_id,
            definition.execution_request.repo_root.as_path(),
            definition.execution_request.cleanup_policy,
        )?;
        let cleanup = self
            .discard_unpublished_scheduled_resources(
                run_id,
                definition.execution_request.repo_root.as_path(),
            )
            .map_err(|error| error.to_string());
        self.store
            .lock()
            .expect("app store should not be poisoned")
            .finalize_preparing_scheduled_work_cleanup(
                occurrence_id,
                run_id,
                intended_terminal,
                resource,
                detail,
                cleanup,
            )?;
        Ok(())
    }
}
