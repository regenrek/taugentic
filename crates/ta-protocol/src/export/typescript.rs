use std::{fs, path::Path};

use ts_rs::{Config as TsConfig, TS};

use crate::wire::*;

use super::{PROTOCOL_VERSION, ProtocolExportError, schema};

/// `(export name, value)` — shared between `generated/index.ts` and `generated/index.js`.
const PROTOCOL_TS_CONSTS: &[(&str, &str)] = &[
    ("PROTOCOL_VERSION", PROTOCOL_VERSION),
    ("DAEMON_DEFAULT_SOCKET_NAME", DAEMON_DEFAULT_SOCKET_NAME),
    ("DAEMON_SOCKET_NAME_ENV_VAR", DAEMON_SOCKET_NAME_ENV_VAR),
    ("METHOD_DAEMON_EVENT", METHOD_DAEMON_EVENT),
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
    ("METHOD_DAEMON_RUN_START", METHOD_DAEMON_RUN_START),
    ("METHOD_DAEMON_RUN_RESUME", METHOD_DAEMON_RUN_RESUME),
    ("METHOD_DAEMON_RUN_FORK", METHOD_DAEMON_RUN_FORK),
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

/// `(export name, value)` — numeric protocol constants shared between
/// `generated/index.ts` and `generated/index.js`.
const PROTOCOL_TS_NUMBER_CONSTS: &[(&str, u32)] = &[
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

fn export_core_types(cfg: &TsConfig) -> Result<(), ProtocolExportError> {
    export_ts::<AgentStreamTurnId>(cfg)?;
    export_ts::<AgentStreamItemId>(cfg)?;
    export_ts::<AgentToolCallOutcome>(cfg)?;
    export_ts::<RuntimeLanePendingState>(cfg)?;
    export_ts::<AgentStreamEvent>(cfg)?;
    export_ts::<StreamEmission>(cfg)?;
    export_ts::<AgentStreamFrame>(cfg)?;
    export_ts::<BudgetScope>(cfg)?;
    export_ts::<BudgetMetric>(cfg)?;
    export_ts::<BudgetBreach>(cfg)?;
    export_ts::<BudgetSnapshot>(cfg)?;
    export_ts::<BudgetExceededEvent>(cfg)?;
    export_ts::<BudgetEvent>(cfg)?;
    export_ts::<ArtifactId>(cfg)?;
    export_ts::<ArtifactKind>(cfg)?;
    export_ts::<ArtifactEvent>(cfg)?;
    export_ts::<ArtifactSummary>(cfg)?;
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
    export_ts::<AgentAssistantRow>(cfg)?;
    export_ts::<AgentToolCallRow>(cfg)?;
    export_ts::<AgentPendingStateRow>(cfg)?;
    export_ts::<AgentTurnRow>(cfg)?;
    export_ts::<AgentTurnsPageResult>(cfg)?;
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
    export_ts::<SessionOverviewQuery>(cfg)?;
    export_ts::<SessionOverviewResult>(cfg)?;
    export_ts::<SessionOverviewLaneStatus>(cfg)?;
    export_ts::<ApprovalAttentionState>(cfg)?;
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
    export_ts::<ta_work_source::WorkItemKey>(cfg)?;
    export_ts::<ta_work_source::WorkSourceKind>(cfg)?;
    export_ts::<ta_work_source::WorkSource>(cfg)?;
    export_ts::<ta_work_source::WorkItemStatus>(cfg)?;
    export_ts::<ta_work_source::WorkItem>(cfg)?;
    export_ts::<ta_work_source::SourceCursor>(cfg)?;
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
    export_ts::<SubscribeRunEventsRequest>(cfg)?;
    export_ts::<RunEventDelta>(cfg)?;
    export_ts::<RunEventStreamError>(cfg)?;
    export_ts::<RunEventStreamPayload>(cfg)?;
    export_ts::<RunEventStreamItem>(cfg)?;
    export_ts::<SubscribeRunEventsResult>(cfg)?;
    export_ts::<StartRunCommand>(cfg)?;
    export_ts::<DaemonRunCompleteWithResultParams>(cfg)?;
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
    export_ts::<AgentRuntimeStrategyHealthStatus>(cfg)?;
    export_ts::<AgentRuntimeStrategyHealth>(cfg)?;
    export_ts::<AgentRuntimeStrategyInfo>(cfg)?;
    export_ts::<AuthProfileId>(cfg)?;
    export_ts::<AuthProfileConnectionState>(cfg)?;
    export_ts::<AuthProfileLoginMethod>(cfg)?;
    export_ts::<AuthProfileRef>(cfg)?;
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
    export_ts::<LocalModelApiStandard>(cfg)?;
    export_ts::<LocalModelAuthMode>(cfg)?;
    export_ts::<LocalModelEndpointCapabilities>(cfg)?;
    export_ts::<LocalModelEndpointConfig>(cfg)?;
    export_ts::<RuntimeProfileSummary>(cfg)?;
    export_ts::<RuntimeProfileModelIdPatch>(cfg)?;
    export_ts::<RuntimeProfileAuthProfilePatch>(cfg)?;
    export_ts::<RuntimeProfileLocalEndpointPatch>(cfg)?;
    export_ts::<RuntimeProfilePatch>(cfg)?;
    export_ts::<AgentRuntimeSelection>(cfg)?;
    export_ts::<AgentRuntimeSnapshot>(cfg)?;
    export_ts::<GetAgentRuntimeQuery>(cfg)?;
    export_ts::<DaemonAgentRuntimeSelectProfileParams>(cfg)?;
    export_ts::<DaemonAgentRuntimePatchProfileParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeAuthLoginParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeAuthLogoutParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeSetExtensionEnabledParams>(cfg)?;
    export_ts::<DaemonAgentRuntimeTestLocalEndpointParams>(cfg)?;
    export_ts::<LocalModelEndpointTestStatus>(cfg)?;
    export_ts::<LocalModelEndpointTestResult>(cfg)?;
    Ok(())
}

fn write_generated_index(generated_dir: &Path) -> Result<(), ProtocolExportError> {
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

fn write_generated_runtime_index(generated_dir: &Path) -> Result<(), ProtocolExportError> {
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

fn write_generated_schema_runtime(generated_dir: &Path) -> Result<(), ProtocolExportError> {
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
        "AgentAssistantRow",
        "AgentToolCallRow",
        "AgentPendingStateRow",
        "AgentTurnRow",
        "AgentTurnsPageResult",
        "ArtifactId",
        "ArtifactKind",
        "ArtifactEvent",
        "ArtifactSummary",
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
        "SessionOverviewQuery",
        "SessionOverviewResult",
        "SessionOverviewLaneStatus",
        "ApprovalAttentionState",
        "DaemonInitializeParams",
        "DaemonInitializeResult",
        "DaemonPendingTransitionKind",
        "DaemonPendingTransitionView",
        "DaemonRuntimeMode",
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
}

fn append_agent_runtime_generated_exports(lines: &mut Vec<String>) {
    for name in [
        "AgentRuntimeStrategyId",
        "AgentRuntimeModelId",
        "AgentRuntimeModelRef",
        "AgentRuntimeStrategyHealthStatus",
        "AgentRuntimeStrategyHealth",
        "AgentRuntimeStrategyInfo",
        "AuthProfileId",
        "AuthProfileConnectionState",
        "AuthProfileLoginMethod",
        "AuthProfileRef",
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
        "LocalModelApiStandard",
        "LocalModelAuthMode",
        "LocalModelEndpointCapabilities",
        "LocalModelEndpointConfig",
        "RuntimeProfileSummary",
        "RuntimeProfileModelIdPatch",
        "RuntimeProfileAuthProfilePatch",
        "RuntimeProfileLocalEndpointPatch",
        "RuntimeProfilePatch",
        "AgentRuntimeSelection",
        "AgentRuntimeSnapshot",
        "GetAgentRuntimeQuery",
        "DaemonAgentRuntimeSelectProfileParams",
        "DaemonAgentRuntimePatchProfileParams",
        "DaemonAgentRuntimeAuthLoginParams",
        "DaemonAgentRuntimeAuthLogoutParams",
        "DaemonAgentRuntimeSetExtensionEnabledParams",
        "DaemonAgentRuntimeTestLocalEndpointParams",
        "LocalModelEndpointTestStatus",
        "LocalModelEndpointTestResult",
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
            "METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT",
            METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT,
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
            "METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT",
            METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET",
            METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET,
        ),
        (
            "METHOD_DAEMON_AGENT_RUNTIME_LOCAL_ENDPOINT_TEST",
            METHOD_DAEMON_AGENT_RUNTIME_LOCAL_ENDPOINT_TEST,
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

fn rewrite_generated_typescript_imports(generated_dir: &Path) -> Result<(), ProtocolExportError> {
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
            .map(rewrite_typescript_import_line)
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
