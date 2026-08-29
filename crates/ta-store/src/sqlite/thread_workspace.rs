use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::{AgentTurnRow, SessionId};

use super::SqliteStore;
use crate::{
    ProjectionRepository, StoreError, ThreadWorkspaceEvent, ThreadWorkspaceEventRecord,
    ThreadWorkspaceRecord, ThreadWorkspaceRepository, derive_thread_workspace,
};

impl ThreadWorkspaceRepository for SqliteStore {
    fn thread_workspace(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ThreadWorkspaceRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT projection_json FROM thread_workspaces WHERE session_id = ?1",
                params![session_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "thread workspace",
                source,
            })?
            .map(|json| Self::decode("thread workspace", json))
            .transpose()
    }

    fn append_thread_workspace_event(
        &mut self,
        session_id: &SessionId,
        occurred_at_ms: u64,
        event: ThreadWorkspaceEvent,
    ) -> Result<ThreadWorkspaceRecord, StoreError> {
        if self.session(session_id)?.is_none() {
            return Err(StoreError::MissingRecord {
                entity: "session",
                key: session_id.as_str().to_string(),
            });
        }
        if let ThreadWorkspaceEvent::PinAdded { pin } = &event {
            let json = self
                .conn
                .query_row(
                    "SELECT data_json FROM agent_turn_rows WHERE session_id = ?1 AND sequence = ?2",
                    params![session_id.as_str(), pin.cursor.sequence],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|source| StoreError::QueryStore {
                    entity: "thread workspace pin",
                    source,
                })?;
            let matches = json
                .map(|json| Self::decode::<AgentTurnRow>("agent turn row", json))
                .transpose()?
                .is_some_and(|row| match row {
                    AgentTurnRow::User(row) => row.run_id == pin.run_id,
                    AgentTurnRow::Assistant(row) => row.run_id == pin.run_id,
                    AgentTurnRow::ToolCall(row) => row.run_id == pin.run_id,
                    AgentTurnRow::PendingState(row) => row.run_id == pin.run_id,
                });
            if !matches {
                return Err(StoreError::AgentTurnProjectionViolation {
                    detail: "thread workspace pin must reference a durable turn".to_string(),
                });
            }
        }
        let transaction = self
            .conn
            .transaction()
            .map_err(|source| StoreError::QueryStore {
                entity: "thread workspace",
                source,
            })?;
        let next: u64 = transaction.query_row("SELECT COALESCE(MAX(sequence), 0) + 1 FROM thread_workspace_events WHERE session_id = ?1", params![session_id.as_str()], |row| row.get(0)).map_err(|source| StoreError::QueryStore { entity: "thread workspace", source })?;
        let event_json = Self::encode("thread workspace event", &event)?;
        transaction.execute("INSERT INTO thread_workspace_events (session_id, sequence, occurred_at_ms, data_json) VALUES (?1, ?2, ?3, ?4)", params![session_id.as_str(), next, occurred_at_ms, event_json]).map_err(|source| StoreError::QueryStore { entity: "thread workspace", source })?;
        let mut statement = transaction.prepare("SELECT sequence, occurred_at_ms, data_json FROM thread_workspace_events WHERE session_id = ?1 ORDER BY sequence ASC").map_err(|source| StoreError::QueryStore { entity: "thread workspace", source })?;
        let events = statement
            .query_map(params![session_id.as_str()], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|source| StoreError::QueryStore {
                entity: "thread workspace",
                source,
            })?
            .map(|row| {
                row.map_err(|source| StoreError::QueryStore {
                    entity: "thread workspace",
                    source,
                })
                .and_then(|(sequence, occurred_at_ms, json)| {
                    Self::decode("thread workspace event", json).map(|payload| {
                        ThreadWorkspaceEventRecord {
                            sequence,
                            occurred_at_ms,
                            payload,
                        }
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let projection = derive_thread_workspace(session_id.clone(), &events)?;
        let projection_json = Self::encode("thread workspace", &projection)?;
        transaction.execute("INSERT INTO thread_workspaces (session_id, projection_json) VALUES (?1, ?2) ON CONFLICT(session_id) DO UPDATE SET projection_json = excluded.projection_json", params![session_id.as_str(), projection_json]).map_err(|source| StoreError::QueryStore { entity: "thread workspace", source })?;
        transaction
            .commit()
            .map_err(|source| StoreError::QueryStore {
                entity: "thread workspace",
                source,
            })?;
        Ok(projection)
    }
}
