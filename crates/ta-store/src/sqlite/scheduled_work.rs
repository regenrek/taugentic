use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::ScheduledWorkOccurrenceState;

use super::*;
use crate::{
    ClaimScheduledWorkOccurrence, ReserveScheduledWorkOccurrence, ScheduledWorkClaimResult,
    ScheduledWorkRepository, SessionProjection, cleanup_pending_run_id, preparing_run_id,
    scheduled_claim_matches_definition,
};

impl ScheduledWorkRepository for SqliteStore {
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
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled_work_create_transaction",
                source,
            })?;
        tx.execute(
            "INSERT INTO scheduled_work_definitions (id, session_id, data_json) VALUES (?, ?, ?)",
            params![
                definition.id.as_str(),
                definition.session_id.as_str(),
                Self::encode("scheduled_work_definition", &definition)?
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "scheduled work",
            source,
        })?;
        tx.execute("INSERT INTO scheduled_work_occurrences (id, scheduled_work_id, run_id, state, data_json) VALUES (?, ?, NULL, 'pending', ?)", params![occurrence.id.as_str(), occurrence.scheduled_work_id.as_str(), Self::encode("scheduled_work_occurrence", &occurrence)?]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "scheduled_work_create_transaction",
            source,
        })
    }

    fn scheduled_work(
        &self,
        id: &ta_protocol::wire::ScheduledWorkId,
    ) -> Result<Option<ta_protocol::wire::ScheduledWorkDefinition>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM scheduled_work_definitions WHERE id = ?",
                [id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work",
                source,
            })?
            .map(|json| Self::decode("scheduled_work_definition", json))
            .transpose()
    }
    fn scheduled_work_occurrence(
        &self,
        id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
    ) -> Result<Option<ta_protocol::wire::ScheduledWorkOccurrence>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM scheduled_work_occurrences WHERE id = ?",
                [id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work occurrence",
                source,
            })?
            .map(|json| Self::decode("scheduled_work_occurrence", json))
            .transpose()
    }
    fn scheduled_work_occurrences(
        &self,
    ) -> Result<Vec<ta_protocol::wire::ScheduledWorkOccurrence>, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT data_json FROM scheduled_work_occurrences ORDER BY id")
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work occurrences",
                source,
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work occurrences",
                source,
            })?;
        rows.map(|row| {
            row.map_err(|source| StoreError::QueryStore {
                entity: "scheduled work occurrences",
                source,
            })
            .and_then(|json| Self::decode("scheduled_work_occurrence", json))
        })
        .collect()
    }

    fn reserve_scheduled_work_occurrence(
        &mut self,
        input: ReserveScheduledWorkOccurrence,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let mut occurrence = self
            .scheduled_work_occurrence(&input.occurrence_id)?
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
            run_id: input.run_id.clone(),
        };
        // `runs` has a foreign key. Preparing is intentionally not a run, so
        // its reservation lives solely in the durable occurrence JSON until
        // the atomic publication inserts the queued projection.
        let changed = self.conn.execute("UPDATE scheduled_work_occurrences SET run_id = NULL, state = 'preparing', data_json = ? WHERE id = ? AND state = 'pending'", params![Self::encode("scheduled_work_occurrence", &occurrence)?, occurrence.id.as_str()]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence.id.as_str().to_string(),
            });
        }
        Ok(occurrence)
    }

    fn publish_prepared_scheduled_work_occurrence(
        &mut self,
        input: ClaimScheduledWorkOccurrence,
    ) -> Result<ScheduledWorkClaimResult, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled_work_claim_transaction",
                source,
            })?;
        let definition_json: String = tx
            .query_row(
                "SELECT data_json FROM scheduled_work_definitions WHERE id = ?",
                [input.scheduled_work_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work",
                key: input.scheduled_work_id.as_str().to_string(),
            })?;
        let definition: ta_protocol::wire::ScheduledWorkDefinition =
            Self::decode("scheduled_work_definition", definition_json)?;
        let occurrence_json: String = tx
            .query_row(
                "SELECT data_json FROM scheduled_work_occurrences WHERE id = ?",
                [input.occurrence_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "scheduled work occurrence",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: input.occurrence_id.as_str().to_string(),
            })?;
        let mut occurrence: ta_protocol::wire::ScheduledWorkOccurrence =
            Self::decode("scheduled_work_occurrence", occurrence_json)?;
        if occurrence.scheduled_work_id != input.scheduled_work_id
            || preparing_run_id(&occurrence) != Some(&input.run.id)
        {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: input.occurrence_id.as_str().to_string(),
            });
        }
        let session_json: String = tx
            .query_row(
                "SELECT data_json FROM sessions WHERE id = ?",
                [definition.session_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "session",
                key: definition.session_id.as_str().to_string(),
            })?;
        let session: SessionProjection = Self::decode("session_projection", session_json)?;
        let workspace_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?)",
                [definition.execution_request.workspace_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        if !workspace_exists
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
        occurrence.state = ScheduledWorkOccurrenceState::Claimed {
            run_id: input.run.id.clone(),
        };
        tx.execute(
            "INSERT INTO runs (id, session_id, data_json, last_commit_id) VALUES (?, ?, ?, NULL)",
            params![
                input.run.id.as_str(),
                input.run.session_id.as_str(),
                Self::encode("run_projection", &input.run)?
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "run",
            source,
        })?;
        let expected = Self::encode(
            "scheduled_work_occurrence",
            &ta_protocol::wire::ScheduledWorkOccurrence {
                state: ScheduledWorkOccurrenceState::Preparing {
                    run_id: input.run.id.clone(),
                },
                ..occurrence.clone()
            },
        )?;
        let changed = tx.execute("UPDATE scheduled_work_occurrences SET run_id = ?, state = 'claimed', data_json = ? WHERE id = ? AND state = 'preparing' AND run_id IS NULL AND data_json = ?", params![input.run.id.as_str(), Self::encode("scheduled_work_occurrence", &occurrence)?, occurrence.id.as_str(), expected]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence.id.as_str().to_string(),
            });
        }
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "scheduled_work_claim_transaction",
            source,
        })?;
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
        let mut occurrence = self
            .scheduled_work_occurrence(occurrence_id)?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if preparing_run_id(&occurrence) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        occurrence.state = ScheduledWorkOccurrenceState::PreparationFailed {
            run_id: run_id.clone(),
            detail,
        };
        let expected = Self::encode(
            "scheduled_work_occurrence",
            &ta_protocol::wire::ScheduledWorkOccurrence {
                state: ScheduledWorkOccurrenceState::Preparing {
                    run_id: run_id.clone(),
                },
                ..occurrence.clone()
            },
        )?;
        let changed = self.conn.execute("UPDATE scheduled_work_occurrences SET state = 'preparation_failed', data_json = ? WHERE id = ? AND run_id IS NULL AND state = 'preparing' AND data_json = ?", params![Self::encode("scheduled_work_occurrence", &occurrence)?, occurrence_id.as_str(), expected]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        Ok(occurrence)
    }

    fn request_preparing_scheduled_work_cancellation(
        &mut self,
        occurrence_id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
        run_id: &ta_protocol::wire::RunId,
        resource: ta_protocol::wire::ScheduledWorkUnpublishedResource,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let current = self
            .scheduled_work_occurrence(occurrence_id)?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if preparing_run_id(&current) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        let mut occurrence = current.clone();
        occurrence.state = ScheduledWorkOccurrenceState::PreparationCancellationRequested {
            run_id: run_id.clone(),
            resource,
        };
        let changed = self.conn.execute("UPDATE scheduled_work_occurrences SET state = 'preparation_cancellation_requested', data_json = ? WHERE id = ? AND state = 'preparing' AND run_id IS NULL AND data_json = ?", params![Self::encode("scheduled_work_occurrence", &occurrence)?, occurrence_id.as_str(), Self::encode("scheduled_work_occurrence", &current)?]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        Ok(occurrence)
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
        let current = self
            .scheduled_work_occurrence(occurrence_id)?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "scheduled work occurrence",
                key: occurrence_id.as_str().to_string(),
            })?;
        if cleanup_pending_run_id(&current) != Some(run_id) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        let (intended_terminal, resource) = match &current.state {
            ScheduledWorkOccurrenceState::PreparationCancellationRequested { resource, .. } => (
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Cancelled,
                resource.clone(),
            ),
            ScheduledWorkOccurrenceState::Preparing { .. } => (intended_terminal, resource),
            _ => unreachable!("cleanup_pending_run_id validated the state"),
        };
        let mut occurrence = current.clone();
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
        let state = match occurrence.state {
            ScheduledWorkOccurrenceState::CleanupRequired { .. } => "cleanup_required",
            ScheduledWorkOccurrenceState::PreparationCancelled { .. } => "preparation_cancelled",
            _ => "preparation_failed",
        };
        let changed = self.conn.execute("UPDATE scheduled_work_occurrences SET state = ?, data_json = ? WHERE id = ? AND run_id IS NULL AND data_json = ?", params![state, Self::encode("scheduled_work_occurrence", &occurrence)?, occurrence_id.as_str(), Self::encode("scheduled_work_occurrence", &current)?]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: occurrence_id.as_str().to_string(),
            });
        }
        Ok(occurrence)
    }

    fn cancel_scheduled_work_occurrence(
        &mut self,
        id: &ta_protocol::wire::ScheduledWorkOccurrenceId,
    ) -> Result<ta_protocol::wire::ScheduledWorkOccurrence, StoreError> {
        let current =
            self.scheduled_work_occurrence(id)?
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "scheduled work occurrence",
                    key: id.as_str().to_string(),
                })?;
        if !matches!(current.state, ScheduledWorkOccurrenceState::Pending) {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: id.as_str().to_string(),
            });
        }
        let mut occurrence = current.clone();
        occurrence.state = ScheduledWorkOccurrenceState::Cancelled { run_id: None };
        let changed = self.conn.execute("UPDATE scheduled_work_occurrences SET state = 'cancelled', data_json = ? WHERE id = ? AND state = 'pending' AND data_json = ?", params![Self::encode("scheduled_work_occurrence", &occurrence)?, id.as_str(), Self::encode("scheduled_work_occurrence", &current)?]).map_err(|source| StoreError::QueryStore { entity: "scheduled work occurrence", source })?;
        if changed != 1 {
            return Err(StoreError::ScheduledWorkOccurrenceNotPending {
                occurrence_id: id.as_str().to_string(),
            });
        }
        Ok(occurrence)
    }
}
