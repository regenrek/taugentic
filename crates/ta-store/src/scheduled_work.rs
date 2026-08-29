use ta_protocol::wire::{
    RunId, RunStatus, ScheduledWorkDefinition, ScheduledWorkId, ScheduledWorkOccurrence,
    ScheduledWorkOccurrenceId, ScheduledWorkPreparationTerminal, ScheduledWorkUnpublishedResource,
};

use crate::RunProjection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimScheduledWorkOccurrence {
    pub scheduled_work_id: ScheduledWorkId,
    pub occurrence_id: ScheduledWorkOccurrenceId,
    pub run: RunProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveScheduledWorkOccurrence {
    pub scheduled_work_id: ScheduledWorkId,
    pub occurrence_id: ScheduledWorkOccurrenceId,
    pub run_id: RunId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledWorkClaimResult {
    pub definition: ScheduledWorkDefinition,
    pub occurrence: ScheduledWorkOccurrence,
    pub run: RunProjection,
}

pub trait ScheduledWorkRepository {
    fn create_scheduled_work(
        &mut self,
        definition: ScheduledWorkDefinition,
        occurrence: ScheduledWorkOccurrence,
    ) -> Result<(), crate::StoreError>;
    fn scheduled_work(
        &self,
        scheduled_work_id: &ScheduledWorkId,
    ) -> Result<Option<ScheduledWorkDefinition>, crate::StoreError>;
    fn scheduled_work_occurrence(
        &self,
        occurrence_id: &ScheduledWorkOccurrenceId,
    ) -> Result<Option<ScheduledWorkOccurrence>, crate::StoreError>;
    fn scheduled_work_occurrences(&self)
    -> Result<Vec<ScheduledWorkOccurrence>, crate::StoreError>;
    fn reserve_scheduled_work_occurrence(
        &mut self,
        input: ReserveScheduledWorkOccurrence,
    ) -> Result<ScheduledWorkOccurrence, crate::StoreError>;
    fn publish_prepared_scheduled_work_occurrence(
        &mut self,
        input: ClaimScheduledWorkOccurrence,
    ) -> Result<ScheduledWorkClaimResult, crate::StoreError>;
    fn fail_preparing_scheduled_work_occurrence(
        &mut self,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
        detail: String,
    ) -> Result<ScheduledWorkOccurrence, crate::StoreError>;
    fn request_preparing_scheduled_work_cancellation(
        &mut self,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
        resource: ScheduledWorkUnpublishedResource,
    ) -> Result<ScheduledWorkOccurrence, crate::StoreError>;
    fn finalize_preparing_scheduled_work_cleanup(
        &mut self,
        occurrence_id: &ScheduledWorkOccurrenceId,
        run_id: &RunId,
        intended_terminal: ScheduledWorkPreparationTerminal,
        resource: ScheduledWorkUnpublishedResource,
        preparation_detail: String,
        cleanup_result: Result<(), String>,
    ) -> Result<ScheduledWorkOccurrence, crate::StoreError>;
    fn cancel_scheduled_work_occurrence(
        &mut self,
        occurrence_id: &ScheduledWorkOccurrenceId,
    ) -> Result<ScheduledWorkOccurrence, crate::StoreError>;
}

pub(crate) fn scheduled_run_source(
    run: &RunProjection,
) -> Option<(&ScheduledWorkId, &ScheduledWorkOccurrenceId)> {
    match &run.source {
        ta_protocol::wire::RunSource::ScheduledWork {
            scheduled_work_id,
            occurrence_id,
            ..
        } => Some((scheduled_work_id, occurrence_id)),
        _ => None,
    }
}

pub(crate) fn scheduled_claim_matches_definition(
    definition: &ScheduledWorkDefinition,
    run: &RunProjection,
    scheduled_work_id: &ScheduledWorkId,
    occurrence_id: &ScheduledWorkOccurrenceId,
) -> bool {
    let Some((run_work_id, run_occurrence_id)) = scheduled_run_source(run) else {
        return false;
    };
    run_work_id == scheduled_work_id
        && run_occurrence_id == occurrence_id
        && run.status == RunStatus::Queued
        && run.session_id == definition.session_id
        && run.objective == definition.objective
        && run.source.route() == &definition.route
        && run.runtime_profile_id == definition.route.runtime_profile_id
        && run.harness == definition.route.harness
        && definition
            .execution_request
            .matches_execution_context(&run.execution_context)
}

pub(crate) fn claimed_run_id(occurrence: &ScheduledWorkOccurrence) -> Option<&RunId> {
    match &occurrence.state {
        ta_protocol::wire::ScheduledWorkOccurrenceState::Claimed { run_id } => Some(run_id),
        _ => None,
    }
}

pub(crate) fn preparing_run_id(occurrence: &ScheduledWorkOccurrence) -> Option<&RunId> {
    match &occurrence.state {
        ta_protocol::wire::ScheduledWorkOccurrenceState::Preparing { run_id } => Some(run_id),
        _ => None,
    }
}

pub(crate) fn cleanup_pending_run_id(occurrence: &ScheduledWorkOccurrence) -> Option<&RunId> {
    match &occurrence.state {
        ta_protocol::wire::ScheduledWorkOccurrenceState::Preparing { run_id }
        | ta_protocol::wire::ScheduledWorkOccurrenceState::PreparationCancellationRequested {
            run_id,
            ..
        } => Some(run_id),
        _ => None,
    }
}
