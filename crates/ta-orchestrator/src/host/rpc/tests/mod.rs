pub(super) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
pub(super) use std::thread;
pub(super) use std::time::{Duration, Instant};

pub(super) use super::{
    close_session_for_event_serialization_failure, spawn_event_forwarder,
    state::{DaemonRpcSessionState, approval_actor_from_session},
};
pub(super) use crate::{
    ActivityPageQuery, AgentRuntimeModelId, AgentRuntimeSelection, AgentRuntimeSnapshot,
    AgentStreamEvent, AgentStreamFrame, ApprovalDecision, ApprovalSnapshotResult,
    ArtifactContentResult, ArtifactId, ArtifactKind, ArtifactSnapshotResult, AuthProfileId,
    ContextReceipt, DAEMON_DEFAULT_SOCKET_NAME, DAEMON_PROTOCOL_VERSION,
    DEFAULT_OUTBOUND_QUEUE_DEPTH, DaemonApprovalDecideParams, DaemonApprovalDecideResult,
    DaemonControlStatusResult, DaemonDiagnostics, DaemonEventCursor, DaemonEventKind,
    DaemonInitializeResult, DaemonNavigationSubscribeResult, DaemonRunCancelParams,
    DaemonSessionAttachParams, DaemonSessionAttachResult, DaemonSessionOpenParams,
    DaemonSessionOpenResult, DaemonStatusResult, DaemonSubscribeResult, GetAgentRuntimeQuery,
    GetArtifactQuery, GetRunQuery, GetRunTimelineQuery, GetSessionQuery, HANDOFF_CLIENT_NAME,
    JsonRpcConnectionRuntime, JsonRpcMessage, JsonRpcRequest, JsonRpcServerSession,
    ListApprovalsQuery, ListArtifactsQuery, ListReceiptsRequest, ListReceiptsResult, ListRunsQuery,
    ListSessionsQuery, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT, METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
    METHOD_DAEMON_AGENT_RUNTIME_GET, METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_CONTEXT_RECEIPTS_LIST,
    METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE, METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE,
    METHOD_DAEMON_CONTROL_STATUS, METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_NAVIGATION_INVALIDATED, METHOD_DAEMON_NAVIGATION_SUBSCRIBE,
    METHOD_DAEMON_RECIPES_LIST, METHOD_DAEMON_RUN_CANCEL, METHOD_DAEMON_RUN_EVENT,
    METHOD_DAEMON_RUN_GET, METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_START,
    METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS, METHOD_DAEMON_RUN_TIMELINE, METHOD_DAEMON_SESSION_ATTACH,
    METHOD_DAEMON_SESSION_GET, METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN,
    METHOD_DAEMON_SESSION_OVERVIEW, METHOD_DAEMON_STATUS, METHOD_DAEMON_SUBSCRIBE,
    METHOD_DAEMON_WORK_ITEM_DISMISS, METHOD_DAEMON_WORK_ITEM_LIST, METHOD_DAEMON_WORK_ITEM_REFRESH,
    METHOD_DAEMON_WORK_ITEM_TRIGGER, METHOD_NOT_FOUND_ERROR_CODE, METHOD_WORKFLOW_LOAD,
    METHOD_WORKFLOW_RELOAD, METHOD_WORKFLOW_STATUS, METHOD_WORKFLOW_VALIDATE, OpenSessionRequest,
    PromoteReceiptRequest, PublicApprovalEvent, PublicDaemonEvent, QuarantineReceiptRequest,
    ReceiptKind, ReceiptState, RecipeListResponse, RunDetail, RunEventDelta, RunEventStreamItem,
    RunEventStreamPayload, RunId, RunStatus, RunSummary, RunTimeline, RuntimeProfileId,
    ServerConfig, SessionId, SessionOverviewLaneStatus, SessionOverviewQuery,
    SessionOverviewResult, SessionStatus, SessionSummary, StartRunCommand, StreamEmission,
    SubscribeRunEventsRequest, SubscribeRunEventsResult, WorkItemDismissParams,
    WorkItemDismissResult, WorkItemListQuery, WorkItemListResult, WorkItemRefreshParams,
    WorkItemTriggerParams, WorkItemTriggerResult, WorkflowLoadParams, WorkflowReloadOutcome,
    WorkflowStatusResult, WorkflowValidateParams, WorkflowValidationReport, WorkspaceSelector,
    host::config::ControlToken,
    host::{
        bootstrap::{BootstrapState, boot, boot_with_store_and_dispatcher},
        config::{test_config, with_test_config_home},
        internal_stop::{InternalDaemonStopResult, METHOD_DAEMON_INTERNAL_STOP},
    },
};
pub(super) use ta_store::{ArtifactRecord, EventRecord, SqliteStore};

mod browser;

fn handle_request(
    state: &BootstrapState,
    shutdown_requested: &Arc<AtomicBool>,
    session: &JsonRpcServerSession,
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    request: JsonRpcRequest,
) -> crate::JsonRpcHandlerResult {
    futures_executor::block_on(super::dispatch::handle_request(
        state,
        shutdown_requested,
        session,
        session_state,
        request,
    ))
}

const TEST_CLIENT_NAME: &str = "test-client";
const TEST_CLIENT_CREDENTIAL: &str =
    "test-client-credential-test-client-credential-test-client-credential";
const OTHER_TEST_CLIENT_CREDENTIAL: &str =
    "other-client-credential-other-client-credential-other-client-cred";
const TEST_OWNER_PRINCIPAL_ID: &str =
    "c368bf31655d0b8d69f400a61d9ddaeaaa8641f41bb58b4b575ca3962c9f792d";
const OTHER_TEST_OWNER_PRINCIPAL_ID: &str =
    "f8a18565ee66711156e616ec3ccdc29fb936e5a0a86543122d6bb175d2fe3dab";

fn issue_test_principal_id(state: &BootstrapState, client_name: &str) -> String {
    state
        .app
        .resolve_or_issue_session_principal(client_name, None)
        .expect("test principal should issue")
        .principal_id
}

fn initialized_test_session_state(
    principal_id: &str,
    client_name: &str,
    session_id: Option<SessionId>,
) -> Arc<Mutex<DaemonRpcSessionState>> {
    Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(client_name.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(principal_id.to_string()),
        attached_session_id: session_id,
    }))
}

fn test_session_authority() -> crate::SessionAuthority {
    crate::SessionAuthority::new("session-authority-1session-authority-1")
        .expect("session authority")
}

fn ensure_running_run(
    state: &BootstrapState,
    session_id: &SessionId,
    objective: &str,
) -> crate::orchestration::AppDeferredMutationResult<RunSummary> {
    let selection = explicit_runtime_selection(state);
    state
        .app
        .seed_running_run_for_tests(session_id, objective, &selection)
        .expect("seeded run should start")
}

fn explicit_runtime_selection(state: &BootstrapState) -> AgentRuntimeSelection {
    state
        .app
        .seed_auth_profile_for_tests(ta_store::connected_test_auth_profile(
            "profile-openai-test",
            "openai-chatgpt",
            "openai",
        ))
        .expect("test auth profile should persist");
    AgentRuntimeSelection {
        runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
            .expect("runtime profile id"),
        auth_profile_id: Some(AuthProfileId::new("profile-openai-test").expect("auth profile id")),
        model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
    }
}

fn codex_runtime_selection(state: &BootstrapState) -> AgentRuntimeSelection {
    state
        .app
        .seed_auth_profile_for_tests(ta_store::connected_test_auth_profile(
            "profile-codex-test",
            "codex-chatgpt",
            "codex",
        ))
        .expect("test auth profile should persist");
    AgentRuntimeSelection {
        runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
            .expect("runtime profile id"),
        auth_profile_id: Some(AuthProfileId::new("profile-codex-test").expect("auth profile id")),
        model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
    }
}

fn start_run_command(state: &BootstrapState, objective: &str) -> StartRunCommand {
    StartRunCommand::new(objective, explicit_runtime_selection(state))
}

mod approval_methods;
mod artifact_methods;
mod code_host_methods;
mod control_events;
mod git_methods;
mod initialize_sessions;
mod plugins;
mod receipt_methods;
mod recipe_methods;
mod run_methods;
mod scheduled_work_methods;
mod session_reads;
mod status_runtime;
mod subscribe_methods;
mod terminal_methods;
mod thread_workspace_methods;
mod work_item_methods;
mod workflow_methods;
mod workspace_file_methods;

fn test_session() -> JsonRpcServerSession {
    let (outbound_tx, _outbound_rx) = tokio::sync::mpsc::channel(1);
    JsonRpcServerSession::new(7, outbound_tx, Arc::new(AtomicBool::new(true)))
}

fn initialize_client(
    state: &BootstrapState,
    shutdown_requested: &Arc<AtomicBool>,
    session: &JsonRpcServerSession,
    session_state: &Arc<Mutex<DaemonRpcSessionState>>,
    client_name: &str,
) {
    handle_request(
        state,
        shutdown_requested,
        session,
        session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(99),
            method: METHOD_DAEMON_INITIALIZE.to_string(),
            params: Some(serde_json::json!({
                "clientName": client_name,
                "clientVersion": "0.0.1",
                "protocolVersion": DAEMON_PROTOCOL_VERSION,
                "capabilities": {
                    "notifications": true,
                    "eventSubscriptions": true,
                },
            })),
        },
    )
    .expect("daemon.initialize should succeed");
}
