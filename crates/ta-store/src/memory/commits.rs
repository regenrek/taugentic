use super::*;
use crate::{
    CommitSessionNextRunSelection, CommitSessionOpenWithNavigation,
    SessionNextRunSelectionCommitResult, UserTurnCommit, user_row,
};

impl CommitRepository for InMemoryStore {
    fn commit_session_open_with_navigation(
        &mut self,
        input: CommitSessionOpenWithNavigation,
    ) -> Result<SessionOpenCommitResult, StoreError> {
        let result = self.commit_session_open(CommitSessionOpen {
            session: input.session,
            occurred_at_ms: input.occurred_at_ms,
        })?;
        self.navigation_states
            .insert(input.owner_principal_id, input.navigation);
        Ok(result)
    }

    fn commit_session_next_run_selection(
        &mut self,
        input: CommitSessionNextRunSelection,
    ) -> Result<SessionNextRunSelectionCommitResult, StoreError> {
        let existing = self
            .sessions
            .get(&input.session_id)
            .cloned()
            .ok_or_else(|| StoreError::MissingRecord {
                entity: "session",
                key: input.session_id.as_str().to_string(),
            })?;
        let session = SessionProjection {
            next_run_selection: input.selection,
            ..existing
        };
        self.sessions.insert(session.id.clone(), session.clone());
        Ok(SessionNextRunSelectionCommitResult { session })
    }

    fn commit_session_open(
        &mut self,
        input: CommitSessionOpen,
    ) -> Result<SessionOpenCommitResult, StoreError> {
        if !self.workspaces.contains_key(&input.session.workspace_id) {
            return Err(StoreError::SessionWorkspaceMissing {
                workspace_id: input.session.workspace_id.as_str().to_string(),
            });
        }
        self.sessions
            .insert(input.session.id.clone(), input.session.clone());
        let payload = DaemonEvent::Session(ta_protocol::wire::SessionEvent {
            session_id: input.session.id.clone(),
            status: input.session.status,
        });
        let event = EventRecord {
            sequence: self.next_event_sequence,
            session_id: input.session.id.clone(),
            occurred_at_ms: input.occurred_at_ms,
            payload,
        };
        self.append_seed_event(event.clone())?;
        let commit = crate::CommitBoundary {
            id: self.next_commit_id,
            first_sequence: event.sequence,
            last_sequence: event.sequence,
        };
        self.next_commit_id += 1;
        Ok(SessionOpenCommitResult {
            commit,
            session: input.session,
            event,
        })
    }

    fn commit_run_transition(
        &mut self,
        input: CommitRunTransition,
    ) -> Result<RunTransitionCommitResult, StoreError> {
        if input.events.is_empty() {
            return Err(StoreError::EmptyCommitEvents);
        }
        let Some(existing_session) = self.sessions.get(&input.session_id).cloned() else {
            return Err(StoreError::MissingRecord {
                entity: "session",
                key: input.session_id.as_str().to_string(),
            });
        };
        if input.run.session_id != input.session_id {
            return Err(StoreError::CommitSessionMismatch {
                entity: "run",
                expected: input.session_id.as_str().to_string(),
                actual: input.run.session_id.as_str().to_string(),
            });
        }
        let existing_run = self.runs.get(&input.run.id).cloned();
        validate_run_execution_context(existing_run.as_ref(), &input.run)?;
        crate::validate_run_source_route(existing_run.as_ref(), &input.run)?;
        crate::validate_scheduled_run_source_link(existing_run.as_ref(), &input.run)?;
        crate::validate_auth_profile_mutation(&input)?;
        validate_run_transition_events(&input)?;
        ApprovalLifecycleState::fold_session_records(
            self.events
                .values()
                .filter(|record| record.session_id == input.session_id),
        )?
        .validate_run_transition(&input.run.id, input.events.iter())?;

        let scheduled_terminal_occurrence = crate::scheduled_work::scheduled_run_source(&input.run)
            .and_then(|(_, occurrence_id)| {
                crate::scheduled_terminal_state(input.run.id.clone(), input.run.status)
                    .map(|state| (occurrence_id.clone(), state))
            });
        if let Some((occurrence_id, _)) = &scheduled_terminal_occurrence {
            let occurrence = self
                .scheduled_work_occurrences
                .get(occurrence_id)
                .ok_or_else(|| StoreError::MissingRecord {
                    entity: "scheduled work occurrence",
                    key: occurrence_id.as_str().to_string(),
                })?;
            if crate::claimed_run_id(occurrence) != Some(&input.run.id) {
                return Err(StoreError::ScheduledWorkOccurrenceClaimMismatch {
                    occurrence_id: occurrence_id.as_str().to_string(),
                    run_id: input.run.id.as_str().to_string(),
                });
            }
        }

        if let crate::AuthProfileCommitMutation::SetExhausted {
            auth_profile_id,
            exhaustion,
        } = &input.auth_profile_mutation
        {
            let profile = self.auth_profiles.get_mut(auth_profile_id).ok_or_else(|| {
                StoreError::MissingRecord {
                    entity: "auth profile",
                    key: auth_profile_id.as_str().to_string(),
                }
            })?;
            profile.profile.exhaustion = Some(*exhaustion);
        }

        let mut emitted = Vec::with_capacity(input.events.len());
        let mut persisted = Vec::with_capacity(input.events.len());
        for payload in input.events {
            let sequence = self.next_event_sequence;
            self.next_event_sequence = self.next_event_sequence.saturating_add(1);
            if let UserTurnCommit::Append { text, attachments } = &input.user_turn
                && emitted.is_empty()
            {
                let row = user_row(
                    &input.run,
                    sequence,
                    input.occurred_at_ms,
                    text.clone(),
                    attachments.clone(),
                );
                self.agent_turn_rows.insert(row_sequence(&row), row);
            }
            let event = EventRecord {
                sequence,
                session_id: input.session_id.clone(),
                occurred_at_ms: input.occurred_at_ms,
                payload,
            };
            if let Some(row) = apply_agent_stream_event(
                &mut self.in_flight_assistant_turns,
                &mut self.in_flight_tool_calls,
                &event,
            )? {
                self.agent_turn_rows.insert(row_sequence(&row), row);
            }
            if matches!(
                event_persistence(&event.payload),
                crate::EventPersistence::Durable
            ) {
                self.events.insert(sequence, event.clone());
                persisted.push(event.clone());
            }
            emitted.push(event);
        }

        let run = input.run.with_commit_metadata(
            existing_run.as_ref(),
            input.occurred_at_ms,
            persisted.last().map(|event| event.sequence),
        );
        self.runs.insert(run.id.clone(), run.clone());
        if let Some((occurrence_id, state)) = scheduled_terminal_occurrence {
            let occurrence = self
                .scheduled_work_occurrences
                .get_mut(&occurrence_id)
                .expect("scheduled occurrence validated above");
            occurrence.state = state;
        }
        let session_runs = self
            .runs
            .values()
            .filter(|candidate| candidate.session_id == input.session_id)
            .cloned()
            .collect::<Vec<_>>();
        let session = SessionProjection {
            status: compute_session_status_from_runs(&session_runs),
            ..existing_session
        };
        self.sessions.insert(session.id.clone(), session.clone());

        let commit = if let Some((first, last)) = persisted.first().zip(persisted.last()) {
            let commit = crate::CommitBoundary {
                id: self.next_commit_id,
                first_sequence: first.sequence,
                last_sequence: last.sequence,
            };
            self.next_commit_id += 1;
            commit
        } else {
            crate::CommitBoundary {
                id: 0,
                first_sequence: 0,
                last_sequence: 0,
            }
        };

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
        let before = self.clone();
        let mut results = Vec::with_capacity(input.transitions.len());
        for transition in input.transitions {
            match self.commit_run_transition(transition) {
                Ok(result) => results.push(result),
                Err(error) => {
                    *self = before;
                    return Err(error);
                }
            }
        }
        Ok(results)
    }

    fn commit_artifact_publish(
        &mut self,
        input: CommitArtifactPublish,
    ) -> Result<ArtifactPublishCommitResult, StoreError> {
        input.artifact.validate_metadata()?;
        if !self.sessions.contains_key(&input.artifact.session_id) {
            return Err(StoreError::MissingRecord {
                entity: "session",
                key: input.artifact.session_id.as_str().to_string(),
            });
        }
        if !self.runs.contains_key(&input.artifact.run_id) {
            return Err(StoreError::MissingRecord {
                entity: "run",
                key: input.artifact.run_id.as_str().to_string(),
            });
        }
        let artifact_run = self
            .runs
            .get(&input.artifact.run_id)
            .expect("run existence checked above");
        if artifact_run.session_id != input.artifact.session_id {
            return Err(StoreError::CommitSessionMismatch {
                entity: "artifact",
                expected: artifact_run.session_id.as_str().to_string(),
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
        self.save_seed_artifact(input.artifact.clone())?;
        let payload = DaemonEvent::Artifact(ArtifactEvent {
            artifact: crate::project_artifact_summary(&input.artifact),
        });
        let event = EventRecord {
            sequence: self.next_event_sequence,
            session_id: input.artifact.session_id.clone(),
            occurred_at_ms: input.occurred_at_ms,
            payload,
        };
        self.append_seed_event(event.clone())?;
        let commit = crate::CommitBoundary {
            id: self.next_commit_id,
            first_sequence: event.sequence,
            last_sequence: event.sequence,
        };
        self.next_commit_id += 1;

        Ok(ArtifactPublishCommitResult {
            commit,
            artifact: input.artifact,
            event,
        })
    }

    fn commit_receipt_event(
        &mut self,
        input: CommitReceiptEvent,
    ) -> Result<ReceiptEventCommitResult, StoreError> {
        if !self.sessions.contains_key(&input.session_id) {
            return Err(StoreError::MissingRecord {
                entity: "session",
                key: input.session_id.as_str().to_string(),
            });
        }
        let event = EventRecord {
            sequence: self.next_event_sequence,
            session_id: input.session_id,
            occurred_at_ms: input.occurred_at_ms,
            payload: input.event,
        };
        self.append_seed_event(event.clone())?;
        let commit = crate::CommitBoundary {
            id: self.next_commit_id,
            first_sequence: event.sequence,
            last_sequence: event.sequence,
        };
        self.next_commit_id += 1;
        Ok(ReceiptEventCommitResult { commit, event })
    }

    fn commit_checkpoint_persist(
        &mut self,
        input: CommitCheckpointPersist,
    ) -> Result<CheckpointPersistCommitResult, StoreError> {
        if !self.runs.contains_key(&input.checkpoint.run_id) {
            return Err(StoreError::MissingRecord {
                entity: "run",
                key: input.checkpoint.run_id.as_str().to_string(),
            });
        }
        let revisions = self
            .checkpoints
            .entry(input.checkpoint.run_id.clone())
            .or_default();
        if revisions.contains_key(&input.checkpoint.revision) {
            return Err(StoreError::DuplicateRecord {
                entity: "checkpoint",
                key: format!(
                    "{}:{}",
                    input.checkpoint.run_id.as_str(),
                    input.checkpoint.revision
                ),
            });
        }
        revisions.insert(input.checkpoint.revision, input.checkpoint.clone());
        let commit = crate::CommitBoundary {
            id: self.next_commit_id,
            first_sequence: 0,
            last_sequence: 0,
        };
        self.next_commit_id += 1;
        Ok(CheckpointPersistCommitResult {
            commit,
            checkpoint: input.checkpoint,
        })
    }
}
