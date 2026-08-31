use crate::{
    ActivityPageQuery, AgentTurnsPageQuery, BrowserActionRequest, BrowserClearDataRequest,
    BrowserProfileRequest, CancelScheduledWorkRequest, CodeHostAccountConnectParams,
    CodeHostAccountDisconnectParams, CodeHostAccountListParams, CodeHostPullRequestActivityParams,
    CodeHostPullRequestChecksParams, CodeHostPullRequestCommentCreateParams,
    CodeHostPullRequestDetailParams, CodeHostPullRequestEnsureParams,
    CodeHostPullRequestListParams, CodeHostPushApplyParams, CodeHostPushPrepareParams,
    CodeHostRepositoryContextParams, ContinueRunRequest, CreateScheduledWorkRequest,
    DaemonAgentRuntimeAuthLoginCompleteParams, DaemonAgentRuntimeAuthLoginParams,
    DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimeAuthProfilePreferencesSetParams,
    DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    DaemonApprovalDecideParams, DaemonDiagnosticsParams, DaemonInitializeParams,
    DaemonNavigationIntentParams, DaemonNavigationSnapshotParams, DaemonNavigationSubscribeParams,
    DaemonProjectOpenParams, DaemonRunCancelParams, DaemonRunCompleteWithResultParams,
    DaemonSessionAttachParams, DaemonSessionOpenParams, DaemonSessionSetNextRunSelectionParams,
    DaemonStatusParams, DaemonSubscribeParams, DaemonWorkspaceGetParams, DaemonWorkspaceListParams,
    DaemonWorkspaceOpenParams, ForkRunRequest, GetAgentRuntimeQuery, GetArtifactQuery, GetRunQuery,
    GetRunTimelineQuery, GetSessionQuery, GitCheckpointApplyRevertParams, GitCheckpointListParams,
    GitCheckpointPrepareRevertParams, GitCommitParams, GitDiffParams, GitPathsMutationParams,
    GitRepositorySnapshotParams, InspectPluginPackageRequest, InstallPluginPackageRequest,
    JoinRunRequest, JsonRpcErrorObject, JsonRpcRequest, ListApprovalsQuery, ListArtifactsQuery,
    ListNativeRunsRequest, ListPluginInstallationsRequest, ListReceiptsRequest, ListRunsQuery,
    ListScheduledWorkRequest, ListSessionsQuery, METHOD_DAEMON_ACTIVITY_PAGE,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN, METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
    METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET,
    METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET, METHOD_DAEMON_AGENT_RUNTIME_GET,
    METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH, METHOD_DAEMON_AGENT_TURNS_PAGE,
    METHOD_DAEMON_APPROVAL_DECIDE, METHOD_DAEMON_APPROVAL_LIST, METHOD_DAEMON_ARTIFACT_GET,
    METHOD_DAEMON_ARTIFACT_LIST, METHOD_DAEMON_BROWSER_ACTION, METHOD_DAEMON_BROWSER_CLEAR_DATA,
    METHOD_DAEMON_BROWSER_PROFILE, METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT,
    METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT, METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
    METHOD_DAEMON_CODE_HOST_PUSH_PREPARE, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
    METHOD_DAEMON_CONTEXT_RECEIPTS_LIST, METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE,
    METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE, METHOD_DAEMON_CONTROL_STATUS,
    METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT, METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT,
    METHOD_DAEMON_GIT_CHECKPOINT_LIST, METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT,
    METHOD_DAEMON_GIT_COMMIT, METHOD_DAEMON_GIT_DIFF, METHOD_DAEMON_GIT_SNAPSHOT,
    METHOD_DAEMON_GIT_STAGE, METHOD_DAEMON_GIT_UNSTAGE, METHOD_DAEMON_INITIALIZE,
    METHOD_DAEMON_NAVIGATION_INTENT, METHOD_DAEMON_NAVIGATION_SNAPSHOT,
    METHOD_DAEMON_NAVIGATION_SUBSCRIBE, METHOD_DAEMON_PLUGIN_INSPECT, METHOD_DAEMON_PLUGIN_INSTALL,
    METHOD_DAEMON_PLUGIN_LIST, METHOD_DAEMON_PLUGIN_UNINSTALL, METHOD_DAEMON_PROJECT_OPEN,
    METHOD_DAEMON_RECIPES_LIST, METHOD_DAEMON_RUN_CANCEL, METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT,
    METHOD_DAEMON_RUN_CONTINUE, METHOD_DAEMON_RUN_FORK, METHOD_DAEMON_RUN_GET,
    METHOD_DAEMON_RUN_JOIN, METHOD_DAEMON_RUN_LINEAGE_GRAPH, METHOD_DAEMON_RUN_LIST,
    METHOD_DAEMON_RUN_LIST_NATIVE, METHOD_DAEMON_RUN_REPLAY_EVENTS, METHOD_DAEMON_RUN_RESUME,
    METHOD_DAEMON_RUN_SPAWN, METHOD_DAEMON_RUN_START, METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    METHOD_DAEMON_RUN_SWITCH_ACCOUNT_AND_RESUME, METHOD_DAEMON_RUN_TIMELINE,
    METHOD_DAEMON_SCHEDULED_WORK_CANCEL, METHOD_DAEMON_SCHEDULED_WORK_CREATE,
    METHOD_DAEMON_SCHEDULED_WORK_LIST, METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_GET,
    METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN, METHOD_DAEMON_SESSION_OVERVIEW,
    METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION, METHOD_DAEMON_STATUS, METHOD_DAEMON_SUBSCRIBE,
    METHOD_DAEMON_TERMINAL_ATTACH, METHOD_DAEMON_TERMINAL_CLOSE, METHOD_DAEMON_TERMINAL_DETACH,
    METHOD_DAEMON_TERMINAL_INPUT, METHOD_DAEMON_TERMINAL_LIST, METHOD_DAEMON_TERMINAL_RESIZE,
    METHOD_DAEMON_TERMINAL_SPAWN, METHOD_DAEMON_THREAD_WORKSPACE_GET,
    METHOD_DAEMON_THREAD_WORKSPACE_UPDATE, METHOD_DAEMON_WORK_ITEM_DISMISS,
    METHOD_DAEMON_WORK_ITEM_LIST, METHOD_DAEMON_WORK_ITEM_REFRESH, METHOD_DAEMON_WORK_ITEM_TRIGGER,
    METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL, METHOD_DAEMON_WORKSPACE_FILE_READ,
    METHOD_DAEMON_WORKSPACE_FILE_TREE, METHOD_DAEMON_WORKSPACE_FILE_WRITE,
    METHOD_DAEMON_WORKSPACE_GET, METHOD_DAEMON_WORKSPACE_LIST, METHOD_DAEMON_WORKSPACE_OPEN,
    METHOD_WORKFLOW_LOAD, METHOD_WORKFLOW_RELOAD, METHOD_WORKFLOW_STATUS, METHOD_WORKFLOW_VALIDATE,
    PromoteReceiptRequest, QuarantineReceiptRequest, ResumeRunRequest, RunLineageGraphRequest,
    SessionOverviewQuery, SpawnRunRequest, StartRunCommand, SubscribeRunEventsRequest,
    SwitchAccountAndResumeRequest, TerminalAttachParams, TerminalCloseParams, TerminalDetachParams,
    TerminalInputParams, TerminalListParams, TerminalResizeParams, TerminalSpawnParams,
    ThreadWorkspaceQuery, ThreadWorkspaceUpdateCommand, UninstallPluginRequest,
    VoiceStreamEndParams, VoiceStreamExchangeParams, VoiceStreamOpenParams, WorkItemDismissParams,
    WorkItemListQuery, WorkItemRefreshParams, WorkItemTriggerParams, WorkflowLoadParams,
    WorkflowReloadParams, WorkflowValidateParams, WorkspaceFileOpenExternalParams,
    WorkspaceFileReadParams, WorkspaceFileTreeParams, WorkspaceFileWriteParams,
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
    SessionSetNextRunSelection(DaemonSessionSetNextRunSelectionParams),
    WorkspaceOpen(DaemonWorkspaceOpenParams),
    WorkspaceList(DaemonWorkspaceListParams),
    WorkspaceGet(DaemonWorkspaceGetParams),
    WorkspaceFileTree(WorkspaceFileTreeParams),
    WorkspaceFileRead(WorkspaceFileReadParams),
    WorkspaceFileWrite(WorkspaceFileWriteParams),
    WorkspaceFileOpenExternal(WorkspaceFileOpenExternalParams),
    CodeHostAccountList(CodeHostAccountListParams),
    CodeHostAccountConnect(CodeHostAccountConnectParams),
    CodeHostAccountDisconnect(CodeHostAccountDisconnectParams),
    CodeHostRepositoryContext(CodeHostRepositoryContextParams),
    CodeHostPushPrepare(CodeHostPushPrepareParams),
    CodeHostPushApply(CodeHostPushApplyParams),
    CodeHostPullRequestList(CodeHostPullRequestListParams),
    CodeHostPullRequestDetail(CodeHostPullRequestDetailParams),
    CodeHostPullRequestEnsure(CodeHostPullRequestEnsureParams),
    CodeHostPullRequestChecks(CodeHostPullRequestChecksParams),
    CodeHostPullRequestActivity(CodeHostPullRequestActivityParams),
    CodeHostPullRequestCommentCreate(CodeHostPullRequestCommentCreateParams),
    GitSnapshot(GitRepositorySnapshotParams),
    GitDiff(GitDiffParams),
    GitStage(GitPathsMutationParams),
    GitUnstage(GitPathsMutationParams),
    GitCommit(GitCommitParams),
    GitCheckpointList(GitCheckpointListParams),
    GitCheckpointPrepareRevert(GitCheckpointPrepareRevertParams),
    GitCheckpointApplyRevert(GitCheckpointApplyRevertParams),
    TerminalSpawn(TerminalSpawnParams),
    TerminalList(TerminalListParams),
    TerminalAttach(TerminalAttachParams),
    TerminalInput(TerminalInputParams),
    TerminalResize(TerminalResizeParams),
    TerminalDetach(TerminalDetachParams),
    TerminalClose(TerminalCloseParams),
    ProjectOpen(DaemonProjectOpenParams),
    ActivityPage(ActivityPageQuery),
    AgentTurnsPage(AgentTurnsPageQuery),
    ThreadWorkspaceGet(ThreadWorkspaceQuery),
    ThreadWorkspaceUpdate(ThreadWorkspaceUpdateCommand),
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
    ScheduledWorkCreate(CreateScheduledWorkRequest),
    ScheduledWorkList(ListScheduledWorkRequest),
    ScheduledWorkCancel(CancelScheduledWorkRequest),
    PluginInspect(InspectPluginPackageRequest),
    PluginInstall(InstallPluginPackageRequest),
    PluginList(ListPluginInstallationsRequest),
    PluginUninstall(UninstallPluginRequest),
    BrowserProfile(BrowserProfileRequest),
    BrowserAction(BrowserActionRequest),
    BrowserClearData(BrowserClearDataRequest),
    ArtifactGet(GetArtifactQuery),
    ArtifactList(ListArtifactsQuery),
    ContextReceiptsList(ListReceiptsRequest),
    ContextReceiptsPromote(PromoteReceiptRequest),
    ContextReceiptsQuarantine(QuarantineReceiptRequest),
    RunStart(StartRunCommand),
    RunCompleteWithResult(DaemonRunCompleteWithResultParams),
    RunResume(ResumeRunRequest),
    RunFork(ForkRunRequest),
    RunContinue(ContinueRunRequest),
    RunSwitchAccountAndResume(SwitchAccountAndResumeRequest),
    RunSpawn(SpawnRunRequest),
    RunJoin(JoinRunRequest),
    RunReplayEvents(SubscribeRunEventsRequest),
    RunSubscribeEvents(SubscribeRunEventsRequest),
    RunCancel(DaemonRunCancelParams),
    RunList(ListRunsQuery),
    RunListNative(ListNativeRunsRequest),
    RunLineageGraph(RunLineageGraphRequest),
    RunGet(GetRunQuery),
    RunTimeline(GetRunTimelineQuery),
    VoiceOpen(VoiceStreamOpenParams),
    VoiceExchange(VoiceStreamExchangeParams),
    VoiceEnd(VoiceStreamEndParams),
    RecipesList,
    AgentRuntimeGet(GetAgentRuntimeQuery),
    AgentRuntimeProfilePatch(DaemonAgentRuntimePatchProfileParams),
    AgentRuntimeAuthLogin(DaemonAgentRuntimeAuthLoginParams),
    AgentRuntimeAuthLoginComplete(DaemonAgentRuntimeAuthLoginCompleteParams),
    AgentRuntimeAuthLogout(DaemonAgentRuntimeAuthLogoutParams),
    AgentRuntimeAuthProfilePreferencesSet(DaemonAgentRuntimeAuthProfilePreferencesSetParams),
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
            METHOD_DAEMON_SESSION_SET_NEXT_RUN_SELECTION => {
                Ok(Self::SessionSetNextRunSelection(parse_params(request)?))
            }
            METHOD_DAEMON_WORKSPACE_OPEN => Ok(Self::WorkspaceOpen(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_LIST => Ok(Self::WorkspaceList(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_GET => Ok(Self::WorkspaceGet(parse_params(request)?)),
            METHOD_DAEMON_WORKSPACE_FILE_TREE => {
                Ok(Self::WorkspaceFileTree(parse_params(request)?))
            }
            METHOD_DAEMON_WORKSPACE_FILE_READ => {
                Ok(Self::WorkspaceFileRead(parse_params(request)?))
            }
            METHOD_DAEMON_WORKSPACE_FILE_WRITE => {
                Ok(Self::WorkspaceFileWrite(parse_params(request)?))
            }
            METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL => {
                Ok(Self::WorkspaceFileOpenExternal(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST => {
                Ok(Self::CodeHostAccountList(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT => {
                Ok(Self::CodeHostAccountConnect(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT => {
                Ok(Self::CodeHostAccountDisconnect(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT => {
                Ok(Self::CodeHostRepositoryContext(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PUSH_PREPARE => {
                Ok(Self::CodeHostPushPrepare(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PUSH_APPLY => {
                Ok(Self::CodeHostPushApply(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST => {
                Ok(Self::CodeHostPullRequestList(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL => {
                Ok(Self::CodeHostPullRequestDetail(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE => {
                Ok(Self::CodeHostPullRequestEnsure(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS => {
                Ok(Self::CodeHostPullRequestChecks(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY => {
                Ok(Self::CodeHostPullRequestActivity(parse_params(request)?))
            }
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE => Ok(
                Self::CodeHostPullRequestCommentCreate(parse_params(request)?),
            ),
            METHOD_DAEMON_GIT_SNAPSHOT => Ok(Self::GitSnapshot(parse_params(request)?)),
            METHOD_DAEMON_GIT_DIFF => Ok(Self::GitDiff(parse_params(request)?)),
            METHOD_DAEMON_GIT_STAGE => Ok(Self::GitStage(parse_params(request)?)),
            METHOD_DAEMON_GIT_UNSTAGE => Ok(Self::GitUnstage(parse_params(request)?)),
            METHOD_DAEMON_GIT_COMMIT => Ok(Self::GitCommit(parse_params(request)?)),
            METHOD_DAEMON_GIT_CHECKPOINT_LIST => {
                Ok(Self::GitCheckpointList(parse_params(request)?))
            }
            METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT => {
                Ok(Self::GitCheckpointPrepareRevert(parse_params(request)?))
            }
            METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT => {
                Ok(Self::GitCheckpointApplyRevert(parse_params(request)?))
            }
            METHOD_DAEMON_TERMINAL_SPAWN => Ok(Self::TerminalSpawn(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_LIST => Ok(Self::TerminalList(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_ATTACH => Ok(Self::TerminalAttach(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_INPUT => Ok(Self::TerminalInput(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_RESIZE => Ok(Self::TerminalResize(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_DETACH => Ok(Self::TerminalDetach(parse_params(request)?)),
            METHOD_DAEMON_TERMINAL_CLOSE => Ok(Self::TerminalClose(parse_params(request)?)),
            METHOD_DAEMON_PROJECT_OPEN => Ok(Self::ProjectOpen(parse_params(request)?)),
            METHOD_DAEMON_ACTIVITY_PAGE => Ok(Self::ActivityPage(parse_params(request)?)),
            METHOD_DAEMON_AGENT_TURNS_PAGE => Ok(Self::AgentTurnsPage(parse_params(request)?)),
            METHOD_DAEMON_THREAD_WORKSPACE_GET => {
                Ok(Self::ThreadWorkspaceGet(parse_params(request)?))
            }
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE => {
                Ok(Self::ThreadWorkspaceUpdate(parse_params(request)?))
            }
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
            METHOD_DAEMON_SCHEDULED_WORK_CREATE => {
                Ok(Self::ScheduledWorkCreate(parse_params(request)?))
            }
            METHOD_DAEMON_SCHEDULED_WORK_LIST => {
                Ok(Self::ScheduledWorkList(parse_params(request)?))
            }
            METHOD_DAEMON_SCHEDULED_WORK_CANCEL => {
                Ok(Self::ScheduledWorkCancel(parse_params(request)?))
            }
            METHOD_DAEMON_PLUGIN_INSPECT => Ok(Self::PluginInspect(parse_params(request)?)),
            METHOD_DAEMON_PLUGIN_INSTALL => Ok(Self::PluginInstall(parse_params(request)?)),
            METHOD_DAEMON_PLUGIN_LIST => Ok(Self::PluginList(parse_params(request)?)),
            METHOD_DAEMON_PLUGIN_UNINSTALL => Ok(Self::PluginUninstall(parse_params(request)?)),
            METHOD_DAEMON_BROWSER_PROFILE => Ok(Self::BrowserProfile(parse_params(request)?)),
            METHOD_DAEMON_BROWSER_ACTION => Ok(Self::BrowserAction(parse_params(request)?)),
            METHOD_DAEMON_BROWSER_CLEAR_DATA => Ok(Self::BrowserClearData(parse_params(request)?)),
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
            METHOD_DAEMON_RUN_CONTINUE => Ok(Self::RunContinue(parse_params(request)?)),
            METHOD_DAEMON_RUN_SWITCH_ACCOUNT_AND_RESUME => {
                Ok(Self::RunSwitchAccountAndResume(parse_params(request)?))
            }
            METHOD_DAEMON_RUN_SPAWN => Ok(Self::RunSpawn(parse_params(request)?)),
            METHOD_DAEMON_RUN_JOIN => Ok(Self::RunJoin(parse_params(request)?)),
            METHOD_DAEMON_RUN_REPLAY_EVENTS => Ok(Self::RunReplayEvents(parse_params(request)?)),
            METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS => {
                Ok(Self::RunSubscribeEvents(parse_params(request)?))
            }
            METHOD_DAEMON_RUN_CANCEL => Ok(Self::RunCancel(parse_params(request)?)),
            METHOD_DAEMON_RUN_LIST => Ok(Self::RunList(parse_params(request)?)),
            METHOD_DAEMON_RUN_LIST_NATIVE => Ok(Self::RunListNative(parse_params(request)?)),
            METHOD_DAEMON_RUN_LINEAGE_GRAPH => Ok(Self::RunLineageGraph(parse_params(request)?)),
            METHOD_DAEMON_RUN_GET => Ok(Self::RunGet(parse_params(request)?)),
            METHOD_DAEMON_RUN_TIMELINE => Ok(Self::RunTimeline(parse_params(request)?)),
            crate::METHOD_DAEMON_VOICE_OPEN => Ok(Self::VoiceOpen(parse_params(request)?)),
            crate::METHOD_DAEMON_VOICE_EXCHANGE => Ok(Self::VoiceExchange(parse_params(request)?)),
            crate::METHOD_DAEMON_VOICE_END => Ok(Self::VoiceEnd(parse_params(request)?)),
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
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_PROFILE_PREFERENCES_SET => Ok(
                Self::AgentRuntimeAuthProfilePreferencesSet(parse_params(request)?),
            ),
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
