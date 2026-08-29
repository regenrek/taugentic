use super::*;
use crate::{
    CommitSessionNextRunSelection, CommitSessionOpenWithNavigation,
    SessionNextRunSelectionCommitResult, UserTurnCommit, user_row,
};

fn scheduled_terminal_occurrence_tx(
    tx: &rusqlite::Transaction<'_>,
    run: &RunProjection,
) -> Result<
    Option<(
        ta_protocol::wire::ScheduledWorkOccurrenceId,
        ta_protocol::wire::ScheduledWorkOccurrenceState,
        String,
    )>,
    StoreError,
> {
    crate::scheduled_work::scheduled_run_source(run)
        .and_then(|(_, occurrence_id)| {
            crate::scheduled_terminal_state(run.id.clone(), run.status)
                .map(|state| (occurrence_id.clone(), state))
        })
        .map(|(occurrence_id, state)| {
            let occurrence_json: String = tx
                .query_row(
                    "SELECT data_json FROM scheduled_work_occurrences WHERE id = ?",
                    [occurrence_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|source| StoreError::QueryStore {
                    entity: "scheduled work occurrence",
                    source,
                })?
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "scheduled work occurrence",
                    key: occurrence_id.as_str().to_string(),
                })?;
            let occurrence: ta_protocol::wire::ScheduledWorkOccurrence =
                SqliteStore::decode("scheduled_work_occurrence", occurrence_json.clone())?;
            if crate::claimed_run_id(&occurrence) != Some(&run.id) {
                return Err(StoreError::ScheduledWorkOccurrenceClaimMismatch {
                    occurrence_id: occurrence_id.as_str().to_string(),
                    run_id: run.id.as_str().to_string(),
                });
            }
            Ok((occurrence_id, state, occurrence_json))
        })
        .transpose()
}

fn settle_scheduled_terminal_occurrence_tx(
    tx: &rusqlite::Transaction<'_>,
    run: &RunProjection,
) -> Result<(), StoreError> {
    let Some((occurrence_id, state, expected_claimed_json)) =
        scheduled_terminal_occurrence_tx(tx, run)?
    else {
        return Ok(());
    };
    let state_name = match state {
        ta_protocol::wire::ScheduledWorkOccurrenceState::Completed { .. } => "completed",
        ta_protocol::wire::ScheduledWorkOccurrenceState::Failed { .. } => "failed",
        ta_protocol::wire::ScheduledWorkOccurrenceState::BudgetExceeded { .. } => "budget_exceeded",
        ta_protocol::wire::ScheduledWorkOccurrenceState::Cancelled { .. } => "cancelled",
        ta_protocol::wire::ScheduledWorkOccurrenceState::Pending
        | ta_protocol::wire::ScheduledWorkOccurrenceState::Preparing { .. }
        | ta_protocol::wire::ScheduledWorkOccurrenceState::PreparationCancellationRequested {
            ..
        }
        | ta_protocol::wire::ScheduledWorkOccurrenceState::Claimed { .. } => {
            unreachable!("terminal state only")
        }
        ta_protocol::wire::ScheduledWorkOccurrenceState::PreparationFailed { .. } => {
            "preparation_failed"
        }
        ta_protocol::wire::ScheduledWorkOccurrenceState::PreparationCancelled { .. } => {
            "preparation_cancelled"
        }
        ta_protocol::wire::ScheduledWorkOccurrenceState::CleanupRequired { .. } => {
            "cleanup_required"
        }
    };
    let mut occurrence: ta_protocol::wire::ScheduledWorkOccurrence =
        SqliteStore::decode("scheduled_work_occurrence", expected_claimed_json.clone())?;
    occurrence.state = state;
    let changed = tx
        .execute(
            "UPDATE scheduled_work_occurrences SET state = ?, data_json = ? WHERE id = ? AND state = 'claimed' AND run_id = ? AND data_json = ?",
            params![
                state_name,
                SqliteStore::encode("scheduled_work_occurrence", &occurrence)?,
                occurrence_id.as_str(),
                run.id.as_str(),
                expected_claimed_json
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "scheduled work occurrence",
            source,
        })?;
    if changed != 1 {
        return Err(StoreError::ScheduledWorkOccurrenceClaimMismatch {
            occurrence_id: occurrence_id.as_str().to_string(),
            run_id: run.id.as_str().to_string(),
        });
    }
    Ok(())
}

impl CommitRepository for SqliteStore {
    fn commit_session_open_with_navigation(
        &mut self,
        input: CommitSessionOpenWithNavigation,
    ) -> Result<SessionOpenCommitResult, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "session_navigation_transaction",
                source,
            })?;
        let workspace_exists: i64 = tx
            .query_row(
                "SELECT COUNT(1) FROM workspaces WHERE id = ?",
                [input.session.workspace_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        if workspace_exists == 0 {
            return Err(StoreError::SessionWorkspaceMissing {
                workspace_id: input.session.workspace_id.as_str().to_string(),
            });
        }
        tx.execute(
            "INSERT INTO sessions (id, data_json, workspace_id, last_commit_id) VALUES (?, ?, ?, NULL)",
            params![
                input.session.id.as_str(),
                Self::encode("session_projection", &input.session)?,
                input.session.workspace_id.as_str(),
            ],
        )
        .map_err(|source| StoreError::QueryStore { entity: "session", source })?;
        let session_id = input.session.id.clone();
        let payload = DaemonEvent::Session(ta_protocol::wire::SessionEvent {
            session_id: session_id.clone(),
            status: input.session.status,
        });
        let sequence = self.next_runtime_sequence;
        tx.execute(
            "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json) VALUES (?, ?, ?, ?)",
            params![
                sequence as i64,
                input.session.id.as_str(),
                input.occurred_at_ms as i64,
                Self::encode("daemon_event", &payload)?,
            ],
        )
        .map_err(|source| StoreError::QueryStore { entity: "event", source })?;
        tx.execute(
            "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence) VALUES (?, ?, ?, ?, ?)",
            params![
                input.session.id.as_str(),
                "session_open",
                input.occurred_at_ms as i64,
                sequence as i64,
                sequence as i64,
            ],
        )
        .map_err(|source| StoreError::QueryStore { entity: "commit", source })?;
        let commit_id = tx.last_insert_rowid() as u64;
        tx.execute(
            "UPDATE sessions SET last_commit_id = ? WHERE id = ?",
            params![commit_id as i64, input.session.id.as_str()],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "session",
            source,
        })?;
        tx.execute(
            "INSERT INTO navigation_states (owner_principal_id, data_json) VALUES (?, ?) ON CONFLICT(owner_principal_id) DO UPDATE SET data_json = excluded.data_json",
            params![
                input.owner_principal_id,
                Self::encode("navigation_state", &input.navigation)?,
            ],
        )
        .map_err(|source| StoreError::QueryStore { entity: "navigation_state", source })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "session_navigation_transaction",
            source,
        })?;
        self.next_runtime_sequence = self.next_runtime_sequence.saturating_add(1);
        Ok(SessionOpenCommitResult {
            commit: CommitBoundary {
                id: commit_id,
                first_sequence: sequence,
                last_sequence: sequence,
            },
            session: input.session,
            event: EventRecord {
                sequence,
                session_id,
                occurred_at_ms: input.occurred_at_ms,
                payload,
            },
        })
    }

    fn commit_session_next_run_selection(
        &mut self,
        input: CommitSessionNextRunSelection,
    ) -> Result<SessionNextRunSelectionCommitResult, StoreError> {
        let existing_json: String = self
            .conn
            .query_row(
                "SELECT data_json FROM sessions WHERE id = ?",
                [input.session_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "session",
                key: input.session_id.as_str().to_string(),
            })?;
        let existing: SessionProjection = Self::decode("session_projection", existing_json)?;
        let session = SessionProjection {
            next_run_selection: input.selection,
            ..existing
        };
        self.conn
            .execute(
                "UPDATE sessions SET data_json = ? WHERE id = ?",
                params![
                    Self::encode("session_projection", &session)?,
                    session.id.as_str(),
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?;
        Ok(SessionNextRunSelectionCommitResult { session })
    }

    fn commit_session_open(
        &mut self,
        input: CommitSessionOpen,
    ) -> Result<SessionOpenCommitResult, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "session_transaction",
                source,
            })?;
        let workspace_exists: i64 = tx
            .query_row(
                "SELECT COUNT(1) FROM workspaces WHERE id = ?",
                [input.session.workspace_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "workspace",
                source,
            })?;
        if workspace_exists == 0 {
            return Err(StoreError::SessionWorkspaceMissing {
                workspace_id: input.session.workspace_id.as_str().to_string(),
            });
        }
        tx.execute(
            "INSERT OR IGNORE INTO sessions (id, data_json, workspace_id, last_commit_id)
             VALUES (?, ?, ?, NULL)",
            params![
                input.session.id.as_str(),
                Self::encode("session_projection", &input.session)?,
                input.session.workspace_id.as_str()
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "session",
            source,
        })?;
        let session_id = input.session.id.clone();
        let payload = DaemonEvent::Session(ta_protocol::wire::SessionEvent {
            session_id: session_id.clone(),
            status: input.session.status,
        });
        let sequence = self.next_runtime_sequence;
        tx.execute(
            "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json)
             VALUES (?, ?, ?, ?)",
            params![
                sequence as i64,
                input.session.id.as_str(),
                input.occurred_at_ms as i64,
                Self::encode("daemon_event", &payload)?
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "event",
            source,
        })?;
        tx.execute(
            "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
             VALUES (?, ?, ?, ?, ?)",
            params![
                input.session.id.as_str(),
                "session_open",
                input.occurred_at_ms as i64,
                sequence as i64,
                sequence as i64
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "commit",
            source,
        })?;
        let commit_id = tx.last_insert_rowid() as u64;
        tx.execute(
            "UPDATE sessions SET last_commit_id = ? WHERE id = ?",
            params![commit_id as i64, input.session.id.as_str()],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "session",
            source,
        })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "session_transaction",
            source,
        })?;
        self.next_runtime_sequence = self.next_runtime_sequence.saturating_add(1);
        Ok(SessionOpenCommitResult {
            commit: CommitBoundary {
                id: commit_id,
                first_sequence: sequence,
                last_sequence: sequence,
            },
            session: input.session,
            event: EventRecord {
                sequence,
                session_id,
                occurred_at_ms: input.occurred_at_ms,
                payload,
            },
        })
    }

    fn commit_run_transition(
        &mut self,
        input: CommitRunTransition,
    ) -> Result<RunTransitionCommitResult, StoreError> {
        if input.events.is_empty() {
            return Err(StoreError::EmptyCommitEvents);
        }

        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "run_transaction",
                source,
            })?;
        let existing_session_json: String = tx
            .query_row(
                "SELECT data_json FROM sessions WHERE id = ?",
                [input.session_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "session",
                key: input.session_id.as_str().to_string(),
            })?;
        let existing_session: SessionProjection =
            Self::decode("session_projection", existing_session_json)?;
        if input.run.session_id != input.session_id {
            return Err(StoreError::CommitSessionMismatch {
                entity: "run",
                expected: input.session_id.as_str().to_string(),
                actual: input.run.session_id.as_str().to_string(),
            });
        }
        let existing_run: Option<RunProjection> = tx
            .query_row(
                "SELECT data_json FROM runs WHERE id = ?",
                [input.run.id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "run",
                source,
            })?
            .map(|json| Self::decode("run_projection", json))
            .transpose()?;
        validate_run_execution_context(existing_run.as_ref(), &input.run)?;
        crate::validate_run_source_route(existing_run.as_ref(), &input.run)?;
        crate::validate_scheduled_run_source_link(existing_run.as_ref(), &input.run)?;
        crate::validate_auth_profile_mutation(&input)?;
        validate_run_transition_events(&input)?;
        let session_events = Self::events_for_session_tx(&tx, &input.session_id)?;
        ApprovalLifecycleState::fold_session_records(session_events.iter())?
            .validate_run_transition(&input.run.id, input.events.iter())?;

        if let crate::AuthProfileCommitMutation::SetExhausted {
            auth_profile_id,
            exhaustion,
        } = &input.auth_profile_mutation
        {
            let profile_json: String = tx
                .query_row(
                    "SELECT data_json FROM auth_profiles WHERE id = ?",
                    [auth_profile_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|source| StoreError::QueryStore {
                    entity: "auth profile",
                    source,
                })?
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "auth profile",
                    key: auth_profile_id.as_str().to_string(),
                })?;
            let mut profile: crate::AuthProfileProjection =
                Self::decode("auth profile", profile_json)?;
            profile.profile.exhaustion = Some(*exhaustion);
            tx.execute(
                "UPDATE auth_profiles SET data_json = ? WHERE id = ?",
                params![
                    Self::encode("auth profile", &profile)?,
                    auth_profile_id.as_str()
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "auth profile",
                source,
            })?;
        }

        tx.execute(
            "INSERT INTO runs (id, session_id, data_json, last_commit_id) VALUES (?, ?, ?, NULL)
             ON CONFLICT(id) DO UPDATE SET session_id = excluded.session_id, data_json = excluded.data_json",
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

        settle_scheduled_terminal_occurrence_tx(&tx, &input.run)?;

        let session_runs = Self::session_runs_tx(&tx, &input.session_id)?;
        let session = SessionProjection {
            status: compute_session_status_from_runs(&session_runs),
            ..existing_session
        };
        tx.execute(
            "INSERT INTO sessions (id, data_json, workspace_id, last_commit_id)
             VALUES (?, ?, ?, NULL)
             ON CONFLICT(id) DO UPDATE SET data_json = excluded.data_json",
            params![
                session.id.as_str(),
                Self::encode("session_projection", &session)?,
                session.workspace_id.as_str()
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "session",
            source,
        })?;

        let mut next_sequence = self.next_runtime_sequence;
        let mut emitted = Vec::with_capacity(input.events.len());
        let mut persisted = Vec::with_capacity(input.events.len());
        for payload in input.events {
            let event = EventRecord {
                sequence: next_sequence,
                session_id: input.session_id.clone(),
                occurred_at_ms: input.occurred_at_ms,
                payload,
            };
            if let UserTurnCommit::Append { text, attachments } = &input.user_turn
                && emitted.is_empty()
            {
                let row = user_row(
                    &input.run,
                    next_sequence,
                    input.occurred_at_ms,
                    text.clone(),
                    attachments.clone(),
                );
                tx.execute(
                    "INSERT INTO agent_turn_rows (sequence, session_id, data_json) VALUES (?, ?, ?)",
                    params![
                        row_sequence(&row) as i64,
                        row_session_id(&row).as_str(),
                        Self::encode("agent_turn_row", &row)?
                    ],
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "agent_turn_row",
                    source,
                })?;
            }
            if let Some(row) = apply_agent_stream_event(
                &mut self.in_flight_assistant_turns,
                &mut self.in_flight_tool_calls,
                &event,
            )? {
                tx.execute(
                    "INSERT INTO agent_turn_rows (sequence, session_id, data_json) VALUES (?, ?, ?)",
                    params![
                        row_sequence(&row) as i64,
                        row_session_id(&row).as_str(),
                        Self::encode("agent_turn_row", &row)?
                    ],
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "agent_turn_row",
                    source,
                })?;
            }
            if matches!(
                event_persistence(&event.payload),
                crate::EventPersistence::Durable
            ) {
                tx.execute(
                    "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json)
                     VALUES (?, ?, ?, ?)",
                    params![
                        next_sequence as i64,
                        input.session_id.as_str(),
                        input.occurred_at_ms as i64,
                        Self::encode("daemon_event", &event.payload)?
                    ],
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "event",
                    source,
                })?;
                persisted.push(event.clone());
            }
            emitted.push(event);
            next_sequence += 1;
        }
        let commit = if let Some((first, last)) = persisted.first().zip(persisted.last()) {
            tx.execute(
                "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    input.session_id.as_str(),
                    "run_transition",
                    input.occurred_at_ms as i64,
                    first.sequence as i64,
                    last.sequence as i64
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "commit",
                source,
            })?;
            let commit_id = tx.last_insert_rowid() as u64;
            tx.execute(
                "UPDATE sessions SET last_commit_id = ? WHERE id = ?",
                params![commit_id as i64, session.id.as_str()],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?;
            CommitBoundary {
                id: commit_id,
                first_sequence: first.sequence,
                last_sequence: last.sequence,
            }
        } else {
            CommitBoundary {
                id: 0,
                first_sequence: 0,
                last_sequence: 0,
            }
        };
        let run = input.run.with_commit_metadata(
            existing_run.as_ref(),
            input.occurred_at_ms,
            persisted.last().map(|event| event.sequence),
        );
        tx.execute(
            "UPDATE runs SET data_json = ?, last_commit_id = CASE WHEN ? = 0 THEN last_commit_id ELSE ? END WHERE id = ?",
            params![
                Self::encode("run_projection", &run)?,
                commit.id as i64,
                commit.id as i64,
                run.id.as_str()
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "run",
            source,
        })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "run_transaction",
            source,
        })?;
        self.next_runtime_sequence = next_sequence;

        Ok(RunTransitionCommitResult {
            commit,
            session,
            run,
            events: emitted,
            persisted_events: persisted,
        })
    }

    fn commit_startup_reconciliation(
        &mut self,
        input: CommitStartupReconciliation,
    ) -> Result<Vec<RunTransitionCommitResult>, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "startup_reconciliation_transaction",
                source,
            })?;
        let mut next_sequence = self.next_runtime_sequence;
        let mut results = Vec::with_capacity(input.transitions.len());
        let mut affected_sessions = BTreeMap::<SessionId, u64>::new();

        for transition in input.transitions {
            if transition.events.is_empty() {
                return Err(StoreError::EmptyCommitEvents);
            }
            if transition.run.session_id != transition.session_id {
                return Err(StoreError::CommitSessionMismatch {
                    entity: "run",
                    expected: transition.session_id.as_str().to_string(),
                    actual: transition.run.session_id.as_str().to_string(),
                });
            }
            let existing_session_json: String = tx
                .query_row(
                    "SELECT data_json FROM sessions WHERE id = ?",
                    [transition.session_id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|source| StoreError::QueryStore {
                    entity: "session",
                    source,
                })?
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "session",
                    key: transition.session_id.as_str().to_string(),
                })?;
            let existing_session: SessionProjection =
                Self::decode("session_projection", existing_session_json)?;
            let existing_run: Option<RunProjection> = tx
                .query_row(
                    "SELECT data_json FROM runs WHERE id = ?",
                    [transition.run.id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|source| StoreError::QueryStore {
                    entity: "run",
                    source,
                })?
                .map(|json| Self::decode("run_projection", json))
                .transpose()?;

            validate_run_execution_context(existing_run.as_ref(), &transition.run)?;
            crate::validate_scheduled_run_source_link(existing_run.as_ref(), &transition.run)?;
            validate_run_transition_events(&transition)?;
            let mut emitted = Vec::with_capacity(transition.events.len());
            let mut persisted = Vec::with_capacity(transition.events.len());
            for payload in transition.events {
                let event = EventRecord {
                    sequence: next_sequence,
                    session_id: transition.session_id.clone(),
                    occurred_at_ms: transition.occurred_at_ms,
                    payload,
                };
                if matches!(
                    event_persistence(&event.payload),
                    crate::EventPersistence::Durable
                ) {
                    tx.execute(
                        "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json)
                         VALUES (?, ?, ?, ?)",
                        params![
                            next_sequence as i64,
                            transition.session_id.as_str(),
                            transition.occurred_at_ms as i64,
                            Self::encode("daemon_event", &event.payload)?
                        ],
                    )
                    .map_err(|source| StoreError::QueryStore {
                        entity: "event",
                        source,
                    })?;
                    persisted.push(event.clone());
                }
                emitted.push(event);
                next_sequence = next_sequence.saturating_add(1);
            }
            let Some((first, last)) = persisted.first().zip(persisted.last()) else {
                return Err(StoreError::EmptyCommitEvents);
            };
            tx.execute(
                "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    transition.session_id.as_str(),
                    "startup_reconciliation",
                    transition.occurred_at_ms as i64,
                    first.sequence as i64,
                    last.sequence as i64
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "commit",
                source,
            })?;
            let commit_id = tx.last_insert_rowid() as u64;
            let run = transition.run.with_commit_metadata(
                existing_run.as_ref(),
                transition.occurred_at_ms,
                persisted.last().map(|event| event.sequence),
            );
            tx.execute(
                "INSERT INTO runs (id, session_id, data_json, last_commit_id) VALUES (?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET session_id = excluded.session_id, data_json = excluded.data_json, last_commit_id = excluded.last_commit_id",
                params![
                    run.id.as_str(),
                    run.session_id.as_str(),
                    Self::encode("run_projection", &run)?,
                    commit_id as i64
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "run",
                source,
            })?;
            settle_scheduled_terminal_occurrence_tx(&tx, &run)?;
            affected_sessions.insert(transition.session_id.clone(), commit_id);
            results.push(RunTransitionCommitResult {
                commit: CommitBoundary {
                    id: commit_id,
                    first_sequence: first.sequence,
                    last_sequence: last.sequence,
                },
                session: existing_session,
                run,
                events: emitted,
                persisted_events: persisted,
            });
        }

        for (session_id, last_commit_id) in affected_sessions {
            let existing_session_json: String = tx
                .query_row(
                    "SELECT data_json FROM sessions WHERE id = ?",
                    [session_id.as_str()],
                    |row| row.get(0),
                )
                .map_err(|source| StoreError::QueryStore {
                    entity: "session",
                    source,
                })?;
            let existing_session: SessionProjection =
                Self::decode("session_projection", existing_session_json)?;
            let session_runs = Self::session_runs_tx(&tx, &session_id)?;
            let session = SessionProjection {
                status: compute_session_status_from_runs(&session_runs),
                ..existing_session
            };
            tx.execute(
                "UPDATE sessions SET data_json = ?, last_commit_id = ? WHERE id = ?",
                params![
                    Self::encode("session_projection", &session)?,
                    last_commit_id as i64,
                    session_id.as_str()
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "session",
                source,
            })?;
            for result in results
                .iter_mut()
                .filter(|result| result.run.session_id == session_id)
            {
                result.session = session.clone();
            }
        }

        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "startup_reconciliation_transaction",
            source,
        })?;
        self.next_runtime_sequence = next_sequence;
        Ok(results)
    }

    fn commit_artifact_publish(
        &mut self,
        input: CommitArtifactPublish,
    ) -> Result<ArtifactPublishCommitResult, StoreError> {
        input.artifact.validate_metadata()?;
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact_transaction",
                source,
            })?;
        let run_json: String = tx
            .query_row(
                "SELECT data_json FROM runs WHERE id = ?",
                [input.artifact.run_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "run",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "run",
                key: input.artifact.run_id.as_str().to_string(),
            })?;
        let artifact_run: RunProjection = Self::decode("run_projection", run_json)?;
        let run_session_id = artifact_run.session_id.as_str().to_string();
        if run_session_id != input.artifact.session_id.as_str() {
            return Err(StoreError::CommitSessionMismatch {
                entity: "artifact",
                expected: run_session_id,
                actual: input.artifact.session_id.as_str().to_string(),
            });
        }
        if artifact_run.status != RunStatus::Running {
            return Err(StoreError::CommitRunStatusMismatch {
                entity: "artifact",
                expected: RunStatus::Running,
                actual: artifact_run.status,
            });
        }
        let changed = tx
            .execute(
                "INSERT OR IGNORE INTO artifacts (id, session_id, run_id, data_json, last_commit_id)
                 VALUES (?, ?, ?, ?, NULL)",
                params![
                    input.artifact.id.as_str(),
                    input.artifact.session_id.as_str(),
                    input.artifact.run_id.as_str(),
                    Self::encode("artifact_record", &input.artifact)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "artifact",
                source,
            })?;
        if changed == 0 {
            return Err(StoreError::DuplicateRecord {
                entity: "artifact",
                key: input.artifact.id.as_str().to_string(),
            });
        }
        let payload = DaemonEvent::Artifact(ta_protocol::wire::ArtifactEvent {
            artifact: crate::project_artifact_summary(&input.artifact),
        });
        let sequence = self.next_runtime_sequence;
        tx.execute(
            "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json)
             VALUES (?, ?, ?, ?)",
            params![
                sequence as i64,
                input.artifact.session_id.as_str(),
                input.occurred_at_ms as i64,
                Self::encode("daemon_event", &payload)?
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "event",
            source,
        })?;
        tx.execute(
            "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
             VALUES (?, ?, ?, ?, ?)",
            params![
                input.artifact.session_id.as_str(),
                "artifact_publish",
                input.occurred_at_ms as i64,
                sequence as i64,
                sequence as i64
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "commit",
            source,
        })?;
        let commit_id = tx.last_insert_rowid() as u64;
        tx.execute(
            "UPDATE artifacts SET last_commit_id = ? WHERE id = ?",
            params![commit_id as i64, input.artifact.id.as_str()],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "artifact",
            source,
        })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "artifact_transaction",
            source,
        })?;
        self.next_runtime_sequence = self.next_runtime_sequence.saturating_add(1);

        Ok(ArtifactPublishCommitResult {
            commit: CommitBoundary {
                id: commit_id,
                first_sequence: sequence,
                last_sequence: sequence,
            },
            artifact: input.artifact.clone(),
            event: EventRecord {
                sequence,
                session_id: input.artifact.session_id,
                occurred_at_ms: input.occurred_at_ms,
                payload,
            },
        })
    }

    fn commit_receipt_event(
        &mut self,
        input: CommitReceiptEvent,
    ) -> Result<ReceiptEventCommitResult, StoreError> {
        let receipt_id = match &input.event {
            DaemonEvent::ContextReceipt(event) => match event {
                ta_protocol::wire::ContextReceiptEvent::Created { receipt }
                | ta_protocol::wire::ContextReceiptEvent::Promoted { receipt }
                | ta_protocol::wire::ContextReceiptEvent::Quarantined { receipt } => {
                    Some(receipt.id.clone())
                }
            },
            _ => None,
        };
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "receipt_event_transaction",
                source,
            })?;
        tx.query_row(
            "SELECT id FROM sessions WHERE id = ?",
            [input.session_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| StoreError::QueryStore {
            entity: "session",
            source,
        })?
        .ok_or_else(|| StoreError::MissingRecord {
            entity: "session",
            key: input.session_id.as_str().to_string(),
        })?;
        let sequence = self.next_runtime_sequence;
        tx.execute(
            "INSERT INTO events (sequence, session_id, occurred_at_ms, payload_json)
             VALUES (?, ?, ?, ?)",
            params![
                sequence as i64,
                input.session_id.as_str(),
                input.occurred_at_ms as i64,
                Self::encode("daemon_event", &input.event)?
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "event",
            source,
        })?;
        tx.execute(
            "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
             VALUES (?, ?, ?, ?, ?)",
            params![
                input.session_id.as_str(),
                "context_receipt",
                input.occurred_at_ms as i64,
                sequence as i64,
                sequence as i64
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "commit",
            source,
        })?;
        let commit_id = tx.last_insert_rowid() as u64;
        if let Some(receipt_id) = receipt_id {
            tx.execute(
                "UPDATE context_receipts SET last_commit_id = ? WHERE id = ?",
                params![commit_id as i64, receipt_id.as_str()],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "context_receipt",
                source,
            })?;
        }
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "receipt_event_transaction",
            source,
        })?;
        self.next_runtime_sequence = self.next_runtime_sequence.saturating_add(1);
        Ok(ReceiptEventCommitResult {
            commit: CommitBoundary {
                id: commit_id,
                first_sequence: sequence,
                last_sequence: sequence,
            },
            event: EventRecord {
                sequence,
                session_id: input.session_id,
                occurred_at_ms: input.occurred_at_ms,
                payload: input.event,
            },
        })
    }

    fn commit_checkpoint_persist(
        &mut self,
        input: CommitCheckpointPersist,
    ) -> Result<CheckpointPersistCommitResult, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint_transaction",
                source,
            })?;
        let session_id: String = tx
            .query_row(
                "SELECT session_id FROM runs WHERE id = ?",
                [input.checkpoint.run_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "run",
                source,
            })?
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "run",
                key: input.checkpoint.run_id.as_str().to_string(),
            })?;
        tx.execute(
            "INSERT INTO commits (session_id, kind, occurred_at_ms, first_sequence, last_sequence)
             VALUES (?, ?, ?, ?, ?)",
            params![
                session_id,
                "checkpoint_persist",
                input.occurred_at_ms as i64,
                0_i64,
                0_i64
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "commit",
            source,
        })?;
        let commit_id = tx.last_insert_rowid();
        let changed = tx
            .execute(
                "INSERT OR IGNORE INTO checkpoints (run_id, revision, data_json, commit_id)
                 VALUES (?, ?, ?, ?)",
                params![
                    input.checkpoint.run_id.as_str(),
                    input.checkpoint.revision as i64,
                    Self::encode("checkpoint_record", &input.checkpoint)?,
                    commit_id
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "checkpoint",
                source,
            })?;
        if changed == 0 {
            return Err(StoreError::DuplicateRecord {
                entity: "checkpoint",
                key: format!(
                    "{}:{}",
                    input.checkpoint.run_id.as_str(),
                    input.checkpoint.revision
                ),
            });
        }
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "checkpoint_transaction",
            source,
        })?;
        Ok(CheckpointPersistCommitResult {
            commit: CommitBoundary {
                id: commit_id as u64,
                first_sequence: 0,
                last_sequence: 0,
            },
            checkpoint: input.checkpoint,
        })
    }
}
