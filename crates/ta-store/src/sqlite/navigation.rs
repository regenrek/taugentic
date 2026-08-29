use super::*;
use crate::{NavigationRepository, NavigationState, SessionApprovalQuery};

impl NavigationRepository for SqliteStore {
    fn navigation_state(&self, owner_principal_id: &str) -> Result<NavigationState, StoreError> {
        let json = self
            .conn
            .query_row(
                "SELECT data_json FROM navigation_states WHERE owner_principal_id = ?",
                [owner_principal_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "navigation_state",
                source,
            })?;
        json.map(|value| Self::decode("navigation_state", value))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }

    fn save_navigation_state(
        &mut self,
        owner_principal_id: &str,
        state: NavigationState,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO navigation_states (owner_principal_id, data_json) VALUES (?, ?)
             ON CONFLICT(owner_principal_id) DO UPDATE SET data_json = excluded.data_json",
                params![
                    owner_principal_id,
                    Self::encode("navigation_state", &state)?
                ],
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "navigation_state",
                source,
            })?;
        Ok(())
    }

    fn delete_temporary_session(
        &mut self,
        owner_principal_id: &str,
        session_id: &SessionId,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?;
        let json: Option<String> = tx
            .query_row(
                "SELECT data_json FROM sessions WHERE id = ?",
                [session_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?;
        let Some(json) = json else { return Ok(false) };
        let projection: SessionProjection = Self::decode("session_projection", json)?;
        if projection.owner_principal_id != owner_principal_id {
            return Ok(false);
        }
        let state_json: Option<String> = tx
            .query_row(
                "SELECT data_json FROM navigation_states WHERE owner_principal_id = ?",
                [owner_principal_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "navigation_state",
                source,
            })?;
        let Some(state_json) = state_json else {
            return Ok(false);
        };
        let mut state: NavigationState = Self::decode("navigation_state", state_json)?;
        if !state.conversations.iter().any(|item| {
            item.session_id == *session_id
                && matches!(
                    item.placement,
                    ta_protocol::wire::ConversationPlacement::Temporary
                )
        }) {
            return Ok(false);
        }
        let run_json = tx
            .prepare("SELECT data_json FROM runs WHERE session_id = ?")
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?
            .query_map([session_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?;
        if run_json
            .into_iter()
            .map(|json| Self::decode::<RunProjection>("run_projection", json))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|run| !is_terminal(run.status))
        {
            return Ok(false);
        }
        let events = tx
            .prepare(
                "SELECT sequence, occurred_at_ms, payload_json FROM events WHERE session_id = ? ORDER BY sequence ASC",
            )
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?
            .query_map([session_id.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| StoreError::QueryStore {
                entity: "temporary_session",
                source,
            })?
            .into_iter()
            .map(|(sequence, occurred_at_ms, payload_json)| {
                Ok(EventRecord {
                    sequence: u64::try_from(sequence).map_err(|source| StoreError::DecodeRecord {
                        entity: "event_sequence",
                        source: serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            source,
                        )),
                    })?,
                    session_id: session_id.clone(),
                    occurred_at_ms: u64::try_from(occurred_at_ms).map_err(|source| StoreError::DecodeRecord {
                        entity: "event_occurred_at_ms",
                        source: serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            source,
                        )),
                    })?,
                    payload: Self::decode("daemon_event", payload_json)?,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        if !ApprovalLifecycleState::fold_session_records(events.iter())?
            .approvals_for_query(&SessionApprovalQuery {
                session_id: session_id.clone(),
                run_id: None,
                approval_id: None,
            })
            .is_empty()
        {
            return Ok(false);
        }
        tx.execute(
            "UPDATE sessions SET last_commit_id = NULL WHERE id = ?",
            [session_id.as_str()],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "temporary_session",
            source,
        })?;
        for sql in [
            "DELETE FROM checkpoints WHERE run_id IN (SELECT id FROM runs WHERE session_id = ?)",
            "DELETE FROM artifacts WHERE session_id = ?",
            "DELETE FROM context_receipts WHERE session_id = ?",
            "DELETE FROM agent_turn_rows WHERE session_id = ?",
            "DELETE FROM events WHERE session_id = ?",
            "DELETE FROM commits WHERE session_id = ?",
            "DELETE FROM runs WHERE session_id = ?",
            "DELETE FROM sessions WHERE id = ?",
        ] {
            tx.execute(sql, [session_id.as_str()])
                .map_err(|source| StoreError::QueryStore {
                    entity: "temporary_session",
                    source,
                })?;
        }
        state
            .conversations
            .retain(|item| item.session_id != *session_id);
        tx.execute(
            "UPDATE navigation_states SET data_json = ? WHERE owner_principal_id = ?",
            params![
                Self::encode("navigation_state", &state)?,
                owner_principal_id
            ],
        )
        .map_err(|source| StoreError::QueryStore {
            entity: "navigation_state",
            source,
        })?;
        tx.commit().map_err(|source| StoreError::QueryStore {
            entity: "temporary_session",
            source,
        })?;
        self.in_flight_assistant_turns
            .retain(|_, turn| turn.session_id != *session_id);
        self.in_flight_tool_calls
            .retain(|_, call| call.session_id != *session_id);
        Ok(true)
    }
}

fn is_terminal(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Completed | RunStatus::Failed | RunStatus::BudgetExceeded | RunStatus::Cancelled
    )
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::StoreSeedRepository;
    use ta_protocol::wire::{
        ArtifactId, ArtifactKind, ConversationPlacement, RunHarnessKind, RunId, RuntimeProfileId,
        SessionId, SessionStatus,
    };

    const OWNER: &str = "navigation-owner";

    fn test_db_path() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("taugentic-navigation-delete-{nanos}.sqlite3"))
    }

    #[test]
    fn terminal_temporary_delete_is_atomic_and_orphan_free_in_sqlite() {
        let path = test_db_path();
        let mut store = SqliteStore::open(&path).expect("store");
        let session_id = SessionId::new("session-temporary").expect("session id");
        let run_id = RunId::new("run-temporary").expect("run id");
        let artifact_id = ArtifactId::new("artifact-temporary").expect("artifact id");
        StoreSeedRepository::save_principal(
            &mut store,
            PrincipalProjection {
                id: OWNER.to_string(),
                client_name: "navigation-tests".to_string(),
                credential_hash: "navigation-credential".to_string(),
            },
        )
        .expect("principal");
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "navigation-tests".to_string(),
                owner_principal_id: OWNER.to_string(),
                current_session_authority_hash: "authority".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Temporary".to_string(),
                status: SessionStatus::Idle,
                workspace_id: crate::default_test_workspace_id(),
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            })
            .expect("session");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: RuntimeProfileId::new("runtime-test").expect("profile id"),
                objective: "temporary test".to_string(),
                status: RunStatus::Completed,
                harness: RunHarnessKind::Unknown,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            })
            .expect("run");
        store
            .save_navigation_state(
                OWNER,
                NavigationState {
                    spaces: Vec::new(),
                    projects: Vec::new(),
                    conversations: vec![crate::NavigationConversationMetadata {
                        session_id: session_id.clone(),
                        placement: ConversationPlacement::Temporary,
                        archived: false,
                        pinned: false,
                    }],
                },
            )
            .expect("navigation state");
        store
            .save_artifact(ArtifactRecord {
                id: artifact_id.clone(),
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/temporary.patch".to_string(),
            })
            .expect("artifact");
        store
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: crate::test_checkpoint_record(run_id.clone(), 1),
                occurred_at_ms: 1,
            })
            .expect("checkpoint");
        store.in_flight_assistant_turns.insert(
            AssistantTurnKey {
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                turn_id: None,
            },
            InFlightAssistantTurn {
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                turn_id: None,
                started_at_ms: 1,
                text: "partial".to_string(),
            },
        );
        store.in_flight_tool_calls.insert(
            ToolCallKey {
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                turn_id: None,
                item_id: None,
            },
            InFlightToolCall {
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                turn_id: None,
                item_id: None,
                tool_name: "test".to_string(),
                input: String::new(),
                started_at_ms: 1,
                output: String::new(),
            },
        );

        assert!(
            store
                .delete_temporary_session(OWNER, &session_id)
                .expect("terminal delete")
        );
        assert!(store.session(&session_id).expect("session read").is_none());
        assert!(store.run(&run_id).expect("run read").is_none());
        assert!(
            store
                .checkpoints_for_run(&run_id)
                .expect("checkpoint read")
                .is_empty()
        );
        assert!(
            store
                .artifact(&artifact_id)
                .expect("artifact read")
                .is_none()
        );
        assert!(
            store
                .events_for_session(&session_id)
                .expect("event read")
                .is_empty()
        );
        assert!(
            store
                .navigation_state(OWNER)
                .expect("navigation state")
                .conversations
                .is_empty()
        );
        assert!(store.in_flight_assistant_turns.is_empty());
        assert!(store.in_flight_tool_calls.is_empty());

        drop(store);
        let _ = std::fs::remove_file(path);
    }
}
