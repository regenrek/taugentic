use ta_protocol::wire::{
    ApprovalId, ApprovalRequest, ArtifactId, AuthProfileId, AuthProfilePreferences,
    CodeHostAccountId, RunId, SessionId, WorkspaceId,
};

use crate::{
    ArtifactPublishCommitResult, ArtifactRecord, CheckpointPersistCommitResult, CheckpointRecord,
    CommitArtifactPublish, CommitCheckpointPersist, CommitReceiptEvent, CommitRunTransition,
    CommitSessionNextRunSelection, CommitSessionOpen, CommitSessionOpenWithNavigation,
    CommitStartupReconciliation, EventRecord, NativeRunListPage, NativeRunListQuery,
    PluginRepository, ReceiptEventCommitResult, RunEventRange, RunEventRangeQuery, RunProjection,
    RunTransitionCommitResult, ScheduledWorkRepository, SessionAgentTurnsPage,
    SessionAgentTurnsPageQuery, SessionApprovalQuery, SessionEventPage, SessionEventPageQuery,
    SessionEventRange, SessionEventRangeQuery, SessionNextRunSelectionCommitResult,
    SessionOpenCommitResult, SessionProjection, StoreError, WorkItemRepository,
    WorkspaceProjection, projections::PrincipalProjection, receipts::ReceiptRepository,
};

pub trait AuthProfileRepository {
    fn auth_profile(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<Option<crate::AuthProfileProjection>, StoreError>;
    fn auth_profiles(&self) -> Result<Vec<crate::AuthProfileProjection>, StoreError>;
    fn save_auth_profile(
        &mut self,
        profile: crate::AuthProfileProjection,
    ) -> Result<(), StoreError>;
    fn replace_auth_profile_preferences(
        &mut self,
        auth_profile_id: &AuthProfileId,
        preferences: AuthProfilePreferences,
    ) -> Result<(), StoreError>;
    fn remove_auth_profile(&mut self, auth_profile_id: &AuthProfileId) -> Result<bool, StoreError>;
}

/// Persists only redacted code-host account metadata. Credentials belong to
/// the host secret store and must never cross this repository boundary.
pub trait CodeHostAccountRepository {
    fn code_host_account(
        &self,
        account_id: &CodeHostAccountId,
    ) -> Result<Option<crate::CodeHostAccountProjection>, StoreError>;
    fn code_host_accounts(&self) -> Result<Vec<crate::CodeHostAccountProjection>, StoreError>;
    fn save_code_host_account(
        &mut self,
        account: crate::CodeHostAccountProjection,
    ) -> Result<(), StoreError>;
    fn remove_code_host_account(
        &mut self,
        account_id: &CodeHostAccountId,
    ) -> Result<bool, StoreError>;
}

pub trait EventLogRepository {
    fn events(&self) -> Result<Vec<EventRecord>, StoreError>;
    /// Latest `limit` records in descending global sequence order (newest first).
    fn events_tail_desc(&self, limit: usize) -> Result<Vec<EventRecord>, StoreError>;
    fn events_for_session(&self, session_id: &SessionId) -> Result<Vec<EventRecord>, StoreError>;
    fn approvals_for_session(
        &self,
        query: &SessionApprovalQuery,
    ) -> Result<Vec<ApprovalRequest>, StoreError>;
    fn approval_lookup(
        &self,
        session_id: &SessionId,
        approval_id: &ApprovalId,
    ) -> Result<crate::SessionApprovalLookup, StoreError>;
    fn session_event_page(
        &self,
        query: &SessionEventPageQuery,
    ) -> Result<SessionEventPage, StoreError>;
    fn session_event_range(
        &self,
        query: &SessionEventRangeQuery,
    ) -> Result<SessionEventRange, StoreError>;
    fn read_run_events(&self, query: &RunEventRangeQuery) -> Result<RunEventRange, StoreError>;
    fn session_agent_turns_page(
        &self,
        query: &SessionAgentTurnsPageQuery,
    ) -> Result<SessionAgentTurnsPage, StoreError>;
}

pub trait ProjectionRepository {
    fn session(&self, session_id: &SessionId) -> Result<Option<SessionProjection>, StoreError>;
    fn sessions(&self) -> Result<Vec<SessionProjection>, StoreError>;

    fn run(&self, run_id: &RunId) -> Result<Option<RunProjection>, StoreError>;
    fn runs(&self) -> Result<Vec<RunProjection>, StoreError>;
    fn list_native_runs(&self, query: &NativeRunListQuery)
    -> Result<NativeRunListPage, StoreError>;
}

pub trait WorkspaceRepository {
    /// Insert or replace the workspace row. The daemon owns identity and
    /// canonicalization; this method records the projection verbatim and
    /// enforces `root_realpath` UNIQUE at the schema layer.
    fn upsert_workspace(
        &mut self,
        workspace: WorkspaceProjection,
    ) -> Result<WorkspaceProjection, StoreError>;

    fn workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceProjection>, StoreError>;

    fn workspace_by_root_realpath(
        &self,
        root_realpath: &str,
    ) -> Result<Option<WorkspaceProjection>, StoreError>;

    fn workspaces(&self) -> Result<Vec<WorkspaceProjection>, StoreError>;
}

pub trait ThreadWorkspaceRepository {
    fn thread_workspace(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<crate::ThreadWorkspaceRecord>, StoreError>;
    fn append_thread_workspace_event(
        &mut self,
        session_id: &SessionId,
        occurred_at_ms: u64,
        event: crate::ThreadWorkspaceEvent,
    ) -> Result<crate::ThreadWorkspaceRecord, StoreError>;
}

/// Navigation metadata has one durable store owner. Session/run/approval and
/// workspace data is intentionally not duplicated here.
pub trait NavigationRepository {
    fn navigation_state(
        &self,
        owner_principal_id: &str,
    ) -> Result<crate::NavigationState, StoreError>;
    fn save_navigation_state(
        &mut self,
        owner_principal_id: &str,
        state: crate::NavigationState,
    ) -> Result<(), StoreError>;
    fn delete_temporary_session(
        &mut self,
        owner_principal_id: &str,
        session_id: &SessionId,
    ) -> Result<bool, StoreError>;
}

pub trait PrincipalRepository {
    fn principal_by_credential_hash(
        &self,
        credential_hash: &str,
    ) -> Result<Option<PrincipalProjection>, StoreError>;

    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError>;
}

pub trait SessionAuthorityRepository {
    fn rotate_session_authority(
        &mut self,
        session_id: &SessionId,
        owner_principal_id: &str,
        presented_authority_hash: &str,
        next_authority_hash: &str,
    ) -> Result<Option<SessionProjection>, StoreError>;
}

pub trait CommitRepository {
    fn commit_run_transition(
        &mut self,
        input: CommitRunTransition,
    ) -> Result<RunTransitionCommitResult, StoreError>;

    fn commit_startup_reconciliation(
        &mut self,
        input: CommitStartupReconciliation,
    ) -> Result<Vec<RunTransitionCommitResult>, StoreError>;

    fn commit_session_open(
        &mut self,
        input: CommitSessionOpen,
    ) -> Result<SessionOpenCommitResult, StoreError>;

    fn commit_session_open_with_navigation(
        &mut self,
        input: CommitSessionOpenWithNavigation,
    ) -> Result<SessionOpenCommitResult, StoreError>;

    fn commit_session_next_run_selection(
        &mut self,
        input: CommitSessionNextRunSelection,
    ) -> Result<SessionNextRunSelectionCommitResult, StoreError>;

    fn commit_artifact_publish(
        &mut self,
        input: CommitArtifactPublish,
    ) -> Result<ArtifactPublishCommitResult, StoreError>;

    fn commit_receipt_event(
        &mut self,
        input: CommitReceiptEvent,
    ) -> Result<ReceiptEventCommitResult, StoreError>;

    fn commit_checkpoint_persist(
        &mut self,
        input: CommitCheckpointPersist,
    ) -> Result<CheckpointPersistCommitResult, StoreError>;
}

pub trait CheckpointRepository {
    fn checkpoints_for_run(&self, run_id: &RunId) -> Result<Vec<CheckpointRecord>, StoreError>;
    fn checkpoints_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<CheckpointRecord>, StoreError>;
    fn checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRecord>, StoreError>;
}

pub trait ArtifactRepository {
    fn artifact(&self, artifact_id: &ArtifactId) -> Result<Option<ArtifactRecord>, StoreError>;
    fn artifacts_for_run(&self, run_id: &RunId) -> Result<Vec<ArtifactRecord>, StoreError>;
    fn artifacts_for_session(
        &self,
        query: &SessionArtifactQuery,
    ) -> Result<Vec<ArtifactRecord>, StoreError>;

    fn artifact_for_session(
        &self,
        query: &SessionArtifactQuery,
    ) -> Result<Option<ArtifactRecord>, StoreError> {
        Ok(self.artifacts_for_session(query)?.into_iter().next())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionArtifactQuery {
    pub session_id: SessionId,
    pub run_id: Option<RunId>,
    pub artifact_id: Option<ArtifactId>,
}

pub trait PersistenceStore:
    EventLogRepository
    + ProjectionRepository
    + PrincipalRepository
    + WorkspaceRepository
    + NavigationRepository
    + SessionAuthorityRepository
    + CommitRepository
    + CheckpointRepository
    + ArtifactRepository
    + AuthProfileRepository
    + CodeHostAccountRepository
    + ReceiptRepository
    + WorkItemRepository
    + ThreadWorkspaceRepository
    + ScheduledWorkRepository
    + PluginRepository
{
}

impl<T> PersistenceStore for T where
    T: EventLogRepository
        + ProjectionRepository
        + PrincipalRepository
        + WorkspaceRepository
        + NavigationRepository
        + SessionAuthorityRepository
        + CommitRepository
        + CheckpointRepository
        + ArtifactRepository
        + AuthProfileRepository
        + CodeHostAccountRepository
        + ReceiptRepository
        + WorkItemRepository
        + ThreadWorkspaceRepository
        + ScheduledWorkRepository
        + PluginRepository
{
}
