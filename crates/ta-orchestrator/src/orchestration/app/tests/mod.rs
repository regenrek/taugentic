use super::*;
pub(crate) use crate::{
    ActivityCursor, ActivityPageQuery, AgentStreamEvent, AgentStreamFrame, AgentStreamTurnId,
    AgentTurnsPageQuery, ApprovalActor, ApprovalAttentionState, ApprovalDecision, ApprovalId,
    ApprovalRequest, ApprovalResolution, ApprovalScope, ApprovalTarget, ArtifactEvent, ArtifactId,
    ArtifactKind, BudgetBreach, BudgetEvent, BudgetExceededEvent, BudgetMetric, BudgetScope,
    BudgetSnapshot, ContextReceiptEvent, DaemonApprovalDecideParams, DaemonEvent, DaemonEventKind,
    GetArtifactQuery, GetRunQuery, GetRunTimelineQuery, ListApprovalsQuery, ListArtifactsQuery,
    ListNativeRunsRequest, ListReceiptsRequest, ListSessionsQuery,
    MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT, NATIVE_RUN_LIST_MAX_LIMIT, OutputContractKind,
    PromoteReceiptRequest, PublicApprovalEvent, PublicDaemonEvent, QuarantineReceiptRequest,
    ReceiptKind, ReceiptState, RunEventStreamError, RunEventStreamItem, RunEventStreamPayload,
    RunHarnessKind, RunId, RunListFilter, RunSource, RunStatus, RunSummary, RunTimelineEventKind,
    RuntimeProfileId, SessionId, SessionOverviewLaneStatus, SessionOverviewQuery, SessionStatus,
    StartRunCommand, StreamEmission, SubscribeRunEventsRequest,
};
pub(crate) use ta_protocol::wire::{
    ConflictSeverity, ConflictWarning, FileClaimConflict, FileClaimKind,
};
pub(crate) use ta_store::{
    ArtifactRecord, AuthProfileRepository, CommitRepository, CommitRunTransition, EventRecord,
    ProjectionRepository, RunProjection, test_support::StoreSeedRepository,
};

mod activity_pages;
mod approvals;
mod artifacts;
mod code_host;
mod git;
mod native_runs;
mod run_detail;
mod run_events;
mod runs;
mod scheduled_work;
mod session_overview;
mod sessions;
mod terminals;
mod thread_workspace;
mod timeline;
mod workspace_files;
mod workspaces;

pub(crate) const RUN_EVENT_REPLAY_BATCH_LIMIT: usize =
    crate::orchestration::run_events_subscribe::RUN_EVENT_REPLAY_BATCH_LIMIT;

pub(crate) const TEST_OWNER_PRINCIPAL_ID: &str = "principal-test-owner";
pub(crate) const OTHER_TEST_OWNER_PRINCIPAL_ID: &str = "other-owner-credential-hash";
pub(crate) const TEST_CLIENT_NAME: &str = "app-tests";
pub(crate) const TEST_CLIENT_CREDENTIAL: &str =
    "test-client-credential-test-client-credential-test-client-credential";

pub(crate) fn agent_stream_event(
    run_id: crate::RunId,
    fragment_sequence: Option<u64>,
    frame: AgentStreamFrame,
) -> AgentStreamEvent {
    AgentStreamEvent {
        run_id,
        emission: StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence,
            frame,
        },
    }
}

pub(crate) fn ensure_running_run(
    service: &AppService,
    session_id: &SessionId,
    objective: &str,
) -> AppDeferredMutationResult<RunSummary> {
    let selection = crate::orchestration::test_runtime_selection(service, "runtime-openai-safe");
    service
        .seed_running_run_for_tests(session_id, objective, &selection)
        .expect("seeded run should start")
}

pub(crate) fn seed_run_projection(service: &AppService, run: RunProjection) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    store.save_run(run).expect("run projection should seed");
}

pub(crate) fn native_run_projection(
    run_id: &str,
    session_id: &SessionId,
    status: RunStatus,
    started_at_ms: u64,
) -> RunProjection {
    RunProjection {
        id: RunId::new(run_id).expect("run id"),
        session_id: session_id.clone(),
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        objective: format!("Objective {run_id}"),
        status,
        harness: RunHarnessKind::Native,
        source: ta_store::default_test_run_source(),
        execution_context: ta_store::default_test_execution_context(),
        result: None,
        contract_violation: None,
        started_at_ms: Some(started_at_ms),
        ended_at_ms: None,
        last_event_seq: Some(started_at_ms / 10),
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
    }
}

pub(crate) fn approval_actor() -> ApprovalActor {
    ApprovalActor::new(TEST_OWNER_PRINCIPAL_ID).expect("approval actor")
}

pub(crate) fn append_agent_stream_tool_started_event(
    service: &AppService,
    session_id: &SessionId,
    run_id: &crate::RunId,
    sequence: u64,
    occurred_at_ms: u64,
) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    store
        .append_event(EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms,
            payload: DaemonEvent::AgentStream(agent_stream_event(
                run_id.clone(),
                None,
                AgentStreamFrame::ToolCallStarted {
                    tool_name: "shell".to_string(),
                    input: "{}".to_string(),
                },
            )),
        })
        .expect("agent stream event should append");
}

pub(crate) fn commit_agent_stream_events(
    service: &AppService,
    session_id: &SessionId,
    run_id: &crate::RunId,
    occurred_at_ms: u64,
    events: Vec<DaemonEvent>,
) {
    commit_agent_stream_events_with_user_turn(
        service,
        session_id,
        run_id,
        occurred_at_ms,
        ta_store::UserTurnCommit::NoUserTurn,
        events,
    );
}

pub(crate) fn commit_agent_stream_events_with_user_turn(
    service: &AppService,
    session_id: &SessionId,
    run_id: &crate::RunId,
    occurred_at_ms: u64,
    user_turn: ta_store::UserTurnCommit,
    events: Vec<DaemonEvent>,
) {
    let mut store = service
        .store
        .lock()
        .expect("app store should not be poisoned");
    let run = store
        .run(run_id)
        .expect("run lookup should succeed")
        .expect("run should exist");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection { ..run },
            user_turn,
            events,
            occurred_at_ms,
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
        })
        .expect("agent stream transition should commit");
}

pub(crate) fn append_and_publish_run_event(
    service: &AppService,
    session_id: &SessionId,
    run_id: &crate::RunId,
    sequence: u64,
    detail: &str,
) {
    let record = EventRecord {
        sequence,
        session_id: session_id.clone(),
        occurred_at_ms: sequence * 10,
        payload: DaemonEvent::Run(
            crate::RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
                .expect("active status"),
        ),
    };
    {
        let mut store = service
            .store
            .lock()
            .expect("app store should not be poisoned");
        store
            .append_event(record.clone())
            .expect("run event should append");
    }
    service.runtime.publish_record(&record);
}

pub(crate) fn start_run_command(service: &AppService, objective: &str) -> StartRunCommand {
    StartRunCommand::new(
        objective,
        crate::orchestration::test_runtime_selection(service, "runtime-openai-safe"),
    )
}

pub(crate) fn open_test_session(service: &AppService, title: &str) -> SessionSummary {
    service
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &OpenSessionRequest {
                title: title.to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open")
        .session
}
