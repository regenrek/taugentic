use crate::{
    ActivityPageQuery, AgentTurnsPageQuery, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    DaemonApprovalDecideParams, DaemonDiagnosticsParams, DaemonInitializeParams,
    DaemonNavigationIntentParams, DaemonNavigationSnapshotParams, DaemonNavigationSubscribeParams,
    DaemonProjectOpenParams, DaemonRunCancelParams, DaemonRunCompleteWithResultParams,
    DaemonSessionAttachParams, DaemonSessionOpenParams, DaemonStatusParams, DaemonSubscribeParams,
    DaemonWorkspaceGetParams, DaemonWorkspaceListParams, DaemonWorkspaceOpenParams, ForkRunRequest,
    GetAgentRuntimeQuery, GetArtifactQuery, GetRunQuery, GetRunTimelineQuery, GetSessionQuery,
    JsonRpcErrorObject, JsonRpcRequest, ListApprovalsQuery, ListArtifactsQuery,
    ListNativeRunsRequest, ListReceiptsRequest, ListRunsQuery, ListSessionsQuery,
    METHOD_DAEMON_ACTIVITY_PAGE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_AGENT_TURNS_PAGE,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_CONTEXT_RECEIPTS_LIST,
    METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE, METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE,
    METHOD_DAEMON_CONTROL_STATUS, METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_NAVIGATION_INTENT, METHOD_DAEMON_NAVIGATION_SNAPSHOT,
    METHOD_DAEMON_NAVIGATION_SUBSCRIBE, METHOD_DAEMON_PROJECT_OPEN, METHOD_DAEMON_RECIPES_LIST,
    METHOD_DAEMON_RUN_CANCEL, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT, METHOD_DAEMON_RUN_FORK,
    METHOD_DAEMON_RUN_GET, METHOD_DAEMON_RUN_LIST, METHOD_DAEMON_RUN_LIST_NATIVE,
    METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_RESUME, METHOD_DAEMON_RUN_START,
    METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS, METHOD_DAEMON_RUN_TIMELINE, METHOD_DAEMON_SESSION_ATTACH,
    METHOD_DAEMON_SESSION_GET, METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN,
    METHOD_DAEMON_SESSION_OVERVIEW, METHOD_DAEMON_STATUS, METHOD_DAEMON_SUBSCRIBE,
    METHOD_DAEMON_WORK_ITEM_DISMISS, METHOD_DAEMON_WORK_ITEM_LIST, METHOD_DAEMON_WORK_ITEM_REFRESH,
    METHOD_DAEMON_WORK_ITEM_TRIGGER, METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST,
    METHOD_DAEMON_WORKSPACE_OPEN, METHOD_WORKFLOW_LOAD, METHOD_WORKFLOW_RELOAD,
    METHOD_WORKFLOW_STATUS, METHOD_WORKFLOW_VALIDATE, PromoteReceiptRequest,
    QuarantineReceiptRequest, ResumeRunRequest, SessionOverviewQuery, StartRunCommand,
    SubscribeRunEventsRequest, WorkItemDismissParams, WorkItemListQuery, WorkItemRefreshParams,
    WorkItemTriggerParams, WorkflowLoadParams, WorkflowReloadParams, WorkflowValidateParams,
    host::internal_stop::{InternalDaemonStopParams, METHOD_DAEMON_INTERNAL_STOP},
    method_not_found, parse_params,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum DaemonRpcRequest {
    Initialize(DaemonInitializeParams),
    Status,
    ControlStatus,
    DiagnosticsSnapshot,
    SessionOpen(DaemonSessionOpenParams),
    SessionAttach(DaemonSessionAttachParams),
    WorkspaceOpen(DaemonWorkspaceOpenParams),
    WorkspaceList(DaemonWorkspaceListParams),
    WorkspaceGet(DaemonWorkspaceGetParams),
    ProjectOpen(DaemonProjectOpenParams),
    ActivityPage(ActivityPageQuery),
    AgentTurnsPage(AgentTurnsPageQuery),
    SessionList(ListSessionsQuery),
    SessionOverview(SessionOverviewQuery),
    NavigationSnapshot(DaemonNavigationSnapshotParams),
    NavigationIntent(DaemonNavigationIntentParams),
    NavigationSubscribe(DaemonNavigationSubscribeParams),
    SessionGet(GetSessionQuery),
    ApprovalList(ListApprovalsQuery),
    ApprovalDecide(DaemonApprovalDecideParams),
    WorkItemList(WorkItemListQuery),
    WorkItemRefresh(WorkItemRefreshParams),
    WorkItemDismiss(WorkItemDismissParams),
    WorkItemTrigger(WorkItemTriggerParams),
    ArtifactGet(GetArtifactQuery),
    ArtifactList(ListArtifactsQuery),
    ContextReceiptsList(ListReceiptsRequest),
    ContextReceiptsPromote(PromoteReceiptRequest),
    ContextReceiptsQuarantine(QuarantineReceiptRequest),
    RunStart(StartRunCommand),
    RunCompleteWithResult(DaemonRunCompleteWithResultParams),
    RunResume(ResumeRunRequest),
    RunFork(ForkRunRequest),
    RunReplayEvents(SubscribeRunEventsRequest),
    RunSubscribeEvents(SubscribeRunEventsRequest),
    RunCancel(DaemonRunCancelParams),
    RunList(ListRunsQuery),
    RunListNative(ListNativeRunsRequest),
    RunGet(GetRunQuery),
    RunTimeline(GetRunTimelineQuery),
    RecipesList,
    AgentRuntimeGet(GetAgentRuntimeQuery),
    AgentRuntimeProfilePatch(DaemonAgentRuntimePatchProfileParams),
    AgentRuntimeAuthLogin(DaemonAgentRuntimeAuthLoginParams),
    AgentRuntimeAuthLoginComplete(DaemonAgentRuntimeAuthLoginCompleteParams),
    AgentRuntimeAuthLogout(DaemonAgentRuntimeAuthLogoutParams),
    AgentRuntimeExtensionSet(DaemonAgentRuntimeSetExtensionEnabledParams),
    WorkflowLoad(WorkflowLoadParams),
    WorkflowStatus,
    WorkflowReload(WorkflowReloadParams),
    WorkflowValidate(WorkflowValidateParams),
    InternalStop(InternalDaemonStopParams),
    Subscribe(DaemonSubscribeParams),
}

impl DaemonRpcRequest {
    pub(super) fn parse(request: &JsonRpcRequest) -> Result<Self, JsonRpcErrorObject> {
        match request.method.as_str() {
            METHOD_DAEMON_INITIALIZE => Ok(Self::Initialize(parse_params(request)?)),
            METHOD_DAEMON_STATUS => {
                let _: DaemonStatusParams = parse_params(request)?;
                Ok(Self::Status)
            }
            METHOD_DAEMON_CONTROL_STATUS => {
                let _: DaemonStatusParams = parse_params(request)?;
                Ok(Self::ControlStatus)
            }
            METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT => {
                let _: DaemonDiagnosticsParams = parse_params(request)?;
                Ok(Self::DiagnosticsSnapshot)
            }
            METHOD_DAEMON_SESSION_OPEN => Ok(Self::SessionOpen(parse_params(request)?)),
            METHOD_DAEMON_SESSION_ATTACH => Ok(Self::SessionAttach(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_OPEN => Ok(Self::WorkspaceOpen(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_LIST => Ok(Self::WorkspaceList(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_GET => Ok(Self::WorkspaceGet(parse_params(request)?)),
            METHOD_DAEMON_PROJECT_OPEN => Ok(Self::ProjectOpen(parse_params(request)?)),
            METHOD_DAEMON_ACTIVITY_PAGE => Ok(Self::ActivityPage(parse_params(request)?)),
            METHOD_DAEMON_AGENT_TURNS_PAGE => Ok(Self::AgentTurnsPage(parse_params(request)?)),
            METHOD_DAEMON_SESSION_LIST => Ok(Self::SessionList(parse_params(request)?)),
            METHOD_DAEMON_SESSION_OVERVIEW => Ok(Self::SessionOverview(parse_params(request)?)),
            METHOD_DAEMON_NAVIGATION_SNAPSHOT => {
                Ok(Self::NavigationSnapshot(parse_params(request)?))
            }
            METHOD_DAEMON_NAVIGATION_INTENT => Ok(Self::NavigationIntent(parse_params(request)?)),
            METHOD_DAEMON_NAVIGATION_SUBSCRIBE => {
                Ok(Self::NavigationSubscribe(parse_params(request)?))
            }
            METHOD_DAEMON_SESSION_GET => Ok(Self::SessionGet(parse_params(request)?)),
            METHOD_DAEMON_APPROVAL_LIST => Ok(Self::ApprovalList(parse_params(request)?)),
            METHOD_DAEMON_APPROVAL_DECIDE => Ok(Self::ApprovalDecide(parse_params(request)?)),
            METHOD_DAEMON_WORK_ITEM_LIST => Ok(Self::WorkItemList(parse_params(request)?)),
            METHOD_DAEMON_WORK_ITEM_REFRESH => Ok(Self::WorkItemRefresh(parse_params(request)?)),
            METHOD_DAEMON_WORK_ITEM_DISMISS => Ok(Self::WorkItemDismiss(parse_params(request)?)),
            METHOD_DAEMON_WORK_ITEM_TRIGGER => Ok(Self::WorkItemTrigger(parse_params(request)?)),
            METHOD_DAEMON_ARTIFACT_GET => Ok(Self::ArtifactGet(parse_params(request)?)),
            METHOD_DAEMON_ARTIFACT_LIST => Ok(Self::ArtifactList(parse_params(request)?)),
            METHOD_DAEMON_CONTEXT_RECEIPTS_LIST => {
                Ok(Self::ContextReceiptsList(parse_params(request)?))
            }
            METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE => {
                Ok(Self::ContextReceiptsPromote(parse_params(request)?))
            }
            METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE => {
                Ok(Self::ContextReceiptsQuarantine(parse_params(request)?))
            }
            METHOD_DAEMON_RUN_START => Ok(Self::RunStart(parse_params(request)?)),
            METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT => {
                Ok(Self::RunCompleteWithResult(parse_params(request)?))
            }
            METHOD_DAEMON_RUN_RESUME => Ok(Self::RunResume(parse_params(request)?)),
            METHOD_DAEMON_RUN_FORK => Ok(Self::RunFork(parse_params(request)?)),
            METHOD_DAEMON_RUN_REPLAY_EVENTS => Ok(Self::RunReplayEvents(parse_params(request)?)),
            METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS => {
                Ok(Self::RunSubscribeEvents(parse_params(request)?))
            }
            METHOD_DAEMON_RUN_CANCEL => Ok(Self::RunCancel(parse_params(request)?)),
            METHOD_DAEMON_RUN_LIST => Ok(Self::RunList(parse_params(request)?)),
            METHOD_DAEMON_RUN_LIST_NATIVE => Ok(Self::RunListNative(parse_params(request)?)),
            METHOD_DAEMON_RUN_GET => Ok(Self::RunGet(parse_params(request)?)),
            METHOD_DAEMON_RUN_TIMELINE => Ok(Self::RunTimeline(parse_params(request)?)),
            METHOD_DAEMON_RECIPES_LIST => Ok(Self::RecipesList),
            METHOD_DAEMON_AGENT_RUNTIME_GET => Ok(Self::AgentRuntimeGet(parse_params(request)?)),
            METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH => {
                Ok(Self::AgentRuntimeProfilePatch(parse_params(request)?))
            }
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN => {
                Ok(Self::AgentRuntimeAuthLogin(parse_params(request)?))
            }
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE => {
                Ok(Self::AgentRuntimeAuthLoginComplete(parse_params(request)?))
            }
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT => {
                Ok(Self::AgentRuntimeAuthLogout(parse_params(request)?))
            }
            METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET => {
                Ok(Self::AgentRuntimeExtensionSet(parse_params(request)?))
            }
            METHOD_WORKFLOW_LOAD => Ok(Self::WorkflowLoad(parse_params(request)?)),
            METHOD_WORKFLOW_STATUS => {
                let _: WorkflowReloadParams = parse_params(request)?;
                Ok(Self::WorkflowStatus)
            }
            METHOD_WORKFLOW_RELOAD => Ok(Self::WorkflowReload(parse_params(request)?)),
            METHOD_WORKFLOW_VALIDATE => Ok(Self::WorkflowValidate(parse_params(request)?)),
            METHOD_DAEMON_INTERNAL_STOP => Ok(Self::InternalStop(parse_params(request)?)),
            METHOD_DAEMON_SUBSCRIBE => Ok(Self::Subscribe(parse_params(request)?)),
            _ => Err(method_not_found(&request.method)),
        }
    }
}
