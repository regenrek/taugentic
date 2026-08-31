use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
#[cfg(test)]
use ta_protocol::wire::RunHarnessKind;
use ta_protocol::wire::{
    AgentTurnRow, ApprovalId, ApprovalRequest, ArtifactEvent, ArtifactId, ContextReceipt,
    DaemonEvent, ReceiptId, RunId, RunStatus, SessionId, WorkspaceId,
};
use ta_protocol::wire::{SourceCursor, WorkItem, WorkItemKey};

#[cfg(any(test, feature = "test-support"))]
use crate::StoreSeedRepository;
use crate::{
    ArtifactPublishCommitResult, ArtifactRecord, ArtifactRepository, AssistantTurnKey,
    CheckpointPersistCommitResult, CheckpointRecord, CheckpointRepository, CommitArtifactPublish,
    CommitCheckpointPersist, CommitReceiptEvent, CommitRepository, CommitRunTransition,
    CommitSessionOpen, CommitStartupReconciliation, CreateReceipt, EventLogRepository, EventRecord,
    InFlightAssistantTurn, InFlightToolCall, NativeRunListPage, NativeRunListQuery,
    PrincipalRepository, ProjectionRepository, ReceiptEventCommitResult, ReceiptListQuery,
    ReceiptRepository, RunEventRange, RunEventRangeQuery, RunProjection, RunTransitionCommitResult,
    SessionAgentTurnsPage, SessionAgentTurnsPageQuery, SessionApprovalQuery, SessionArtifactQuery,
    SessionAuthorityRepository, SessionEventPage, SessionEventPageQuery, SessionEventRange,
    SessionEventRangeQuery, SessionOpenCommitResult, SessionProjection, StoreError, ToolCallKey,
    WorkspaceProjection, apply_agent_stream_event, apply_promote, apply_quarantine,
    approval_lifecycle::ApprovalLifecycleState, build_returned_receipt,
    compute_session_status_from_runs, event_persistence, list_native_runs_from_projections,
    projections::PrincipalProjection, receipt_matches_query, receipt_unique_key, row_sequence,
    row_session_id, run_event_range_from_records, validate_run_execution_context,
    validate_run_transition_events,
};

mod artifacts;
mod auth_profiles;
mod browser_profiles;
mod checkpoints;
mod code_host_accounts;
mod commits;
mod events;
mod navigation;
mod plugins;
mod principals;
mod projections;
mod receipts;
mod scheduled_work;
#[cfg(any(test, feature = "test-support"))]
mod seed;
mod sessions;
#[cfg(test)]
mod tests;
mod thread_workspace;
mod work_items;
mod workspaces;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryStore {
    next_commit_id: u64,
    next_event_sequence: u64,
    events: BTreeMap<u64, EventRecord>,
    principals: BTreeMap<String, PrincipalProjection>,
    #[serde(default)]
    workspaces: BTreeMap<WorkspaceId, WorkspaceProjection>,
    #[serde(default)]
    navigation_states: BTreeMap<String, crate::NavigationState>,
    #[serde(default)]
    auth_profiles: BTreeMap<ta_protocol::wire::AuthProfileId, crate::AuthProfileProjection>,
    #[serde(default)]
    code_host_accounts:
        BTreeMap<ta_protocol::wire::CodeHostAccountId, crate::CodeHostAccountProjection>,
    #[serde(default)]
    browser_profiles: BTreeMap<String, ta_protocol::wire::BrowserProfile>,
    sessions: BTreeMap<SessionId, SessionProjection>,
    runs: BTreeMap<RunId, RunProjection>,
    #[serde(default)]
    scheduled_work_definitions:
        BTreeMap<ta_protocol::wire::ScheduledWorkId, ta_protocol::wire::ScheduledWorkDefinition>,
    #[serde(default)]
    scheduled_work_occurrences: BTreeMap<
        ta_protocol::wire::ScheduledWorkOccurrenceId,
        ta_protocol::wire::ScheduledWorkOccurrence,
    >,
    plugin_installations: BTreeMap<
        (String, ta_protocol::wire::PluginId, String, String),
        ta_protocol::wire::PluginInstallation,
    >,
    checkpoints: BTreeMap<RunId, BTreeMap<u64, CheckpointRecord>>,
    artifacts: BTreeMap<ArtifactId, ArtifactRecord>,
    receipts: BTreeMap<ReceiptId, ContextReceipt>,
    #[serde(default)]
    work_items: BTreeMap<WorkItemKey, WorkItem>,
    #[serde(default)]
    work_source_cursors: BTreeMap<String, SourceCursor>,
    #[serde(default)]
    thread_workspaces: BTreeMap<SessionId, crate::ThreadWorkspaceRecord>,
    #[serde(default)]
    thread_workspace_events: BTreeMap<SessionId, Vec<crate::ThreadWorkspaceEventRecord>>,
    #[cfg(any(test, feature = "test-support"))]
    #[serde(default, skip)]
    fail_next_receipt_create: bool,
    agent_turn_rows: BTreeMap<u64, AgentTurnRow>,
    in_flight_assistant_turns: BTreeMap<AssistantTurnKey, InFlightAssistantTurn>,
    in_flight_tool_calls: BTreeMap<ToolCallKey, InFlightToolCall>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            next_commit_id: 1,
            next_event_sequence: 1,
            events: BTreeMap::new(),
            principals: BTreeMap::new(),
            workspaces: BTreeMap::new(),
            navigation_states: BTreeMap::new(),
            auth_profiles: BTreeMap::new(),
            code_host_accounts: BTreeMap::new(),
            browser_profiles: BTreeMap::new(),
            sessions: BTreeMap::new(),
            runs: BTreeMap::new(),
            scheduled_work_definitions: BTreeMap::new(),
            scheduled_work_occurrences: BTreeMap::new(),
            plugin_installations: BTreeMap::new(),
            checkpoints: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            receipts: BTreeMap::new(),
            work_items: BTreeMap::new(),
            work_source_cursors: BTreeMap::new(),
            thread_workspaces: BTreeMap::new(),
            thread_workspace_events: BTreeMap::new(),
            #[cfg(any(test, feature = "test-support"))]
            fail_next_receipt_create: false,
            agent_turn_rows: BTreeMap::new(),
            in_flight_assistant_turns: BTreeMap::new(),
            in_flight_tool_calls: BTreeMap::new(),
        }
    }

    pub fn current() -> Self {
        Self::new()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn fail_next_receipt_create_for_tests(&mut self) {
        self.fail_next_receipt_create = true;
    }

    fn append_seed_event(&mut self, event: EventRecord) -> Result<(), StoreError> {
        if self.events.contains_key(&event.sequence) {
            return Err(StoreError::DuplicateRecord {
                entity: "event",
                key: event.sequence.to_string(),
            });
        }

        if let Some(row) = apply_agent_stream_event(
            &mut self.in_flight_assistant_turns,
            &mut self.in_flight_tool_calls,
            &event,
        )? {
            self.agent_turn_rows.insert(row_sequence(&row), row);
        }
        self.next_event_sequence = self
            .next_event_sequence
            .max(event.sequence.saturating_add(1));
        self.events.insert(event.sequence, event);
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn save_seed_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError> {
        self.principals.insert(principal.id.clone(), principal);
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn save_seed_workspace(&mut self, workspace: WorkspaceProjection) -> Result<(), StoreError> {
        self.workspaces.insert(workspace.id().clone(), workspace);
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn save_seed_session(&mut self, session: SessionProjection) -> Result<(), StoreError> {
        if !self.workspaces.contains_key(&session.workspace_id) {
            self.workspaces.insert(
                session.workspace_id.clone(),
                crate::test_workspace(
                    session.workspace_id.as_str(),
                    crate::default_test_workspace_root(),
                ),
            );
        }
        self.sessions.insert(session.id.clone(), session);
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn save_seed_run(&mut self, run: RunProjection) -> Result<(), StoreError> {
        self.runs.insert(run.id.clone(), run);
        Ok(())
    }

    fn save_seed_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), StoreError> {
        artifact.validate_metadata()?;
        if self.artifacts.contains_key(&artifact.id) {
            return Err(StoreError::DuplicateRecord {
                entity: "artifact",
                key: artifact.id.as_str().to_string(),
            });
        }

        self.artifacts.insert(artifact.id.clone(), artifact);
        Ok(())
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}
