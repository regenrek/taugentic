use std::{fs, path::Path};

use crate::wire::*;

use super::{
    ProtocolExportError, schema,
    typescript::{CANCELLATION_TYPES, PROTOCOL_TS_CONSTS, PROTOCOL_TS_NUMBER_CONSTS},
};

pub(super) fn write_generated_index(generated_dir: &Path) -> Result<(), ProtocolExportError> {
    let mut lines = Vec::new();
    append_core_generated_exports(&mut lines);
    append_agent_runtime_generated_exports(&mut lines);
    append_core_generated_consts(&mut lines);
    append_agent_runtime_generated_consts(&mut lines);
    lines.extend(
        PROTOCOL_TS_NUMBER_CONSTS
            .iter()
            .map(|(name, value)| format!("export const {name} = {value};")),
    );
    lines.push(String::new());

    fs::write(generated_dir.join("index.ts"), lines.join("\n"))?;
    Ok(())
}

pub(super) fn write_generated_runtime_index(
    generated_dir: &Path,
) -> Result<(), ProtocolExportError> {
    let mut lines = Vec::new();
    append_core_generated_consts(&mut lines);
    append_agent_runtime_generated_consts(&mut lines);
    lines.extend(
        PROTOCOL_TS_NUMBER_CONSTS
            .iter()
            .map(|(name, value)| format!("export const {name} = {value};")),
    );
    lines.push(String::new());

    fs::write(generated_dir.join("index.js"), lines.join("\n"))?;
    Ok(())
}

pub(super) fn write_generated_schema_runtime(
    generated_dir: &Path,
) -> Result<(), ProtocolExportError> {
    let schemas = schema::build_runtime_json_schemas()?;

    let runtime_body = render_runtime_schema_lines(&schemas, true)?;
    fs::write(generated_dir.join("runtime.ts"), runtime_body)?;
    let runtime_js_body = render_runtime_schema_lines(&schemas, false)?;
    fs::write(generated_dir.join("runtime.js"), runtime_js_body)?;
    Ok(())
}

fn append_core_generated_exports(lines: &mut Vec<String>) {
    for name in [
        "AgentStreamTurnId",
        "AgentStreamItemId",
        "AgentToolCallOutcome",
        "RuntimeLanePendingState",
        "AgentStreamEvent",
        "AgentStreamFrame",
        "BudgetScope",
        "BudgetMetric",
        "BudgetBreach",
        "BudgetSnapshot",
        "BudgetExceededEvent",
        "BudgetEvent",
        "ApprovalDecision",
        "ApprovalId",
        "ApprovalRequest",
        "PublicApprovalEvent",
        "PublicApprovalResolution",
        "ApprovalScope",
        "ActivityCursor",
        "ActivityPageQuery",
        "PublicActivityPageItem",
        "PublicActivityPageResult",
        "AgentTurnsPageQuery",
        "AgentUserRow",
        "AgentAssistantRow",
        "AgentToolCallRow",
        "AgentPendingStateRow",
        "AgentTurnRow",
        "AgentTurnsPageResult",
        "ThreadWorkspaceQuery",
        "ThreadWorkspaceUpdateCommand",
        "ThreadWorkspaceMutation",
        "ThreadWorkspacePin",
        "ThreadWorkspaceWorkLogEntry",
        "ThreadWorkspaceWorkLogKind",
        "ThreadWorkspaceResult",
        "ArtifactId",
        "ArtifactKind",
        "ArtifactMetadata",
        "ImageMediaType",
        "ImageArtifactProvenance",
        "ImageArtifactMetadata",
        "ArtifactEvent",
        "ArtifactSummary",
        "ArtifactContentResult",
        "ContextReceipt",
        "DaemonActualRuntimeMode",
        "DaemonClientCapabilities",
        "DaemonControlAction",
        "DaemonControlErrorCode",
        "DaemonEventCursor",
        "ContextReceiptEvent",
        "PublicContextReceipt",
        "PublicContextReceiptEvent",
        "ReceiptKind",
        "ReceiptProvenance",
        "ReceiptState",
        "RunFailureKind",
        "RunStatusReason",
        "RunStatusEvent",
        "ScheduledWorkId",
        "ScheduledWorkOccurrenceId",
        "ScheduledWorkExecutionRequest",
        "ScheduledWorkUnpublishedResource",
        "ScheduledWorkAttentionPolicy",
        "ScheduledWorkDefinition",
        "ScheduledWorkOccurrenceState",
        "ScheduledWorkPreparationTerminal",
        "ScheduledWorkOccurrence",
        "CreateScheduledWorkRequest",
        "CreateScheduledWorkResult",
        "ListScheduledWorkRequest",
        "ListScheduledWorkResult",
        "CancelScheduledWorkRequest",
        "PluginId",
        "PluginCapability",
        "PluginLifecycleState",
        "PluginLifecycleFailure",
        "PluginInspection",
        "PluginInstallation",
        "InspectPluginPackageRequest",
        "InstallPluginPackageRequest",
        "InstallPluginPackageResult",
        "ListPluginInstallationsRequest",
        "ListPluginInstallationsResult",
        "UninstallPluginRequest",
        "RunReconciledOnStartupEvent",
        "TokenUsageRecordedEvent",
        "TokenUsageTotals",
        "PublicDaemonEvent",
        "PublicDaemonEventEnvelope",
        "DaemonEventKind",
        "ConflictEvent",
        "WorkspaceMode",
        "WorktreeCleanupPolicy",
        "WorktreeInfo",
        "FileClaimKind",
        "ConflictSeverity",
        "FileClaimConflict",
        "ConflictWarning",
        "ConflictSummary",
        "WorkspaceId",
        "WorkspacePath",
        "WorkspacePathError",
        "Workspace",
        "DaemonWorkspaceOpenParams",
        "DaemonWorkspaceOpenResult",
        "DaemonProjectOpenParams",
        "DaemonProjectOpenResult",
        "DaemonWorkspaceListParams",
        "DaemonWorkspaceListResult",
        "DaemonWorkspaceGetParams",
        "DaemonWorkspaceGetResult",
        "WorkspaceFileKind",
        "WorkspaceFileEntry",
        "WorkspaceFileAttachmentRequest",
        "WorkspaceFileAttachment",
        "NativeImagePreview",
        "WorkspaceFileTreeParams",
        "WorkspaceFileTreeResult",
        "WorkspaceFileReadParams",
        "BoundedFileContent",
        "WorkspaceFileReadResult",
        "WorkspaceFileWriteParams",
        "WorkspaceFileWriteResult",
        "WorkspaceFileOpenExternalParams",
        "WorkspaceFileOpenExternalResult",
        "GitChangeKind",
        "GitFileStatus",
        "GitWorktreeSummary",
        "GitRepositorySnapshot",
        "GitRepositorySnapshotParams",
        "GitRepositorySnapshotResult",
        "GitDiffScope",
        "GitDiffParams",
        "GitDiffResult",
        "GitPathsMutationParams",
        "GitCommitParams",
        "GitMutationResult",
        "GitCheckpointPhase",
        "GitCheckpointSummary",
        "GitCheckpointListParams",
        "GitCheckpointListResult",
        "GitCheckpointPrepareRevertParams",
        "GitCheckpointPrepareRevertResult",
        "GitCheckpointApplyRevertParams",
        "CodeHostAccountId",
        "CodeHostPullRequestId",
        "CodeHostProviderKind",
        "CodeHostAccount",
        "CodeHostAccountListParams",
        "CodeHostAccountListResult",
        "CodeHostAccountConnectParams",
        "CodeHostAccountConnectResult",
        "CodeHostAccountDisconnectParams",
        "CodeHostAccountDisconnectResult",
        "CodeHostRepositoryRef",
        "CodeHostRemote",
        "CodeHostRepositoryContextParams",
        "CodeHostRepositoryContextResult",
        "CodeHostCommitSummary",
        "CodeHostPushPrepareParams",
        "CodeHostPushPrepareResult",
        "CodeHostPushApplyParams",
        "CodeHostPushApplyResult",
        "CodeHostPullRequestState",
        "CodeHostPullRequestSummary",
        "CodeHostPullRequestDetail",
        "CodeHostPage",
        "CodeHostPullRequestListParams",
        "CodeHostPullRequestDetailParams",
        "CodeHostPullRequestEnsureParams",
        "CodeHostPullRequestEnsureResult",
        "CodeHostCheckStatus",
        "CodeHostCheck",
        "CodeHostPullRequestChecksResult",
        "CodeHostPullRequestChecksParams",
        "CodeHostCommentKind",
        "CodeHostComment",
        "CodeHostReview",
        "CodeHostTimelineItem",
        "CodeHostPullRequestActivityResult",
        "CodeHostPullRequestActivityParams",
        "CodeHostPullRequestCommentCreateParams",
        "CodeHostPullRequestCommentCreateResult",
        "TerminalSessionId",
        "TerminalSessionStatus",
        "TerminalSessionSummary",
        "TerminalSpawnParams",
        "TerminalSpawnResult",
        "TerminalListParams",
        "TerminalListResult",
        "TerminalAttachParams",
        "TerminalAttachResult",
        "TerminalInputParams",
        "TerminalInputResult",
        "TerminalResizeParams",
        "TerminalResizeResult",
        "TerminalDetachParams",
        "TerminalDetachResult",
        "TerminalCloseParams",
        "TerminalCloseResult",
        "TerminalStreamEvent",
        "TerminalEventParams",
        "TrustState",
        "ExecutionContext",
        "WorkspaceScope",
        "SandboxProfile",
        "ProcessExecPolicy",
        "PermissionPolicy",
        "NetworkPolicy",
        "EnvPolicy",
        "WorkspaceCapabilityUnsupported",
        "SessionOverviewQuery",
        "SessionOverviewResult",
        "SessionOverviewLaneStatus",
        "ApprovalAttentionState",
        "SpaceId",
        "ProjectId",
        "NavigationSpace",
        "NavigationProject",
        "ConversationPlacement",
        "NavigationAttention",
        "NavigationConversation",
        "NavigationAgentRow",
        "NavigationSnapshot",
        "DaemonNavigationSnapshotParams",
        "DaemonNavigationSnapshotResult",
        "DaemonNavigationIntent",
        "DaemonNavigationIntentParams",
        "DaemonNavigationIntentResult",
        "DaemonNavigationSubscribeParams",
        "DaemonNavigationSubscribeResult",
        "DaemonNavigationInvalidatedParams",
        "DesktopDaemonLifecycleStatus",
        "DesktopDaemonLifecycleProjection",
        "DaemonInitializeParams",
        "DaemonInitializeResult",
        "DaemonPendingTransitionKind",
        "DaemonPendingTransitionView",
        "DaemonRuntimeMode",
        "WorkspaceSelector",
        "DaemonSessionOpenParams",
        "DaemonSessionOpenResult",
        "DaemonSessionAttachParams",
        "DaemonSessionAttachResult",
        "DaemonApprovalDecideParams",
        "DaemonApprovalDecideResult",
        "DaemonServerCapabilities",
        "DaemonControlStatusResult",
        "DelegateRequest",
        "DaemonStatusParams",
        "DaemonStatusResult",
        "DaemonDiagnosticsParams",
        "DaemonDiagnostics",
        "DaemonDiagnosticError",
        "DaemonDiagnosticTokenUsage",
        "DaemonSandboxCapabilitySnapshot",
        "DaemonProviderHealthDiagnostic",
        "DaemonSubscribeParams",
        "DaemonSubscribeResult",
        "DaemonTransitionStatus",
        "GetArtifactQuery",
        "ListApprovalsQuery",
        "ApprovalSnapshotResult",
        "WorkItemKey",
        "WorkSourceKind",
        "WorkSource",
        "WorkItemStatus",
        "WorkItem",
        "SourceCursor",
        "WorkItemListQuery",
        "WorkItemListResult",
        "WorkSourceSyncStatus",
        "WorkSourceSyncState",
        "WorkItemRefreshParams",
        "WorkItemDismissParams",
        "WorkItemDismissResult",
        "WorkItemTriggerParams",
        "WorkItemTriggerResult",
        "GetRunQuery",
        "GetSessionQuery",
        "ListRunsQuery",
        "RunListFilter",
        "ListNativeRunsRequest",
        "RunListEntry",
        "ListNativeRunsResult",
        "RunLineageGraphRequest",
        "RunLineageGraphEdge",
        "RunLineageGraphResult",
        "GetRunTimelineQuery",
        "RunTimeline",
        "RunTimelineRun",
        "RunTimelineEventKind",
        "RunTimelineEvent",
        "RunTimelineEventPayload",
        "ListArtifactsQuery",
        "ArtifactSnapshotResult",
        "ListReceiptsRequest",
        "ListReceiptsResult",
        "PromoteReceiptRequest",
        "QuarantineReceiptRequest",
        "OutputContractKind",
        "CapsuleResult",
        "DebugResult",
        "PatchResult",
        "ReviewResult",
        "ReviewVerdict",
        "ReviewFinding",
        "FindingSeverity",
        "TestResult",
        "PlanResult",
        "PlanStep",
        "ValidationError",
        "VoicePermissionState",
        "VoicePhase",
        "VoiceEvent",
        "CapsuleRecipe",
        "ListRecipesParams",
        "RecipeListResponse",
        "RecipeValidationError",
        "RecipeResolutionError",
        "ListSessionsQuery",
        "RunEvent",
        "RunEventDelta",
        "RunEventStreamError",
        "RunEventStreamItem",
        "RunEventStreamPayload",
        "RunHarnessKind",
        "RunDetail",
        "RunId",
        "RunRecord",
        "RunStatus",
        "RunSummary",
        "ResumeRunRequest",
        "ResumeRunResult",
        "ResumeRunState",
        "ForkRunRequest",
        "ForkRunResult",
        "ContinueRunRequest",
        "ContinueRunResult",
        "SwitchAccountAndResumeRequest",
        "SwitchAccountAndResumeResult",
        "SpawnRunRequest",
        "SpawnRunResult",
        "JoinRunRequest",
        "JoinRunResult",
        "SubscribeRunEventsRequest",
        "SubscribeRunEventsResult",
        "SessionAuthority",
        "SessionEvent",
        "SessionId",
        "SessionOverview",
        "SessionStatus",
        "SessionSummary",
        "StartRunCommand",
        "DaemonRunCompleteWithResultParams",
        "WorkflowDefinition",
        "WorkflowSourceBinding",
        "WorkflowSourceKind",
        "WorkflowOrchestratorPolicy",
        "WorkflowRetryPolicy",
        "WorkflowPolicy",
        "WorkflowApprovalPolicy",
        "WorkflowFileWriteApproval",
        "WorkflowProcessApproval",
        "WorkflowNetworkApproval",
        "WorkflowRuntimeProfileRef",
        "WorkflowOutputsPolicy",
        "WorkflowOutputRequirement",
        "WorkflowBudgets",
        "WorkflowBudgetLimits",
        "WorkflowLoadParams",
        "WorkflowReloadParams",
        "WorkflowValidateParams",
        "WorkflowValidationReport",
        "WorkflowValidationError",
        "WorkflowStatusResult",
        "WorkflowLoadedStatus",
        "WorkflowReloadOutcome",
    ] {
        lines.push(format!("export type {{ {name} }} from \"./{name}.js\";"));
    }

    for public_type in CANCELLATION_TYPES {
        lines.push(format!(
            "export type {{ {} }} from \"./{}.js\";",
            public_type.name, public_type.name
        ));
    }
}

fn append_agent_runtime_generated_exports(lines: &mut Vec<String>) {
    for name in [
        "AgentRuntimeStrategyId",
        "AgentRuntimeModelId",
        "AgentRuntimeModelRef",
        "AgentRuntimeMediaCapability",
        "AgentRuntimeMediaCapabilities",
        "AgentRuntimeStrategyHealthStatus",
        "AgentRuntimeStrategyHealth",
        "AgentRuntimeStrategyInfo",
        "AuthProfileId",
        "AuthProfileConnectionState",
        "AuthProfileLoginMethod",
        "AuthProfileRef",
        "AuthProfilePreferences",
        "AuthProfileUsage",
        "AuthProfileUsageWindow",
        "AuthProfileState",
        "AuthProfileLoginChallenge",
        "AuthProfileLoginResult",
        "AuthProfileLogoutResult",
        "RuntimeExtensionId",
        "RuntimeExtensionDescriptor",
        "RuntimeExtensionAvailability",
        "RuntimeExtensionMcpServer",
        "RuntimeExtensionMcpStdioServer",
        "RuntimeExtensionMcpHttpServer",
        "RuntimeExtensionEnvVar",
        "RuntimeExtensionHttpHeader",
        "RuntimeExtensionState",
        "RuntimeProfileId",
        "RuntimePolicyMode",
        "RuntimeProfileExecutionKind",
        "RuntimeProfileSummary",
        "RuntimeProfilePatch",
        "AgentRuntimeSelection",
        "AgentRuntimeSnapshot",
        "GetAgentRuntimeQuery",
        "DaemonAgentRuntimePatchProfileParams",
        "DaemonAgentRuntimeAuthLoginParams",
        "DaemonAgentRuntimeAuthLoginCompleteParams",
        "DaemonAgentRuntimeAuthLogoutParams",
        "DaemonAgentRuntimeSetExtensionEnabledParams",
    ] {
        lines.push(format!("export type {{ {name} }} from \"./{name}.js\";"));
    }
}

fn append_core_generated_consts(lines: &mut Vec<String>) {
    lines.extend(
        PROTOCOL_TS_CONSTS
            .iter()
            .map(|(name, value)| format!("export const {name} = {value:?};")),
    );
}

fn append_agent_runtime_generated_consts(lines: &mut Vec<String>) {
    for (name, value) in [
        (
            "METHOD_DAEMON_AGENT_RUNTIME_GET",
            METHOD_DAEMON_AGENT_RUNTIME_GET,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH",
            METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN",
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE",
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN_COMPLETE,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT",
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET",
            METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
        ),
    ] {
        lines.push(format!("export const {name} = {value:?};"));
    }
}

fn render_runtime_schema_lines(
    schemas: &[(&'static str, serde_json::Value)],
    typed: bool,
) -> Result<String, ProtocolExportError> {
    let mut lines = vec!["export const PROTOCOL_JSON_SCHEMAS = {".to_string()];
    for (name, schema) in schemas {
        lines.push(format!(
            "  {name}: {},",
            serde_json::to_string_pretty(schema)?
        ));
    }
    lines.push(if typed {
        "} as const;".to_string()
    } else {
        "};".to_string()
    });
    lines.push(String::new());
    Ok(lines.join("\n"))
}

pub(super) fn rewrite_generated_typescript_imports(
    generated_dir: &Path,
) -> Result<(), ProtocolExportError> {
    for entry in fs::read_dir(generated_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !entry.file_type()?.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("ts")
        {
            continue;
        }

        let source = fs::read_to_string(&path)?;
        let rewritten = source
            .lines()
            .map(|line| rewrite_typescript_import_line(line.trim_end()))
            .collect::<Vec<_>>()
            .join("\n");

        if rewritten != source {
            fs::write(path, format!("{rewritten}\n"))?;
        }
    }

    Ok(())
}

fn rewrite_typescript_import_line(line: &str) -> String {
    const IMPORT_PREFIX: &str = " from \"./";
    const IMPORT_SUFFIX: &str = "\";";

    if !line.contains(IMPORT_PREFIX) || !line.ends_with(IMPORT_SUFFIX) || line.contains(".js\";") {
        return line.to_string();
    }

    format!("{}.js\";", &line[..line.len() - IMPORT_SUFFIX.len()])
}

#[cfg(test)]
mod tests {
    use super::{append_core_generated_consts, append_core_generated_exports};
    #[test]
    fn export_protocol_artifacts_include_complete_scheduled_work_public_barrel() {
        let scheduled_work_types = [
            "ScheduledWorkId",
            "ScheduledWorkOccurrenceId",
            "ScheduledWorkExecutionRequest",
            "ScheduledWorkUnpublishedResource",
            "ScheduledWorkAttentionPolicy",
            "ScheduledWorkDefinition",
            "ScheduledWorkOccurrenceState",
            "ScheduledWorkPreparationTerminal",
            "ScheduledWorkOccurrence",
            "CreateScheduledWorkRequest",
            "CreateScheduledWorkResult",
            "ListScheduledWorkRequest",
            "ListScheduledWorkResult",
            "CancelScheduledWorkRequest",
        ];
        let scheduled_work_methods = [
            "METHOD_DAEMON_SCHEDULED_WORK_CREATE",
            "METHOD_DAEMON_SCHEDULED_WORK_LIST",
            "METHOD_DAEMON_SCHEDULED_WORK_CANCEL",
        ];

        let mut type_barrel = Vec::new();
        append_core_generated_exports(&mut type_barrel);
        let type_barrel = type_barrel.join("\n");
        for name in scheduled_work_types {
            assert!(
                type_barrel.contains(&format!("export type {{ {name} }} from \"./{name}.js\";")),
                "generated index.ts must export {name}"
            );
        }

        let mut runtime_barrel = Vec::new();
        append_core_generated_consts(&mut runtime_barrel);
        let runtime_barrel = runtime_barrel.join("\n");
        for name in scheduled_work_methods {
            assert!(
                runtime_barrel.contains(&format!("export const {name} = ")),
                "generated index.js must export {name}"
            );
        }
    }
}
