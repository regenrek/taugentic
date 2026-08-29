use ta_protocol::wire::ScheduledWorkOccurrenceState;

use super::*;
use crate::{
    ClaimScheduledWorkOccurrence, ReserveScheduledWorkOccurrence, ScheduledWorkClaimResult,
    ScheduledWorkRepository, cleanup_pending_run_id, preparing_run_id,
    scheduled_claim_matches_definition,
};

impl ScheduledWorkRepository for InMemoryStore {
    fn create_scheduled_work(
        &mut self,
        definition: ta_protocol::wire::ScheduledWorkDefinition,
        occurrence: ta_protocol::wire::ScheduledWorkOccurrence,
    ) -> Result<(), StoreError> {
        definition
            .validate()
            .map_err(|error| StoreError::ScheduledWorkValidation {
                detail: error.to_string(),
            })?;
        if occurrence.scheduled_work_id != definition.id
            || occurrence.due_at_ms != definition.due_at_ms
            || !matches!(occurrence.state, ScheduledWorkOccurrenceState::Pending)
        {
            return Err(StoreError::ScheduledWorkValidation {
                detail: "definition and one-shot pending occurrence must agree".to_string(),
            });
        }
        if self.scheduled_work_definitions.contains_key(&definition.id) {
            return Err(StoreError::DuplicateRecord {
                entity: "scheduled work",
                key: definition.id.as_str().to_string(),
            });
        }
        if self.scheduled_work_occurrences.contains_key(&occurrence.id) {
            return Err(StoreError::DuplicateRecord {
                entity: "scheduled work occurrence",
                key: occurrence.id.as_str().to_string(),
            });
        }
        self.scheduled_work_definitions
            .insert(definition.id.clone(), definition);
        self.scheduled_work_occurrences
            .insert(occurrence.id.clone(), occurrence);
        Ok(())
    }

    fn scheduled_work(
        &self,
        id: &ta_protocol::wire::ScheduledWorkId,
    ) -> Result<Option<ta_protocol::wire::ScheduledWorkDefinition>, StoreError> {
        Ok(self.scheduled_work_definitions.get(id).cloned())
    }
    fn scheduled_work_occurrence(
        &self,
        id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
    ) -> Result<Option<ta_protocol::wire::ScheduledWorkOccurrence>, StoreError> {
        Ok(self.scheduled_work_occurrences.get(id).cloned())
    }
    fn scheduled_work_occurrences(
        &self,
    ) -> Result<Vec<ta_protocol::wire::ScheduledWorkOccurrence>, StoreError> {
        Ok(self.scheduled_work_occurrences.values().cloned().collect())
    }

    fn reserve_scheduled_work_occurrence(
        &mut self,
        input: ReserveScheduledWorkOccurrence,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let occurrence = self
            .scheduled_work_occurrences
            .get_mut(&input.occurrence_id)
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: input.occurrence_id.as_str().to_string(),
            })?;
        if occurrence.scheduled_work_id != input.scheduled_work_id
            || !matches!(occurrence.state, ScheduledWorkOccurrenceState::Pending)
        {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: input.occurrence_id.as_str().to_string(),
            });
        }
        occurrence.state = ScheduledWorkOccurrenceState::Preparing {
            run_id: input.run_id,
        };
        Ok(occurrence.clone())
    }

    fn publish_prepared_scheduled_work_occurrence(
        &mut self,
        input: ClaimScheduledWorkOccurrence,
    ) -> Result<ScheduledWorkClaimResult, StoreError> {
        let definition = self
            .scheduled_work_definitions
            .get(&input.scheduled_work_id)
            .cloned()
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work",
                key: input.scheduled_work_id.as_str().to_string(),
            })?;
        let mut occurrence = self
            .scheduled_work_occurrences
            .get(&input.occurrence_id)
            .cloned()
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: input.occurrence_id.as_str().to_string(),
            })?;
        if occurrence.scheduled_work_id != input.scheduled_work_id
            || preparing_run_id(&occurrence) != Some(&input.run.id)
        {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: input.occurrence_id.as_str().to_string(),
            });
        }
        let session =
            self.sessions
                .get(&definition.session_id)
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "session",
                    key: definition.session_id.as_str().to_string(),
                })?;
        if !self
            .workspaces
            .contains_key(&definition.execution_request.workspace_id)
            || session.workspace_id != definition.execution_request.workspace_id
            || !scheduled_claim_matches_definition(
                &definition,
                &input.run,
                &input.scheduled_work_id,
                &input.occurrence_id,
            )
        {
            return Err(StoreError::ScheduledWorkRunSourceMismatch {
                occurrence_id: input.occurrence_id.as_str().to_string(),
            });
        }
        if self.runs.contains_key(&input.run.id) {
            return Err(StoreError::DuplicateRecord {
                entity: "run",
                key: input.run.id.as_str().to_string(),
            });
        }
        occurrence.state = ScheduledWorkOccurrenceState::Claimed {
            run_id: input.run.id.clone(),
        };
        self.scheduled_work_occurrences
            .insert(occurrence.id.clone(), occurrence.clone());
        self.runs.insert(input.run.id.clone(), input.run.clone());
        Ok(ScheduledWorkClaimResult {
            definition,
            occurrence,
            run: input.run,
        })
    }

    fn fail_preparing_scheduled_work_occurrence(
        &mut self,
        occurrence_id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
        run_id: &ta_protocol::wire::RunId,
        detail: String,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let occurrence = self
            .scheduled_work_occurrences
            .get_mut(occurrence_id)
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if preparing_run_id(occurrence) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        occurrence.state = ScheduledWorkOccurrenceState::PreparationFailed {
            run_id: run_id.clone(),
            detail,
        };
        Ok(occurrence.clone())
    }

    fn request_preparing_scheduled_work_cancellation(
        &mut self,
        occurrence_id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
        run_id: &ta_protocol::wire::RunId,
        resource: ta_protocol::wire::ScheduledWorkUnpublishedResource,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let occurrence = self
            .scheduled_work_occurrences
            .get_mut(occurrence_id)
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if preparing_run_id(occurrence) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        occurrence.state = ScheduledWorkOccurrenceState::PreparationCancellationRequested {
            run_id: run_id.clone(),
            resource,
        };
        Ok(occurrence.clone())
    }

    fn finalize_preparing_scheduled_work_cleanup(
        &mut self,
        occurrence_id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
        run_id: &ta_protocol::wire::RunId,
        intended_terminal: ta_protocol::wire::ScheduledWorkPreparationTerminal,
        resource: ta_protocol::wire::ScheduledWorkUnpublishedResource,
        preparation_detail: String,
        cleanup_result: Result<(), String>,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let occurrence = self
            .scheduled_work_occurrences
            .get_mut(occurrence_id)
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if cleanup_pending_run_id(occurrence) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        let (intended_terminal, resource) = match &occurrence.state {
            ScheduledWorkOccurrenceState::PreparationCancellationRequested { resource, .. } => (
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Cancelled,
                resource.clone(),
            ),
            ScheduledWorkOccurrenceState::Preparing { .. } => (intended_terminal, resource),
            _ => unreachable!("cleanup_pending_run_id validated the state"),
        };
        occurrence.state = match cleanup_result {
            Ok(()) => match intended_terminal {
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed => {
                    ScheduledWorkOccurrenceState::PreparationFailed {
                        run_id: run_id.clone(),
                        detail: preparation_detail,
                    }
                }
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Cancelled => {
                    ScheduledWorkOccurrenceState::PreparationCancelled {
                        run_id: run_id.clone(),
                    }
                }
            },
            Err(cleanup_detail) => ScheduledWorkOccurrenceState::CleanupRequired {
                run_id: run_id.clone(),
                resource,
                intended_terminal,
                preparation_detail,
                cleanup_detail,
            },
        };
        Ok(occurrence.clone())
    }

    fn cancel_scheduled_work_occurrence(
        &mut self,
        id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let occurrence = self.scheduled_work_occurrences.get_mut(id).ok_or_else(|| {
            StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: id.as_str().to_string(),
            }
        })?;
        occurrence.state = match &occurrence.state {
            ScheduledWorkOccurrenceState::Pending => {
                ScheduledWorkOccurrenceState::Cancelled { run_id: None }
            }
            ScheduledWorkOccurrenceState::Preparing { .. } => {
                return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                    occurrence_id: id.as_str().to_string(),
                });
            }
            _ => {
                return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                    occurrence_id: id.as_str().to_string(),
                });
            }
        };
        Ok(occurrence.clone())
    }
}
