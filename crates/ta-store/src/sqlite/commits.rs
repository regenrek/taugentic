use super::*;

impl CommitRepository for SqliteStore {
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
        tx.execute(
            "INSERT OR IGNORE INTO sessions (id, data_json, last_commit_id) VALUES (?, ?, NULL)",
            params![
                input.session.id.as_str(),
                Self::encode("session_projection", &input.session)?
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
        validate_run_transition_events(&input)?;
        let session_events = Self::events_for_session_tx(&tx, &input.session_id)?;
        ApprovalLifecycleState::fold_session_records(session_events.iter())?
            .validate_run_transition(&input.run.id, input.events.iter())?;

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

        let session_runs = Self::session_runs_tx(&tx, &input.session_id)?;
        let session = SessionProjection {
            status: compute_session_status_from_runs(&session_runs),
            ..existing_session
        };
        tx.execute(
            "INSERT INTO sessions (id, data_json, last_commit_id) VALUES (?, ?, NULL)
             ON CONFLICT(id) DO UPDATE SET data_json = excluded.data_json",
            params![
                session.id.as_str(),
                Self::encode("session_projection", &session)?
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
            artifact: ta_protocol::wire::ArtifactSummary {
                id: input.artifact.id.clone(),
                run_id: input.artifact.run_id.clone(),
                kind: input.artifact.kind,
                storage_path: input.artifact.storage_path.clone(),
            },
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
