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
    ArtifactRecord, CommitRepository, CommitRunTransition, EventRecord, ProjectionRepository,
    RunProjection, test_support::StoreSeedRepository,
};

mod activity_pages;
mod approvals;
mod artifacts;
mod native_runs;
mod run_events;
mod runs;
mod session_overview;
mod sessions;
mod timeline;
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
    service
        .seed_running_run_for_tests(session_id, objective)
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
        source: RunSource::default(),
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
            events,
            occurred_at_ms,
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
        payload: DaemonEvent::Run(crate::RunEvent {
            run_id: run_id.clone(),
            status: RunStatus::Running,
            detail: detail.to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        }),
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

pub(crate) fn select_runtime_profile(service: &AppService, runtime_profile_id: &str) {
    service
        .select_agent_runtime_profile(&crate::DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: crate::RuntimeProfileId::new(runtime_profile_id)
                .expect("runtime profile id"),
        })
        .expect("runtime profile should select");
}
