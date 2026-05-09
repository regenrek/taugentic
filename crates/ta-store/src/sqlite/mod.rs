use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, params};
use ta_protocol::wire::{
    AgentTurnRow, ApprovalRequest, ArtifactId, ContextReceipt, DaemonEvent, ReceiptId, RunId,
    RunStatus, SessionId,
};
use ta_work_source::{SourceCursor, WorkItem, WorkItemKey, WorkItemStatus, WorkSource};

#[cfg(any(test, feature = "test-support"))]
use crate::StoreSeedRepository;
use crate::{
    ArtifactPublishCommitResult, ArtifactRecord, ArtifactRepository, AssistantTurnKey,
    CheckpointPersistCommitResult, CheckpointRecord, CheckpointRepository, CommitArtifactPublish,
    CommitBoundary, CommitCheckpointPersist, CommitReceiptEvent, CommitRepository,
    CommitRunTransition, CommitSessionOpen, CommitStartupReconciliation, CreateReceipt,
    EventLogRepository, EventRecord, InFlightAssistantTurn, InFlightToolCall, NativeRunListPage,
    NativeRunListQuery, PrincipalRepository, ProjectionRepository, ReceiptEventCommitResult,
    ReceiptListQuery, ReceiptRepository, RunEventRange, RunEventRangeQuery, RunProjection,
    RunTransitionCommitResult, SessionAgentTurnsPage, SessionAgentTurnsPageQuery,
    SessionApprovalQuery, SessionArtifactQuery, SessionAuthorityRepository, SessionEventPage,
    SessionEventPageQuery, SessionEventRange, SessionEventRangeQuery, SessionOpenCommitResult,
    SessionProjection, StoreError, ToolCallKey, WorkItemRepository,
    apply_agent_stream_event, apply_promote, apply_quarantine,
    approval_lifecycle::ApprovalLifecycleState, build_returned_receipt,
    compute_session_status_from_runs, event_persistence, event_run_id,
    list_native_runs_from_projections, projections::PrincipalProjection, receipt_kind_storage,
    receipt_state_storage, receipt_unique_key, row_sequence, row_session_id,
    validate_run_transition_events,
};

mod agent_turns;
mod artifacts;
mod checkpoints;
mod commits;
mod events;
mod migrations;
mod principals;
mod receipts;
mod runs_list;
mod sessions;
#[cfg(test)]
mod tests;
mod work_items;
mod workspaces;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct SqliteStore {
    path: PathBuf,
    conn: Connection,
    next_runtime_sequence: u64,
    in_flight_assistant_turns: BTreeMap<AssistantTurnKey, InFlightAssistantTurn>,
    in_flight_tool_calls: BTreeMap<ToolCallKey, InFlightToolCall>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                StoreError::CreateStoreParentDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let conn = Connection::open(&path).map_err(|source| StoreError::OpenStore {
            path: path.clone(),
            source,
        })?;
        let mut store = Self {
            path,
            conn,
            next_runtime_sequence: 1,
            in_flight_assistant_turns: BTreeMap::new(),
            in_flight_tool_calls: BTreeMap::new(),
        };
        store.configure_connection()?;
        store.ensure_current_store()?;
        store.verify_integrity()?;
        store.next_runtime_sequence = store.next_sequence()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn configure_connection(&self) -> Result<(), StoreError> {
        self.conn
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        self.conn
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        self.conn
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        self.conn
            .pragma_update(None, "wal_autocheckpoint", 1000)
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        self.conn
            .busy_timeout(SQLITE_BUSY_TIMEOUT)
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    fn encode<T: serde::Serialize>(entity: &'static str, value: &T) -> Result<String, StoreError> {
        serde_json::to_string(value).map_err(|source| StoreError::EncodeRecord { entity, source })
    }

    fn decode<T: serde::de::DeserializeOwned>(
        entity: &'static str,
        value: String,
    ) -> Result<T, StoreError> {
        serde_json::from_str(&value).map_err(|source| StoreError::DecodeRecord { entity, source })
    }
}

impl ProjectionRepository for SqliteStore {
    fn session(&self, session_id: &SessionId) -> Result<Option<SessionProjection>, StoreError> {
        self.read_session_projection(session_id)
    }

    fn sessions(&self) -> Result<Vec<SessionProjection>, StoreError> {
        self.read_session_projections()
    }

    fn run(&self, run_id: &RunId) -> Result<Option<RunProjection>, StoreError> {
        self.read_run_projection(run_id)
    }

    fn runs(&self) -> Result<Vec<RunProjection>, StoreError> {
        self.read_run_projections()
    }

    fn list_native_runs(
        &self,
        query: &NativeRunListQuery,
    ) -> Result<NativeRunListPage, StoreError> {
        self.read_native_runs(query)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl StoreSeedRepository for SqliteStore {
    fn append_event(&mut self, event: EventRecord) -> Result<(), StoreError> {
        self.append_seed_event(event)
    }

    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError> {
        self.save_seed_principal(principal)
    }

    fn save_workspace(
        &mut self,
        workspace: crate::WorkspaceProjection,
    ) -> Result<(), StoreError> {
        self.upsert_workspace_row(workspace).map(|_| ())
    }

    fn save_session(&mut self, session: SessionProjection) -> Result<(), StoreError> {
        self.save_seed_session(session)
    }

    fn save_run(&mut self, run: RunProjection) -> Result<(), StoreError> {
        self.save_seed_run(run)
    }

    fn save_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), StoreError> {
        self.save_seed_artifact(artifact)
    }
}
