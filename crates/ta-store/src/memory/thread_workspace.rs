use super::InMemoryStore;
use crate::{
    StoreError, ThreadWorkspaceEvent, ThreadWorkspaceRecord, ThreadWorkspaceRepository,
    derive_thread_workspace,
};
use ta_protocol::wire::{AgentTurnRow, SessionId};

impl ThreadWorkspaceRepository for InMemoryStore {
    fn thread_workspace(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ThreadWorkspaceRecord>, StoreError> {
        Ok(self.thread_workspaces.get(session_id).cloned())
    }

    fn append_thread_workspace_event(
        &mut self,
        session_id: &SessionId,
        occurred_at_ms: u64,
        event: ThreadWorkspaceEvent,
    ) -> Result<ThreadWorkspaceRecord, StoreError> {
        if !self.sessions.contains_key(session_id) {
            return Err(StoreError::MissingRecord {
                entity: "session",
                key: session_id.as_str().to_string(),
            });
        }
        if let ThreadWorkspaceEvent::PinAdded { pin } = &event {
            let matches =
                self.agent_turn_rows
                    .get(&pin.cursor.sequence)
                    .is_some_and(|row| match row {
                        AgentTurnRow::User(row) => {
                            row.session_id == *session_id && row.run_id == pin.run_id
                        }
                        AgentTurnRow::Assistant(row) => {
                            row.session_id == *session_id && row.run_id == pin.run_id
                        }
                        AgentTurnRow::ToolCall(row) => {
                            row.session_id == *session_id && row.run_id == pin.run_id
                        }
                        AgentTurnRow::PendingState(row) => {
                            row.session_id == *session_id && row.run_id == pin.run_id
                        }
                    });
            if !matches {
                return Err(StoreError::AgentTurnProjectionViolation {
                    detail: "thread workspace pin must reference a durable turn".to_string(),
                });
            }
        }
        let mut candidate = self
            .thread_workspace_events
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let sequence = candidate
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1));
        candidate.push(crate::ThreadWorkspaceEventRecord {
            sequence,
            occurred_at_ms,
            payload: event,
        });
        let projection = derive_thread_workspace(session_id.clone(), &candidate)?;
        self.thread_workspace_events
            .insert(session_id.clone(), candidate);
        self.thread_workspaces
            .insert(session_id.clone(), projection.clone());
        Ok(projection)
    }
}
