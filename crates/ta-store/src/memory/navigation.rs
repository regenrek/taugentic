use super::*;
use crate::{NavigationRepository, NavigationState, SessionApprovalQuery};

impl NavigationRepository for InMemoryStore {
    fn navigation_state(&self, owner_principal_id: &str) -> Result<NavigationState, StoreError> {
        Ok(self
            .navigation_states
            .get(owner_principal_id)
            .cloned()
            .unwrap_or_default())
    }

    fn save_navigation_state(
        &mut self,
        owner_principal_id: &str,
        state: NavigationState,
    ) -> Result<(), StoreError> {
        self.navigation_states
            .insert(owner_principal_id.to_string(), state);
        Ok(())
    }

    fn delete_temporary_session(
        &mut self,
        owner_principal_id: &str,
        session_id: &SessionId,
    ) -> Result<bool, StoreError> {
        let Some(session) = self.sessions.get(session_id) else {
            return Ok(false);
        };
        if session.owner_principal_id != owner_principal_id {
            return Ok(false);
        }
        let Some(state) = self.navigation_states.get(owner_principal_id) else {
            return Ok(false);
        };
        if !state.conversations.iter().any(|item| {
            item.session_id == *session_id
                && matches!(
                    item.placement,
                    ta_protocol::wire::ConversationPlacement::Temporary
                )
        }) {
            return Ok(false);
        }
        if self
            .runs
            .values()
            .filter(|run| run.session_id == *session_id)
            .any(|run| !is_terminal(run.status))
            || !self
                .approvals_for_session(&SessionApprovalQuery {
                    session_id: session_id.clone(),
                    run_id: None,
                    approval_id: None,
                })?
                .is_empty()
        {
            return Ok(false);
        }
        let session_run_ids = self
            .runs
            .values()
            .filter(|run| run.session_id == *session_id)
            .map(|run| run.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        self.sessions.remove(session_id);
        self.runs.retain(|_, run| run.session_id != *session_id);
        self.checkpoints
            .retain(|run_id, _| !session_run_ids.contains(run_id));
        self.events
            .retain(|_, event| event.session_id != *session_id);
        self.agent_turn_rows
            .retain(|_, row| *row_session_id(row) != *session_id);
        self.artifacts
            .retain(|_, artifact| artifact.session_id != *session_id);
        self.receipts
            .retain(|_, receipt| receipt.session_id != *session_id);
        self.in_flight_assistant_turns
            .retain(|_, turn| turn.session_id != *session_id);
        self.in_flight_tool_calls
            .retain(|_, call| call.session_id != *session_id);
        if let Some(state) = self.navigation_states.get_mut(owner_principal_id) {
            state
                .conversations
                .retain(|item| item.session_id != *session_id);
        }
        Ok(true)
    }
}

fn is_terminal(status: ta_protocol::wire::RunStatus) -> bool {
    matches!(
        status,
        ta_protocol::wire::RunStatus::Completed
            | ta_protocol::wire::RunStatus::Failed
            | ta_protocol::wire::RunStatus::BudgetExceeded
            | ta_protocol::wire::RunStatus::Cancelled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NavigationConversationMetadata;
    use ta_protocol::wire::{
        ApprovalEvent, ApprovalId, ApprovalRequest, ApprovalScope, ApprovalTarget, ArtifactId,
        ArtifactKind, ContextReceipt, DaemonEvent, ReceiptKind, ReceiptProvenance, ReceiptState,
        RunHarnessKind, RunId, RuntimeProfileId, SessionStatus,
    };

    const OWNER: &str = "navigation-owner";

    fn seed_temporary(store: &mut InMemoryStore, status: RunStatus) -> (SessionId, RunId) {
        let session_id = SessionId::new("session-temporary").expect("session id");
        let run_id = RunId::new("run-temporary").expect("run id");
        store.sessions.insert(
            session_id.clone(),
            SessionProjection {
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
            },
        );
        store.runs.insert(
            run_id.clone(),
            RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: RuntimeProfileId::new("runtime-test").expect("profile id"),
                objective: "temporary test".to_string(),
                status,
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
            },
        );
        store.navigation_states.insert(
            OWNER.to_string(),
            NavigationState {
                spaces: Vec::new(),
                projects: Vec::new(),
                conversations: vec![NavigationConversationMetadata {
                    session_id: session_id.clone(),
                    placement: ta_protocol::wire::ConversationPlacement::Temporary,
                    archived: false,
                    pinned: false,
                }],
            },
        );
        (session_id, run_id)
    }

    #[test]
    fn temporary_delete_rejects_nonterminal_runs_without_mutation() {
        let mut store = InMemoryStore::current();
        let (session_id, run_id) = seed_temporary(&mut store, RunStatus::Running);

        assert!(
            !store
                .delete_temporary_session(OWNER, &session_id)
                .expect("delete decision")
        );
        assert!(store.sessions.contains_key(&session_id));
        assert!(store.runs.contains_key(&run_id));
        assert_eq!(
            store.navigation_states[OWNER].conversations.len(),
            1,
            "rejection must leave navigation metadata intact"
        );
    }

    #[test]
    fn terminal_temporary_delete_removes_every_session_owned_memory_record() {
        let mut store = InMemoryStore::current();
        let (session_id, run_id) = seed_temporary(&mut store, RunStatus::Completed);
        let artifact_id = ArtifactId::new("artifact-temporary").expect("artifact id");
        store.checkpoints.insert(
            run_id.clone(),
            std::collections::BTreeMap::from([(
                1,
                crate::test_checkpoint_record(run_id.clone(), 1),
            )]),
        );
        store.artifacts.insert(
            artifact_id.clone(),
            ArtifactRecord {
                id: artifact_id,
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/temporary.patch".to_string(),
            },
        );
        store.receipts.insert(
            "receipt-temporary".to_string(),
            ContextReceipt {
                id: "receipt-temporary".to_string(),
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                parent_run_id: None,
                kind: ReceiptKind::Summary,
                provenance: ReceiptProvenance {
                    artifact_id: None,
                    agent_turn_id: None,
                    event_seq: None,
                    stream_cursor: None,
                },
                state: ReceiptState::Returned,
                title: None,
                summary: None,
                created_at_ms: 1,
                promoted_at_ms: None,
                quarantined_at_ms: None,
            },
        );
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
        assert!(!store.sessions.contains_key(&session_id));
        assert!(!store.runs.contains_key(&run_id));
        assert!(!store.checkpoints.contains_key(&run_id));
        assert!(store.artifacts.is_empty());
        assert!(store.receipts.is_empty());
        assert!(store.in_flight_assistant_turns.is_empty());
        assert!(store.in_flight_tool_calls.is_empty());
        assert!(store.navigation_states[OWNER].conversations.is_empty());
    }

    #[test]
    fn temporary_delete_rejects_pending_approvals_without_mutation() {
        let mut store = InMemoryStore::current();
        let (session_id, run_id) = seed_temporary(&mut store, RunStatus::Completed);
        let request = ApprovalRequest::new(
            ApprovalId::new("approval-temporary").expect("approval id"),
            run_id.clone(),
            ApprovalScope::ProcessExec,
            1,
            2,
            ApprovalTarget::ProcessExec { command: None },
            "approval remains pending",
        )
        .expect("approval request");
        store
            .append_seed_event(EventRecord {
                sequence: 1,
                session_id: session_id.clone(),
                occurred_at_ms: 1,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested { request }),
            })
            .expect("pending approval event");

        assert!(
            !store
                .delete_temporary_session(OWNER, &session_id)
                .expect("delete decision")
        );
        assert!(store.sessions.contains_key(&session_id));
        assert!(store.runs.contains_key(&run_id));
        assert_eq!(store.events.len(), 1);
        assert_eq!(store.navigation_states[OWNER].conversations.len(), 1);
    }
}
