use std::time::{SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    CancelScheduledWorkRequest, CreateScheduledWorkRequest, CreateScheduledWorkResult,
    ListScheduledWorkResult, ScheduledWorkAttentionPolicy, ScheduledWorkDefinition,
    ScheduledWorkId, ScheduledWorkOccurrence, ScheduledWorkOccurrenceId,
    ScheduledWorkOccurrenceState,
};
use ta_store::PersistenceStore;
use uuid::Uuid;

use super::{AppService, AppServiceError, map_run_execution_error};

impl<S> AppService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) fn create_scheduled_work(
        &self,
        session_id: &crate::SessionId,
        request: CreateScheduledWorkRequest,
    ) -> Result<CreateScheduledWorkResult, AppServiceError> {
        let (route, execution_request) = self
            .run_execution
            .freeze_scheduled_work_execution(session_id, &request.selection)
            .map_err(map_run_execution_error)?;
        let definition = ScheduledWorkDefinition {
            id: ScheduledWorkId::new(format!("scheduled-work-{}", Uuid::new_v4().simple()))
                .expect("generated scheduled work id should be valid"),
            session_id: session_id.clone(),
            objective: request.objective,
            route,
            execution_request,
            due_at_ms: request.due_at_ms,
            attention_policy: ScheduledWorkAttentionPolicy::AttentionOnly,
        };
        definition
            .validate()
            .map_err(|_| AppServiceError::EmptyRunObjective)?;
        let occurrence = ScheduledWorkOccurrence {
            id: ScheduledWorkOccurrenceId::new(format!(
                "scheduled-occurrence-{}",
                Uuid::new_v4().simple()
            ))
            .expect("generated scheduled occurrence id should be valid"),
            scheduled_work_id: definition.id.clone(),
            due_at_ms: definition.due_at_ms,
            state: ScheduledWorkOccurrenceState::Pending,
        };
        self.store
            .lock()
            .expect("app store should not be poisoned")
            .create_scheduled_work(definition.clone(), occurrence.clone())?;
        Ok(CreateScheduledWorkResult {
            definition,
            occurrence,
        })
    }

    pub(crate) fn list_scheduled_work(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<ListScheduledWorkResult, AppServiceError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let occurrences = store
            .scheduled_work_occurrences()?
            .into_iter()
            .map(|occurrence| {
                let definition = store
                    .scheduled_work(&occurrence.scheduled_work_id)?
                    .ok_or_else(|| ta_store::StoreError::MissingRecord {
                        entity: "scheduled work",
                        key: occurrence.scheduled_work_id.as_str().to_string(),
                    })?;
                Ok((definition.session_id == *session_id).then_some(occurrence))
            })
            .collect::<Result<Vec<_>, ta_store::StoreError>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(ListScheduledWorkResult { occurrences })
    }

    pub(crate) fn cancel_scheduled_work(
        &self,
        session_id: &crate::SessionId,
        actor: &crate::ApprovalActor,
        request: &CancelScheduledWorkRequest,
    ) -> Result<(), AppServiceError> {
        let occurrence = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work_occurrence(&request.occurrence_id)?
            .ok_or_else(|| {
                AppServiceError::Store(ta_store::StoreError::MissingRecord {
                    entity: "scheduled work occurrence",
                    key: request.occurrence_id.as_str().to_string(),
                })
            })?;
        let definition = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work(&occurrence.scheduled_work_id)?
            .ok_or_else(|| {
                AppServiceError::Store(ta_store::StoreError::MissingRecord {
                    entity: "scheduled work",
                    key: occurrence.scheduled_work_id.as_str().to_string(),
                })
            })?;
        if definition.session_id != *session_id {
            return Err(AppServiceError::RunSessionMismatch(
                request.occurrence_id.as_str().to_string(),
            ));
        }
        match occurrence.state {
            ScheduledWorkOccurrenceState::Pending => {
                self.store
                    .lock()
                    .expect("app store should not be poisoned")
                    .cancel_scheduled_work_occurrence(&occurrence.id)?;
            }
            ScheduledWorkOccurrenceState::Preparing { ref run_id }
            | ScheduledWorkOccurrenceState::PreparationCancellationRequested {
                ref run_id, ..
            } => {
                self.run_execution
                    .request_scheduled_work_cancellation(&definition.id, &occurrence.id, run_id)
                    .map_err(map_run_execution_error)?;
            }
            ScheduledWorkOccurrenceState::Claimed { ref run_id } => {
                self.run_execution
                    .cancel_run(
                        session_id.clone(),
                        actor.clone(),
                        run_id,
                        Some("Scheduled work cancelled".to_string()),
                    )
                    .map_err(map_run_execution_error)?;
            }
            _ => {
                return Err(AppServiceError::RunNotCancellable(
                    request.occurrence_id.as_str().to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Called only by the daemon's deadline wake. Each reservation is atomic,
    /// so overdue work is claimed once even across a restart boundary.
    pub(crate) fn process_due_scheduled_work(&self) -> Result<(), AppServiceError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppServiceError::SystemClockBeforeUnixEpoch)?
            .as_millis() as u64;
        let due = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work_occurrences()?
            .into_iter()
            .filter(|occurrence| {
                occurrence.due_at_ms <= now
                    && matches!(occurrence.state, ScheduledWorkOccurrenceState::Pending)
            })
            .collect::<Vec<_>>();
        for occurrence in due {
            self.process_due_scheduled_work_occurrence(occurrence)?;
        }
        Ok(())
    }

    /// Completes one snapshot selected by the deadline owner. A concurrent
    /// cancellation or preparation terminalization can make that snapshot
    /// stale before reservation reaches the store. That exact occurrence is
    /// already durably settled, so it must not terminate the coordinator.
    /// Unresolved Pending/Preparing failures remain fatal to the caller.
    pub(super) fn process_due_scheduled_work_occurrence(
        &self,
        occurrence: ScheduledWorkOccurrence,
    ) -> Result<(), AppServiceError> {
        let run_id = crate::RunId::new(format!("run-scheduled-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let run = match self
            .run_execution
            .prepare_and_publish_scheduled_work(
                occurrence.scheduled_work_id,
                occurrence.id.clone(),
                run_id,
            )
            .map_err(map_run_execution_error)
        {
            Ok(run) => run,
            Err(_error) if self.scheduled_work_occurrence_is_terminalized(&occurrence.id)? => {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        self.run_execution
            .schedule_published_scheduled_work(&run)
            .map_err(map_run_execution_error)
    }

    fn scheduled_work_occurrence_is_terminalized(
        &self,
        occurrence_id: &ScheduledWorkOccurrenceId,
    ) -> Result<bool, AppServiceError> {
        let occurrence = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work_occurrence(occurrence_id)?
            .ok_or_else(|| {
                AppServiceError::Store(ta_store::StoreError::MissingRecord {
                    entity: "scheduled work occurrence",
                    key: occurrence_id.as_str().to_string(),
                })
            })?;
        Ok(!matches!(
            occurrence.state,
            ScheduledWorkOccurrenceState::Pending | ScheduledWorkOccurrenceState::Preparing { .. }
        ))
    }

    pub(crate) fn next_scheduled_work_deadline_ms(&self) -> Result<Option<u64>, AppServiceError> {
        Ok(self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .scheduled_work_occurrences()?
            .into_iter()
            .filter_map(|occurrence| {
                matches!(occurrence.state, ScheduledWorkOccurrenceState::Pending)
                    .then_some(occurrence.due_at_ms)
            })
            .min())
    }
}
