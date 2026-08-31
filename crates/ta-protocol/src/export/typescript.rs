use std::{fs, path::Path};

use ts_rs::{Config as TsConfig, TS};

use crate::wire::*;

use super::typescript_generated::{
    rewrite_generated_typescript_imports, write_generated_index, write_generated_runtime_index,
    write_generated_schema_runtime,
};
use super::{PROTOCOL_VERSION, ProtocolExportError};

/// `(export name, value)` — shared between `generated/index.ts` and `generated/index.js`.
pub(super) const PROTOCOL_TS_CONSTS: &[(&str, &str)] = &[
    ("PROTOCOL_VERSION", PROTOCOL_VERSION),
    ("DAEMON_DEFAULT_SOCKET_NAME", DAEMON_DEFAULT_SOCKET_NAME),
    ("DAEMON_SOCKET_NAME_ENV_VAR", DAEMON_SOCKET_NAME_ENV_VAR),
    ("METHOD_DAEMON_EVENT", METHOD_DAEMON_EVENT),
    (
        "METHOD_DAEMON_BROWSER_PROFILE",
        METHOD_DAEMON_BROWSER_PROFILE,
    ),
    ("METHOD_DAEMON_BROWSER_ACTION", METHOD_DAEMON_BROWSER_ACTION),
    (
        "METHOD_DAEMON_BROWSER_CLEAR_DATA",
        METHOD_DAEMON_BROWSER_CLEAR_DATA,
    ),
    ("METHOD_DAEMON_INITIALIZE", METHOD_DAEMON_INITIALIZE),
    ("METHOD_DAEMON_STATUS", METHOD_DAEMON_STATUS),
    ("METHOD_DAEMON_CONTROL_STATUS", METHOD_DAEMON_CONTROL_STATUS),
    (
        "METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT",
        METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT,
    ),
    ("METHOD_DAEMON_SESSION_OPEN", METHOD_DAEMON_SESSION_OPEN),
    ("METHOD_DAEMON_SESSION_ATTACH", METHOD_DAEMON_SESSION_ATTACH),
    ("METHOD_DAEMON_SESSION_LIST", METHOD_DAEMON_SESSION_LIST),
    ("METHOD_DAEMON_SESSION_GET", METHOD_DAEMON_SESSION_GET),
    (
        "METHOD_DAEMON_SESSION_OVERVIEW",
        METHOD_DAEMON_SESSION_OVERVIEW,
    ),
    ("METHOD_DAEMON_WORKSPACE_OPEN", METHOD_DAEMON_WORKSPACE_OPEN),
    ("METHOD_DAEMON_WORKSPACE_LIST", METHOD_DAEMON_WORKSPACE_LIST),
    ("METHOD_DAEMON_WORKSPACE_GET", METHOD_DAEMON_WORKSPACE_GET),
    ("METHOD_DAEMON_GIT_SNAPSHOT", METHOD_DAEMON_GIT_SNAPSHOT),
    ("METHOD_DAEMON_GIT_DIFF", METHOD_DAEMON_GIT_DIFF),
    ("METHOD_DAEMON_GIT_STAGE", METHOD_DAEMON_GIT_STAGE),
    ("METHOD_DAEMON_GIT_UNSTAGE", METHOD_DAEMON_GIT_UNSTAGE),
    ("METHOD_DAEMON_GIT_COMMIT", METHOD_DAEMON_GIT_COMMIT),
    (
        "METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST",
        METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT",
        METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT",
        METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT",
        METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PUSH_PREPARE",
        METHOD_DAEMON_CODE_HOST_PUSH_PREPARE,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PUSH_APPLY",
        METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY,
    ),
    (
        "METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE",
        METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
    ),
    (
        "METHOD_DAEMON_GIT_CHECKPOINT_LIST",
        METHOD_DAEMON_GIT_CHECKPOINT_LIST,
    ),
    (
        "METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT",
        METHOD_DAEMON_GIT_CHECKPOINT_PREPARE_REVERT,
    ),
    (
        "METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT",
        METHOD_DAEMON_GIT_CHECKPOINT_APPLY_REVERT,
    ),
    ("METHOD_DAEMON_TERMINAL_SPAWN", METHOD_DAEMON_TERMINAL_SPAWN),
    ("METHOD_DAEMON_TERMINAL_LIST", METHOD_DAEMON_TERMINAL_LIST),
    (
        "METHOD_DAEMON_TERMINAL_ATTACH",
        METHOD_DAEMON_TERMINAL_ATTACH,
    ),
    ("METHOD_DAEMON_TERMINAL_INPUT", METHOD_DAEMON_TERMINAL_INPUT),
    (
        "METHOD_DAEMON_TERMINAL_RESIZE",
        METHOD_DAEMON_TERMINAL_RESIZE,
    ),
    (
        "METHOD_DAEMON_TERMINAL_DETACH",
        METHOD_DAEMON_TERMINAL_DETACH,
    ),
    ("METHOD_DAEMON_TERMINAL_CLOSE", METHOD_DAEMON_TERMINAL_CLOSE),
    ("METHOD_DAEMON_TERMINAL_EVENT", METHOD_DAEMON_TERMINAL_EVENT),
    ("METHOD_DAEMON_ACTIVITY_PAGE", METHOD_DAEMON_ACTIVITY_PAGE),
    (
        "METHOD_DAEMON_AGENT_TURNS_PAGE",
        METHOD_DAEMON_AGENT_TURNS_PAGE,
    ),
    (
        "METHOD_DAEMON_APPROVAL_DECIDE",
        METHOD_DAEMON_APPROVAL_DECIDE,
    ),
    ("METHOD_DAEMON_APPROVAL_LIST", METHOD_DAEMON_APPROVAL_LIST),
    ("METHOD_DAEMON_WORK_ITEM_LIST", METHOD_DAEMON_WORK_ITEM_LIST),
    (
        "METHOD_DAEMON_WORK_ITEM_REFRESH",
        METHOD_DAEMON_WORK_ITEM_REFRESH,
    ),
    (
        "METHOD_DAEMON_WORK_ITEM_DISMISS",
        METHOD_DAEMON_WORK_ITEM_DISMISS,
    ),
    (
        "METHOD_DAEMON_WORK_ITEM_TRIGGER",
        METHOD_DAEMON_WORK_ITEM_TRIGGER,
    ),
    ("METHOD_DAEMON_ARTIFACT_GET", METHOD_DAEMON_ARTIFACT_GET),
    ("METHOD_DAEMON_ARTIFACT_LIST", METHOD_DAEMON_ARTIFACT_LIST),
    (
        "METHOD_DAEMON_CONTEXT_RECEIPTS_LIST",
        METHOD_DAEMON_CONTEXT_RECEIPTS_LIST,
    ),
    (
        "METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE",
        METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE,
    ),
    (
        "METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE",
        METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE,
    ),
    (
        "METHOD_DAEMON_NAVIGATION_SNAPSHOT",
        METHOD_DAEMON_NAVIGATION_SNAPSHOT,
    ),
    (
        "METHOD_DAEMON_NAVIGATION_INTENT",
        METHOD_DAEMON_NAVIGATION_INTENT,
    ),
    ("METHOD_DAEMON_RUN_START", METHOD_DAEMON_RUN_START),
    ("METHOD_DAEMON_RUN_CANCEL", METHOD_DAEMON_RUN_CANCEL),
    ("METHOD_DAEMON_RUN_RESUME", METHOD_DAEMON_RUN_RESUME),
    ("METHOD_DAEMON_RUN_FORK", METHOD_DAEMON_RUN_FORK),
    ("METHOD_DAEMON_RUN_CONTINUE", METHOD_DAEMON_RUN_CONTINUE),
    (
        "METHOD_DAEMON_RUN_REPLAY_EVENTS",
        METHOD_DAEMON_RUN_REPLAY_EVENTS,
    ),
    (
        "METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS",
        METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
    ),
    ("METHOD_DAEMON_RUN_EVENT", METHOD_DAEMON_RUN_EVENT),
    ("METHOD_DAEMON_RUN_LIST", METHOD_DAEMON_RUN_LIST),
    (
        "METHOD_DAEMON_SCHEDULED_WORK_CREATE",
        METHOD_DAEMON_SCHEDULED_WORK_CREATE,
    ),
    (
        "METHOD_DAEMON_SCHEDULED_WORK_LIST",
        METHOD_DAEMON_SCHEDULED_WORK_LIST,
    ),
    ("METHOD_DAEMON_PLUGIN_INSPECT", METHOD_DAEMON_PLUGIN_INSPECT),
    ("METHOD_DAEMON_PLUGIN_INSTALL", METHOD_DAEMON_PLUGIN_INSTALL),
    ("METHOD_DAEMON_PLUGIN_LIST", METHOD_DAEMON_PLUGIN_LIST),
    (
        "METHOD_DAEMON_PLUGIN_UNINSTALL",
        METHOD_DAEMON_PLUGIN_UNINSTALL,
    ),
    (
        "METHOD_DAEMON_SCHEDULED_WORK_CANCEL",
        METHOD_DAEMON_SCHEDULED_WORK_CANCEL,
    ),
    (
        "METHOD_DAEMON_RUN_LIST_NATIVE",
        METHOD_DAEMON_RUN_LIST_NATIVE,
    ),
    ("METHOD_DAEMON_RUN_GET", METHOD_DAEMON_RUN_GET),
    ("METHOD_DAEMON_RUN_TIMELINE", METHOD_DAEMON_RUN_TIMELINE),
    ("METHOD_DAEMON_RECIPES_LIST", METHOD_DAEMON_RECIPES_LIST),
    ("METHOD_DAEMON_SUBSCRIBE", METHOD_DAEMON_SUBSCRIBE),
    ("METHOD_WORKFLOW_LOAD", METHOD_WORKFLOW_LOAD),
    ("METHOD_WORKFLOW_STATUS", METHOD_WORKFLOW_STATUS),
    ("METHOD_WORKFLOW_RELOAD", METHOD_WORKFLOW_RELOAD),
    ("METHOD_WORKFLOW_VALIDATE", METHOD_WORKFLOW_VALIDATE),
];

pub(super) struct TypeScriptPublicType {
    pub(super) name: &'static str,
    export: fn(&TsConfig) -> Result<(), ProtocolExportError>,
}

pub(super) const CANCELLATION_TYPES: &[TypeScriptPublicType] = &[TypeScriptPublicType {
    name: "DaemonRunCancelParams",
    export: export_ts::<DaemonRunCancelParams>,
}];

/// `(export name, value)` — numeric protocol constants shared between
/// `generated/index.ts` and `generated/index.js`.
pub(super) const PROTOCOL_TS_NUMBER_CONSTS: &[(&str, u32)] = &[
    (
        "DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT",
        DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT,
    ),
    (
        "MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT",
        MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT,
    ),
    (
        "CONTEXT_RECEIPT_LIST_MAX_LIMIT",
        CONTEXT_RECEIPT_LIST_MAX_LIMIT,
    ),
    (
        "NATIVE_RUN_LIST_DEFAULT_LIMIT",
        NATIVE_RUN_LIST_DEFAULT_LIMIT,
    ),
    ("NATIVE_RUN_LIST_MAX_LIMIT", NATIVE_RUN_LIST_MAX_LIMIT),
    ("TERMINAL_MIN_ROWS", TERMINAL_MIN_ROWS as u32),
    ("TERMINAL_MAX_ROWS", TERMINAL_MAX_ROWS as u32),
    ("TERMINAL_MIN_COLS", TERMINAL_MIN_COLS as u32),
    ("TERMINAL_MAX_COLS", TERMINAL_MAX_COLS as u32),
    ("TERMINAL_INPUT_MAX_BYTES", TERMINAL_INPUT_MAX_BYTES as u32),
    ("GIT_PATCH_MAX_BYTES", GIT_PATCH_MAX_BYTES as u32),
    ("GIT_STATUS_MAX_ENTRIES", GIT_STATUS_MAX_ENTRIES as u32),
    (
        "GIT_COMMIT_MESSAGE_MAX_BYTES",
        GIT_COMMIT_MESSAGE_MAX_BYTES as u32,
    ),
    (
        "TERMINAL_OUTPUT_CHUNK_MAX_BYTES",
        TERMINAL_OUTPUT_CHUNK_MAX_BYTES as u32,
    ),
    (
        "TERMINAL_SNAPSHOT_MAX_BYTES",
        TERMINAL_SNAPSHOT_MAX_BYTES as u32,
    ),
    (
        "RUN_TIMELINE_EVENT_DEFAULT_LIMIT",
        RUN_TIMELINE_EVENT_DEFAULT_LIMIT,
    ),
    ("RUN_TIMELINE_EVENT_MAX_LIMIT", RUN_TIMELINE_EVENT_MAX_LIMIT),
];

pub fn export_typescript_bindings(shared_package_dir: &Path) -> Result<(), ProtocolExportError> {
    let generated_dir = shared_package_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;

    let cfg = TsConfig::new()
        .with_out_dir(shared_package_dir)
        .with_import_extension(Some("js"));

    export_core_types(&cfg)?;
    export_agent_runtime_types(&cfg)?;

    write_generated_index(&generated_dir)?;
    write_generated_runtime_index(&generated_dir)?;
    write_generated_schema_runtime(&generated_dir)?;
    rewrite_generated_typescript_imports(&generated_dir)?;

    Ok(())
}

fn export_ts<T: TS + 'static>(cfg: &TsConfig) -> Result<(), ProtocolExportError> {
    T::export_all(cfg)?;
    Ok(())
}

fn export_selected_types(
    types: &[TypeScriptPublicType],
    cfg: &TsConfig,
) -> Result<(), ProtocolExportError> {
    for public_type in types {
        (public_type.export)(cfg)?;
    }
    Ok(())
}

fn export_core_types(cfg: &TsConfig) -> Result<(), ProtocolExportError> {
    export_ts::<AgentStreamTurnId>(cfg)?;
    export_ts::<AgentStreamItemId>(cfg)?;
    export_ts::<AgentToolCallOutcome>(cfg)?;
    export_ts::<RuntimeLanePendingState>(cfg)?;
    export_ts::<AgentStreamEvent>(cfg)?;
    export_ts::<StreamEmission>(cfg)?;
    export_ts::<AgentStreamFrame>(cfg)?;
    export_ts::<VoicePermissionState>(cfg)?;
    export_ts::<VoicePhase>(cfg)?;
    export_ts::<VoiceEvent>(cfg)?;
    export_ts::<BudgetScope>(cfg)?;
    export_ts::<BudgetMetric>(cfg)?;
    export_ts::<BudgetBreach>(cfg)?;
    export_ts::<BudgetSnapshot>(cfg)?;
    export_ts::<BudgetExceededEvent>(cfg)?;
    export_ts::<BudgetEvent>(cfg)?;
    export_ts::<ArtifactId>(cfg)?;
    export_ts::<ArtifactKind>(cfg)?;
    export_ts::<ArtifactMetadata>(cfg)?;
    export_ts::<ImageMediaType>(cfg)?;
    export_ts::<ImageArtifactProvenance>(cfg)?;
    export_ts::<ImageArtifactMetadata>(cfg)?;
    export_ts::<ArtifactEvent>(cfg)?;
    export_ts::<ArtifactSummary>(cfg)?;
    export_ts::<ArtifactContentResult>(cfg)?;
    export_ts::<ApprovalDecision>(cfg)?;
    export_ts::<ApprovalResolutionReason>(cfg)?;
    export_ts::<ApprovalTarget>(cfg)?;
    export_ts::<PublicApprovalResolution>(cfg)?;
    export_ts::<ApprovalScope>(cfg)?;
    export_ts::<ActivityCursor>(cfg)?;
    export_ts::<ActivityPageQuery>(cfg)?;
    export_ts::<PublicActivityPageItem>(cfg)?;
    export_ts::<PublicActivityPageResult>(cfg)?;
    export_ts::<AgentTurnsPageQuery>(cfg)?;
    export_ts::<AgentUserRow>(cfg)?;
    export_ts::<AgentAssistantRow>(cfg)?;
    export_ts::<AgentToolCallRow>(cfg)?;
    export_ts::<AgentPendingStateRow>(cfg)?;
    export_ts::<AgentTurnRow>(cfg)?;
    export_ts::<AgentTurnsPageResult>(cfg)?;
    export_ts::<ThreadWorkspaceQuery>(cfg)?;
    export_ts::<ThreadWorkspaceUpdateCommand>(cfg)?;
    export_ts::<ThreadWorkspaceMutation>(cfg)?;
    export_ts::<ThreadWorkspacePin>(cfg)?;
    export_ts::<ThreadWorkspaceWorkLogEntry>(cfg)?;
    export_ts::<ThreadWorkspaceWorkLogKind>(cfg)?;
    export_ts::<ThreadWorkspaceResult>(cfg)?;
    export_ts::<DaemonEventCursor>(cfg)?;
    export_ts::<PublicApprovalEvent>(cfg)?;
    export_ts::<ContextReceipt>(cfg)?;
    export_ts::<ContextReceiptEvent>(cfg)?;
    export_ts::<PublicContextReceipt>(cfg)?;
    export_ts::<PublicContextReceiptEvent>(cfg)?;
    export_ts::<ReceiptKind>(cfg)?;
    export_ts::<ReceiptProvenance>(cfg)?;
    export_ts::<ReceiptState>(cfg)?;
    export_ts::<RunFailureKind>(cfg)?;
    export_ts::<RunStatusReason>(cfg)?;
    export_ts::<RunStatusEvent>(cfg)?;
    export_ts::<ScheduledWorkId>(cfg)?;
    export_ts::<ScheduledWorkOccurrenceId>(cfg)?;
    export_ts::<ScheduledWorkExecutionRequest>(cfg)?;
    export_ts::<ScheduledWorkUnpublishedResource>(cfg)?;
    export_ts::<ScheduledWorkAttentionPolicy>(cfg)?;
    export_ts::<ScheduledWorkDefinition>(cfg)?;
    export_ts::<ScheduledWorkOccurrenceState>(cfg)?;
    export_ts::<ScheduledWorkPreparationTerminal>(cfg)?;
    export_ts::<ScheduledWorkOccurrence>(cfg)?;
    export_ts::<CreateScheduledWorkRequest>(cfg)?;
    export_ts::<CreateScheduledWorkResult>(cfg)?;
    export_ts::<ListScheduledWorkRequest>(cfg)?;
    export_ts::<ListScheduledWorkResult>(cfg)?;
    export_ts::<CancelScheduledWorkRequest>(cfg)?;
    export_ts::<PluginId>(cfg)?;
    export_ts::<BrowserProfileId>(cfg)?;
    export_ts::<BrowserProfile>(cfg)?;
    export_ts::<BrowserNavigationKind>(cfg)?;
    export_ts::<BrowserNavigationRequest>(cfg)?;
    export_ts::<BrowserActionKind>(cfg)?;
    export_ts::<BrowserActionRequest>(cfg)?;
    export_ts::<BrowserActionDecision>(cfg)?;
    export_ts::<BrowserActionResult>(cfg)?;
    export_ts::<BrowserProfileRequest>(cfg)?;
    export_ts::<BrowserProfileResult>(cfg)?;
    export_ts::<BrowserClearDataRequest>(cfg)?;
    export_ts::<PluginCapability>(cfg)?;
    export_ts::<PluginLifecycleState>(cfg)?;
    export_ts::<PluginLifecycleFailure>(cfg)?;
    export_ts::<PluginInspection>(cfg)?;
    export_ts::<PluginInstallation>(cfg)?;
    export_ts::<InspectPluginPackageRequest>(cfg)?;
    export_ts::<InstallPluginPackageRequest>(cfg)?;
    export_ts::<InstallPluginPackageResult>(cfg)?;
    export_ts::<ListPluginInstallationsRequest>(cfg)?;
    export_ts::<ListPluginInstallationsResult>(cfg)?;
    export_ts::<UninstallPluginRequest>(cfg)?;
    export_ts::<RunEvent>(cfg)?;
    export_ts::<RunReconciledOnStartupEvent>(cfg)?;
    export_ts::<TokenUsageRecordedEvent>(cfg)?;
    export_ts::<TokenUsageTotals>(cfg)?;
    export_ts::<PublicDaemonEvent>(cfg)?;
    export_ts::<PublicDaemonEventEnvelope>(cfg)?;
    export_ts::<DaemonEventKind>(cfg)?;
    export_ts::<ConflictEvent>(cfg)?;
    export_ts::<WorkspaceMode>(cfg)?;
    export_ts::<WorktreeCleanupPolicy>(cfg)?;
    export_ts::<WorktreeInfo>(cfg)?;
    export_ts::<FileClaimKind>(cfg)?;
    export_ts::<ConflictSeverity>(cfg)?;
    export_ts::<FileClaimConflict>(cfg)?;
    export_ts::<ConflictWarning>(cfg)?;
    export_ts::<ConflictSummary>(cfg)?;
    export_ts::<WorkspaceId>(cfg)?;
    export_ts::<WorkspacePath>(cfg)?;
    export_ts::<WorkspacePathError>(cfg)?;
    export_ts::<Workspace>(cfg)?;
    export_ts::<DaemonWorkspaceOpenParams>(cfg)?;
    export_ts::<DaemonWorkspaceOpenResult>(cfg)?;
    export_ts::<DaemonProjectOpenParams>(cfg)?;
    export_ts::<DaemonProjectOpenResult>(cfg)?;
    export_ts::<DaemonWorkspaceListParams>(cfg)?;
    export_ts::<DaemonWorkspaceListResult>(cfg)?;
    export_ts::<DaemonWorkspaceGetParams>(cfg)?;
    export_ts::<DaemonWorkspaceGetResult>(cfg)?;
    export_ts::<WorkspaceFileKind>(cfg)?;
    export_ts::<WorkspaceFileEntry>(cfg)?;
    export_ts::<WorkspaceFileAttachmentRequest>(cfg)?;
    export_ts::<WorkspaceFileAttachment>(cfg)?;
    export_ts::<NativeImagePreview>(cfg)?;
    export_ts::<WorkspaceFileTreeParams>(cfg)?;
    export_ts::<WorkspaceFileTreeResult>(cfg)?;
    export_ts::<WorkspaceFileReadParams>(cfg)?;
    export_ts::<BoundedFileContent>(cfg)?;
    export_ts::<WorkspaceFileReadResult>(cfg)?;
    export_ts::<WorkspaceFileWriteParams>(cfg)?;
    export_ts::<WorkspaceFileWriteResult>(cfg)?;
    export_ts::<WorkspaceFileOpenExternalParams>(cfg)?;
    export_ts::<WorkspaceFileOpenExternalResult>(cfg)?;
    export_ts::<GitChangeKind>(cfg)?;
    export_ts::<GitFileStatus>(cfg)?;
    export_ts::<GitWorktreeSummary>(cfg)?;
    export_ts::<GitRepositorySnapshot>(cfg)?;
    export_ts::<GitRepositorySnapshotParams>(cfg)?;
    export_ts::<GitRepositorySnapshotResult>(cfg)?;
    export_ts::<GitDiffScope>(cfg)?;
    export_ts::<GitDiffParams>(cfg)?;
    export_ts::<GitDiffResult>(cfg)?;
    export_ts::<GitPathsMutationParams>(cfg)?;
    export_ts::<GitCommitParams>(cfg)?;
    export_ts::<GitMutationResult>(cfg)?;
    export_ts::<GitCheckpointPhase>(cfg)?;
    export_ts::<GitCheckpointSummary>(cfg)?;
    export_ts::<GitCheckpointListParams>(cfg)?;
    export_ts::<GitCheckpointListResult>(cfg)?;
    export_ts::<GitCheckpointPrepareRevertParams>(cfg)?;
    export_ts::<GitCheckpointPrepareRevertResult>(cfg)?;
    export_ts::<GitCheckpointApplyRevertParams>(cfg)?;
    export_ts::<CodeHostAccountId>(cfg)?;
    export_ts::<CodeHostPullRequestId>(cfg)?;
    export_ts::<CodeHostProviderKind>(cfg)?;
    export_ts::<CodeHostAccount>(cfg)?;
    export_ts::<CodeHostAccountListParams>(cfg)?;
    export_ts::<CodeHostAccountListResult>(cfg)?;
    export_ts::<CodeHostAccountConnectParams>(cfg)?;
    export_ts::<CodeHostAccountConnectResult>(cfg)?;
    export_ts::<CodeHostAccountDisconnectParams>(cfg)?;
    export_ts::<CodeHostAccountDisconnectResult>(cfg)?;
    export_ts::<CodeHostRepositoryRef>(cfg)?;
    export_ts::<CodeHostRemote>(cfg)?;
    export_ts::<CodeHostRepositoryContextParams>(cfg)?;
    export_ts::<CodeHostRepositoryContextResult>(cfg)?;
    export_ts::<CodeHostCommitSummary>(cfg)?;
    export_ts::<CodeHostPushPrepareParams>(cfg)?;
    export_ts::<CodeHostPushPrepareResult>(cfg)?;
    export_ts::<CodeHostPushApplyParams>(cfg)?;
    export_ts::<CodeHostPushApplyResult>(cfg)?;
    export_ts::<CodeHostPullRequestState>(cfg)?;
    export_ts::<CodeHostPullRequestSummary>(cfg)?;
    export_ts::<CodeHostPullRequestDetail>(cfg)?;
    export_ts::<CodeHostPage>(cfg)?;
    export_ts::<CodeHostPullRequestListParams>(cfg)?;
    export_ts::<CodeHostPullRequestDetailParams>(cfg)?;
    export_ts::<CodeHostPullRequestEnsureParams>(cfg)?;
    export_ts::<CodeHostPullRequestEnsureResult>(cfg)?;
    export_ts::<CodeHostCheckStatus>(cfg)?;
    export_ts::<CodeHostCheck>(cfg)?;
    export_ts::<CodeHostPullRequestChecksResult>(cfg)?;
    export_ts::<CodeHostPullRequestChecksParams>(cfg)?;
    export_ts::<CodeHostCommentKind>(cfg)?;
    export_ts::<CodeHostComment>(cfg)?;
    export_ts::<CodeHostReview>(cfg)?;
    export_ts::<CodeHostTimelineItem>(cfg)?;
    export_ts::<CodeHostPullRequestActivityResult>(cfg)?;
    export_ts::<CodeHostPullRequestActivityParams>(cfg)?;
    export_ts::<CodeHostPullRequestCommentCreateParams>(cfg)?;
    export_ts::<CodeHostPullRequestCommentCreateResult>(cfg)?;
    export_ts::<TerminalSessionId>(cfg)?;
    export_ts::<TerminalSessionStatus>(cfg)?;
    export_ts::<TerminalSessionSummary>(cfg)?;
    export_ts::<TerminalSpawnParams>(cfg)?;
    export_ts::<TerminalSpawnResult>(cfg)?;
    export_ts::<TerminalListParams>(cfg)?;
    export_ts::<TerminalListResult>(cfg)?;
    export_ts::<TerminalAttachParams>(cfg)?;
    export_ts::<TerminalAttachResult>(cfg)?;
    export_ts::<TerminalInputParams>(cfg)?;
    export_ts::<TerminalInputResult>(cfg)?;
    export_ts::<TerminalResizeParams>(cfg)?;
    export_ts::<TerminalResizeResult>(cfg)?;
    export_ts::<TerminalDetachParams>(cfg)?;
    export_ts::<TerminalDetachResult>(cfg)?;
    export_ts::<TerminalCloseParams>(cfg)?;
    export_ts::<TerminalCloseResult>(cfg)?;
    export_ts::<TerminalStreamEvent>(cfg)?;
    export_ts::<TerminalEventParams>(cfg)?;
    export_ts::<TrustState>(cfg)?;
    export_ts::<ExecutionContext>(cfg)?;
    export_ts::<WorkspaceScope>(cfg)?;
    export_ts::<SandboxProfile>(cfg)?;
    export_ts::<ProcessExecPolicy>(cfg)?;
    export_ts::<PermissionPolicy>(cfg)?;
    export_ts::<NetworkPolicy>(cfg)?;
    export_ts::<EnvPolicy>(cfg)?;
    export_ts::<WorkspaceCapabilityUnsupported>(cfg)?;
    export_ts::<SessionOverviewQuery>(cfg)?;
    export_ts::<SessionOverviewResult>(cfg)?;
    export_ts::<SessionOverviewLaneStatus>(cfg)?;
    export_ts::<ApprovalAttentionState>(cfg)?;
    export_ts::<SpaceId>(cfg)?;
    export_ts::<ProjectId>(cfg)?;
    export_ts::<NavigationSpace>(cfg)?;
    export_ts::<NavigationProject>(cfg)?;
    export_ts::<ConversationPlacement>(cfg)?;
    export_ts::<NavigationAttention>(cfg)?;
    export_ts::<NavigationConversation>(cfg)?;
    export_ts::<NavigationAgentRow>(cfg)?;
    export_ts::<NavigationSnapshot>(cfg)?;
    export_ts::<DaemonNavigationSnapshotParams>(cfg)?;
    export_ts::<DaemonNavigationSnapshotResult>(cfg)?;
    export_ts::<DaemonNavigationIntent>(cfg)?;
    export_ts::<DaemonNavigationIntentParams>(cfg)?;
    export_ts::<DaemonNavigationIntentResult>(cfg)?;
    export_ts::<DaemonNavigationSubscribeParams>(cfg)?;
    export_ts::<DaemonNavigationSubscribeResult>(cfg)?;
    export_ts::<DaemonNavigationInvalidatedParams>(cfg)?;
    export_ts::<DesktopDaemonLifecycleStatus>(cfg)?;
    export_ts::<DesktopDaemonLifecycleProjection>(cfg)?;
    export_ts::<DaemonActualRuntimeMode>(cfg)?;
    export_ts::<DaemonClientCapabilities>(cfg)?;
    export_ts::<DaemonControlAction>(cfg)?;
    export_ts::<DaemonControlErrorCode>(cfg)?;
    export_ts::<DaemonServerCapabilities>(cfg)?;
    export_ts::<DaemonInitializeParams>(cfg)?;
    export_ts::<DaemonInitializeResult>(cfg)?;
    export_ts::<DaemonPendingTransitionKind>(cfg)?;
    export_ts::<DaemonPendingTransitionView>(cfg)?;
    export_ts::<DaemonRuntimeMode>(cfg)?;
    export_ts::<WorkspaceSelector>(cfg)?;
    export_ts::<DaemonSessionOpenParams>(cfg)?;
    export_ts::<DaemonSessionOpenResult>(cfg)?;
    export_ts::<DaemonSessionAttachParams>(cfg)?;
    export_ts::<DaemonSessionAttachResult>(cfg)?;
    export_ts::<DaemonApprovalDecideParams>(cfg)?;
    export_ts::<DaemonApprovalDecideResult>(cfg)?;
    export_ts::<DaemonStatusParams>(cfg)?;
    export_ts::<DaemonStatusResult>(cfg)?;
    export_ts::<DaemonDiagnosticsParams>(cfg)?;
    export_ts::<DaemonDiagnostics>(cfg)?;
    export_ts::<DaemonDiagnosticError>(cfg)?;
    export_ts::<DaemonDiagnosticTokenUsage>(cfg)?;
    export_ts::<DaemonSandboxCapabilitySnapshot>(cfg)?;
    export_ts::<DaemonProviderHealthDiagnostic>(cfg)?;
    export_ts::<DaemonControlStatusResult>(cfg)?;
    export_ts::<DelegateRequest>(cfg)?;
    export_ts::<DaemonSubscribeParams>(cfg)?;
    export_ts::<DaemonSubscribeResult>(cfg)?;
    export_ts::<DaemonTransitionStatus>(cfg)?;
    export_ts::<ListSessionsQuery>(cfg)?;
    export_ts::<GetSessionQuery>(cfg)?;
    export_ts::<ListApprovalsQuery>(cfg)?;
    export_ts::<ApprovalSnapshotResult>(cfg)?;
    export_ts::<crate::wire::WorkItemKey>(cfg)?;
    export_ts::<crate::wire::WorkSourceKind>(cfg)?;
    export_ts::<crate::wire::WorkSource>(cfg)?;
    export_ts::<crate::wire::WorkItemStatus>(cfg)?;
    export_ts::<crate::wire::WorkItem>(cfg)?;
    export_ts::<crate::wire::SourceCursor>(cfg)?;
    export_ts::<WorkItemListQuery>(cfg)?;
    export_ts::<WorkItemListResult>(cfg)?;
    export_ts::<WorkSourceSyncStatus>(cfg)?;
    export_ts::<WorkSourceSyncState>(cfg)?;
    export_ts::<WorkItemRefreshParams>(cfg)?;
    export_ts::<WorkItemDismissParams>(cfg)?;
    export_ts::<WorkItemDismissResult>(cfg)?;
    export_ts::<WorkItemTriggerParams>(cfg)?;
    export_ts::<WorkItemTriggerResult>(cfg)?;
    export_ts::<GetArtifactQuery>(cfg)?;
    export_ts::<GetRunQuery>(cfg)?;
    export_ts::<ListRunsQuery>(cfg)?;
    export_ts::<RunListFilter>(cfg)?;
    export_ts::<ListNativeRunsRequest>(cfg)?;
    export_ts::<RunListEntry>(cfg)?;
    export_ts::<ListNativeRunsResult>(cfg)?;
    export_ts::<RunLineageGraphRequest>(cfg)?;
    export_ts::<RunLineageGraphEdge>(cfg)?;
    export_ts::<RunLineageGraphResult>(cfg)?;
    export_ts::<GetRunTimelineQuery>(cfg)?;
    export_ts::<RunTimeline>(cfg)?;
    export_ts::<RunTimelineRun>(cfg)?;
    export_ts::<RunTimelineEventKind>(cfg)?;
    export_ts::<RunTimelineEvent>(cfg)?;
    export_ts::<RunTimelineEventPayload>(cfg)?;
    export_ts::<ListArtifactsQuery>(cfg)?;
    export_ts::<ArtifactSnapshotResult>(cfg)?;
    export_ts::<ListReceiptsRequest>(cfg)?;
    export_ts::<ListReceiptsResult>(cfg)?;
    export_ts::<PromoteReceiptRequest>(cfg)?;
    export_ts::<QuarantineReceiptRequest>(cfg)?;
    export_ts::<OutputContractKind>(cfg)?;
    export_ts::<CapsuleResult>(cfg)?;
    export_ts::<DebugResult>(cfg)?;
    export_ts::<PatchResult>(cfg)?;
    export_ts::<ReviewResult>(cfg)?;
    export_ts::<ReviewVerdict>(cfg)?;
    export_ts::<ReviewFinding>(cfg)?;
    export_ts::<FindingSeverity>(cfg)?;
    export_ts::<TestResult>(cfg)?;
    export_ts::<PlanResult>(cfg)?;
    export_ts::<PlanStep>(cfg)?;
    export_ts::<ValidationError>(cfg)?;
    export_ts::<CapsuleRecipe>(cfg)?;
    export_ts::<ListRecipesParams>(cfg)?;
    export_ts::<RecipeListResponse>(cfg)?;
    export_ts::<RecipeValidationError>(cfg)?;
    export_ts::<RecipeResolutionError>(cfg)?;
    export_ts::<SessionOverview>(cfg)?;
    export_ts::<SessionSummary>(cfg)?;
    export_ts::<RunHarnessKind>(cfg)?;
    export_ts::<RunDetail>(cfg)?;
    export_ts::<RunRecord>(cfg)?;
    export_ts::<RunSummary>(cfg)?;
    export_ts::<ResumeRunRequest>(cfg)?;
    export_ts::<ResumeRunResult>(cfg)?;
    export_ts::<ResumeRunState>(cfg)?;
    export_ts::<ForkRunRequest>(cfg)?;
    export_ts::<ForkRunResult>(cfg)?;
    export_ts::<ContinueRunRequest>(cfg)?;
    export_ts::<ContinueRunResult>(cfg)?;
    export_ts::<SwitchRouteAndResumeRequest>(cfg)?;
    export_ts::<SwitchRouteAndResumeResult>(cfg)?;
    export_ts::<SpawnRunRequest>(cfg)?;
    export_ts::<SpawnRunResult>(cfg)?;
    export_ts::<JoinRunRequest>(cfg)?;
    export_ts::<JoinRunResult>(cfg)?;
    export_ts::<SubscribeRunEventsRequest>(cfg)?;
    export_ts::<RunEventDelta>(cfg)?;
    export_ts::<RunEventStreamError>(cfg)?;
    export_ts::<RunEventStreamPayload>(cfg)?;
    export_ts::<RunEventStreamItem>(cfg)?;
    export_ts::<SubscribeRunEventsResult>(cfg)?;
    export_ts::<StartRunCommand>(cfg)?;
    export_ts::<DaemonRunCompleteWithResultParams>(cfg)?;
    export_selected_types(CANCELLATION_TYPES, cfg)?;
    export_ts::<WorkflowDefinition>(cfg)?;
    export_ts::<WorkflowSourceBinding>(cfg)?;
    export_ts::<WorkflowSourceKind>(cfg)?;
    export_ts::<WorkflowOrchestratorPolicy>(cfg)?;
    export_ts::<WorkflowRetryPolicy>(cfg)?;
    export_ts::<WorkflowPolicy>(cfg)?;
    export_ts::<WorkflowApprovalPolicy>(cfg)?;
    export_ts::<WorkflowFileWriteApproval>(cfg)?;
    export_ts::<WorkflowProcessApproval>(cfg)?;
    export_ts::<WorkflowNetworkApproval>(cfg)?;
    export_ts::<WorkflowRuntimeProfileRef>(cfg)?;
    export_ts::<WorkflowOutputsPolicy>(cfg)?;
    export_ts::<WorkflowOutputRequirement>(cfg)?;
    export_ts::<WorkflowBudgets>(cfg)?;
    export_ts::<WorkflowBudgetLimits>(cfg)?;
    export_ts::<WorkflowLoadParams>(cfg)?;
    export_ts::<WorkflowReloadParams>(cfg)?;
    export_ts::<WorkflowValidateParams>(cfg)?;
    export_ts::<WorkflowValidationReport>(cfg)?;
    export_ts::<WorkflowValidationError>(cfg)?;
    export_ts::<WorkflowStatusResult>(cfg)?;
    export_ts::<WorkflowLoadedStatus>(cfg)?;
    export_ts::<WorkflowReloadOutcome>(cfg)?;
    Ok(())
}

fn export_agent_runtime_types(cfg: &TsConfig) -> Result<(), ProtocolExportError> {
    export_ts::<AgentRuntimeStrategyId>(cfg)?;
    export_ts::<AgentRuntimeModelId>(cfg)?;
    export_ts::<AgentRuntimeModelRef>(cfg)?;
    export_ts::<AgentRuntimeMediaCapability>(cfg)?;
    export_ts::<AgentRuntimeMediaCapabilities>(cfg)?;
    export_ts::<AgentRuntimeStrategyHealthStatus>(cfg)?;
    export_ts::<AgentRuntimeStrategyHealth>(cfg)?;
    export_ts::<AgentRuntimeStrategyInfo>(cfg)?;
    export_ts::<AuthMethodId>(cfg)?;
    export_ts::<AuthMethodRef>(cfg)?;
    export_ts::<AuthProfileId>(cfg)?;
    export_ts::<AuthProfileConnectionState>(cfg)?;
    export_ts::<AuthProfileLoginMethod>(cfg)?;
    export_ts::<AuthProfileRef>(cfg)?;
    export_ts::<AuthProfilePreferences>(cfg)?;
    export_ts::<AuthProfileUsage>(cfg)?;
    export_ts::<AuthProfileUsageWindow>(cfg)?;
    export_ts::<AuthProfileState>(cfg)?;
    export_ts::<AuthProfileLoginChallenge>(cfg)?;
    export_ts::<AuthProfileLoginResult>(cfg)?;
    export_ts::<AuthProfileLogoutResult>(cfg)?;
    export_ts::<RuntimeExtensionId>(cfg)?;
    export_ts::<RuntimeExtensionDescriptor>(cfg)?;
    export_ts::<RuntimeExtensionAvailability>(cfg)?;
    export_ts::<RuntimeExtensionMcpServer>(cfg)?;
    export_ts::<RuntimeExtensionMcpStdioServer>(cfg)?;
    export_ts::<RuntimeExtensionMcpHttpServer>(cfg)?;
    export_ts::<RuntimeExtensionEnvVar>(cfg)?;
    export_ts::<RuntimeExtensionHttpHeader>(cfg)?;
    export_ts::<RuntimeExtensionState>(cfg)?;
    export_ts::<RuntimeProfileId>(cfg)?;
    export_ts::<RuntimePolicyMode>(cfg)?;
    export_ts::<RuntimeProfileExecutionKind>(cfg)?;
    export_ts::<RuntimeProfileSummary>(cfg)?;
    export_ts::<RuntimeProfilePatch>(cfg)?;
    export_ts::<AgentRuntimeSelection>(cfg)?;
    export_ts::<AgentRuntimeSnapshot>(cfg)?;
    export_ts::<GetAgentRuntimeQuery>(cfg)?;
    export_ts::<DaemonAgentRuntimePatchProfileParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeAuthLoginParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeAuthLoginCompleteParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeAuthLogoutParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeSetExtensionEnabledParams>(cfg)?;
    Ok(())
}
