use ta_store::{ArtifactRecord, PersistenceStore, RunEventRangeQuery};

use crate::orchestration::run_events_subscribe::{
    self, RUN_EVENT_REPLAY_BATCH_LIMIT, RunEventSubscription,
};
use crate::{
    ApprovalActor, ArtifactSummary, DaemonRunCompleteWithResultParams, PublicDaemonEvent,
    ResumeRunRequest, ResumeRunResult, RunEventDelta, RunSummary, StartRunCommand,
    SubscribeRunEventsRequest, SubscribeRunEventsResult,
};

use super::{
    AppDeferredMutationResult, AppService, AppServiceError, map_artifact_mutation_result,
    map_run_execution_error, map_run_mutation_result,
};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(dead_code)]
    pub fn record_artifact(
        &self,
        artifact: ArtifactRecord,
    ) -> Result<AppDeferredMutationResult<ArtifactSummary>, AppServiceError> {
        self.run_execution
            .record_artifact(artifact)
            .map(map_artifact_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub fn start_run(
        &self,
        session_id: &crate::SessionId,
        command: &StartRunCommand,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .start_run(session_id.clone(), command.clone())
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub fn resume_run(
        &self,
        session_id: &crate::SessionId,
        request: &ResumeRunRequest,
    ) -> Result<ResumeRunResult, AppServiceError> {
        self.run_execution
            .resume_run(session_id.clone(), request.clone())
            .map_err(map_run_execution_error)
    }

    pub fn complete_run_with_result(
        &self,
        session_id: &crate::SessionId,
        request: &DaemonRunCompleteWithResultParams,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .complete_run_with_result(
                session_id.clone(),
                &request.run_id,
                request.detail.clone(),
                request.result.clone(),
            )
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    pub fn replay_run_events(
        &self,
        session_id: &crate::SessionId,
        request: &SubscribeRunEventsRequest,
    ) -> Result<SubscribeRunEventsResult, AppServiceError> {
        if request.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                request.run_id.as_str().to_string(),
            ));
        }

        let store = self.store.lock().expect("app store should not be poisoned");
        let Some(run) = store.run(&request.run_id)? else {
            return Err(AppServiceError::RunNotFound(
                request.run_id.as_str().to_string(),
            ));
        };
        if run.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        let range = store.read_run_events(&RunEventRangeQuery {
            session_id: session_id.clone(),
            run_id: request.run_id.clone(),
            after_sequence: request.after_seq,
            limit: RUN_EVENT_REPLAY_BATCH_LIMIT,
        })?;
        Ok(SubscribeRunEventsResult {
            events: range
                .records
                .into_iter()
                .map(|record| RunEventDelta {
                    seq: record.sequence,
                    event: PublicDaemonEvent::from(record.payload),
                })
                .collect(),
            latest_event_seq: range.latest_sequence,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn subscribe_run_events(
        &self,
        session_id: &crate::SessionId,
        request: &SubscribeRunEventsRequest,
    ) -> Result<RunEventSubscription, AppServiceError> {
        run_events_subscribe::subscribe_run_events(self, session_id, request)
    }

    pub fn cancel_run(
        &self,
        session_id: &crate::SessionId,
        actor: &ApprovalActor,
        run_id: &crate::RunId,
        reason: Option<String>,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        self.run_execution
            .cancel_run(session_id.clone(), actor.clone(), run_id, reason)
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }

    #[cfg(test)]
    pub(crate) fn seed_running_run_for_tests(
        &self,
        session_id: &crate::SessionId,
        objective: &str,
        selection: &crate::AgentRuntimeSelection,
    ) -> Result<AppDeferredMutationResult<RunSummary>, AppServiceError> {
        let validated_selection = self
            .agent_runtime
            .validate_run_selection(selection)
            .map_err(AppServiceError::from)?;
        self.run_execution
            .seed_running_run_for_tests(
                session_id.clone(),
                objective.to_string(),
                validated_selection,
            )
            .map(map_run_mutation_result)
            .map_err(map_run_execution_error)
    }
}
