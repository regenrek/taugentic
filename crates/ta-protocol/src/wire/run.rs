use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeModelId, AgentRuntimeSelection, AgentRuntimeStrategyId, AgentStreamTurnId,
    AuthProfileExhaustion, AuthProfileId, CapsuleResult, ConflictSummary, ContextReceipt,
    ExecutionContext, OutputContractKind, PublicDaemonEvent, RunId, RuntimeProfileId,
    ScheduledWorkId, ScheduledWorkOccurrenceId, SessionId, TokenUsageTotals, ValidationError,
    WorkspaceFileAttachment, WorkspaceFileAttachmentRequest, WorkspaceMode, WorktreeCleanupPolicy,
    WorktreeInfo, u64_string,
};

/// Default page size for native run-list requests.
pub const NATIVE_RUN_LIST_DEFAULT_LIMIT: u32 = 50;
/// Hard server-side cap for native run-list requests.
pub const NATIVE_RUN_LIST_MAX_LIMIT: u32 = 200;
/// Cortex's intentionally small, presentation-safe lineage snapshot cap.
pub const RUN_LINEAGE_GRAPH_MAX_NODES: u32 = 128;
pub const RUN_LINEAGE_GRAPH_MAX_EDGES: u32 = 127;
pub const RUN_LINEAGE_GRAPH_MAX_BYTES: u32 = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunSource {
    /// A one-shot run claimed from a durable Scheduled Work occurrence. Its
    /// route is frozen at definition creation; it is a root run and never
    /// inherits a recipe, output contract, attachments, or parent history.
    ScheduledWork {
        route: RunExecutionRoute,
        #[serde(rename = "scheduledWorkId")]
        #[ts(rename = "scheduledWorkId")]
        scheduled_work_id: ScheduledWorkId,
        #[serde(rename = "occurrenceId")]
        #[ts(rename = "occurrenceId")]
        occurrence_id: ScheduledWorkOccurrenceId,
    },
    User {
        route: RunExecutionRoute,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "outputContract")]
        #[ts(rename = "outputContract")]
        output_contract: Option<OutputContractKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "modelId")]
        #[ts(rename = "modelId")]
        model_id: Option<AgentRuntimeModelId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "recipeId")]
        #[ts(rename = "recipeId")]
        recipe_id: Option<String>,
        attachments: Vec<WorkspaceFileAttachment>,
    },
    NativeSubagent {
        route: RunExecutionRoute,
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentTurnId")]
        #[ts(rename = "parentTurnId")]
        parent_turn_id: AgentStreamTurnId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "outputContract")]
        #[ts(rename = "outputContract")]
        output_contract: Option<OutputContractKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "modelId")]
        #[ts(rename = "modelId")]
        model_id: Option<AgentRuntimeModelId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "recipeId")]
        #[ts(rename = "recipeId")]
        recipe_id: Option<String>,
        #[serde(default)]
        #[serde(rename = "workspaceScope")]
        #[ts(rename = "workspaceScope")]
        workspace_scope: WorkspaceMode,
        #[serde(default)]
        #[serde(rename = "cleanupPolicy")]
        #[ts(rename = "cleanupPolicy")]
        cleanup_policy: WorktreeCleanupPolicy,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[serde(rename = "plannedWriteFiles")]
        #[ts(rename = "plannedWriteFiles")]
        planned_write_files: Vec<String>,
    },
    /// An independent child run that retains durable parent lineage without
    /// inheriting a parent turn, fork boundary, or native-subagent lifecycle.
    FreshSpawn {
        route: RunExecutionRoute,
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "outputContract")]
        #[ts(rename = "outputContract")]
        output_contract: Option<OutputContractKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "modelId")]
        #[ts(rename = "modelId")]
        model_id: Option<AgentRuntimeModelId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "recipeId")]
        #[ts(rename = "recipeId")]
        recipe_id: Option<String>,
        #[serde(default)]
        #[serde(rename = "workspaceScope")]
        #[ts(rename = "workspaceScope")]
        workspace_scope: WorkspaceMode,
        #[serde(default)]
        #[serde(rename = "cleanupPolicy")]
        #[ts(rename = "cleanupPolicy")]
        cleanup_policy: WorktreeCleanupPolicy,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[serde(rename = "plannedWriteFiles")]
        #[ts(rename = "plannedWriteFiles")]
        planned_write_files: Vec<String>,
    },
    Forked {
        route: RunExecutionRoute,
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentEventSeq")]
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(rename = "parentEventSeq")]
        #[ts(type = "string")]
        parent_event_seq: u64,
    },
    /// A new native continuation created only after the parent failed with a
    /// typed exhaustion fact and the user explicitly selected a validated
    /// replacement route. The parent itself is never resumed or mutated.
    RouteSwitchedContinuation {
        route: RunExecutionRoute,
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentEventSeq")]
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(rename = "parentEventSeq")]
        #[ts(type = "string")]
        parent_event_seq: u64,
    },
}

impl RunSource {
    pub fn route(&self) -> &RunExecutionRoute {
        match self {
            Self::ScheduledWork { route, .. }
            | Self::User { route, .. }
            | Self::NativeSubagent { route, .. }
            | Self::FreshSpawn { route, .. }
            | Self::Forked { route, .. }
            | Self::RouteSwitchedContinuation { route, .. } => route,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunExecutionRoute {
    pub runtime_profile_id: RuntimeProfileId,
    pub provider_id: AgentRuntimeStrategyId,
    pub harness: RunHarnessKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<AgentRuntimeModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile_id: Option<AuthProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunHarnessKind {
    Unknown,
    Native,
    Acp,
    CodexAppServer,
    RealtimeVoice,
}

impl Default for RunHarnessKind {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunStatus {
    Queued,
    Running,
    WaitingForApproval,
    Completed,
    Failed,
    BudgetExceeded,
    Cancelled,
}

impl RunStatus {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Running | Self::WaitingForApproval
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::BudgetExceeded | Self::Cancelled
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunSummary {
    pub id: RunId,
    pub runtime_profile_id: RuntimeProfileId,
    pub objective: String,
    pub status: RunStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunDetail {
    pub summary: RunSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapsuleResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_violation: Option<ValidationError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_receipt: Option<ContextReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    pub execution_context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_summary: Option<ConflictSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsageTotals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_profile_exhaustion: Option<AuthProfileExhaustion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunRecord {
    pub id: RunId,
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    pub runtime_profile_id: RuntimeProfileId,
    pub objective: String,
    pub status: RunStatus,
    pub harness: RunHarnessKind,
    pub source: RunSource,
    pub execution_context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub last_event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_summary: Option<ConflictSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
#[derive(Default)]
pub struct RunListFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<Vec<RunHarnessKind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<RunStatus>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ListNativeRunsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<RunListFilter>,
    pub limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Daemon-owned native run lineage for workspace presentation.
///
/// This is deliberately a required discriminated projection: desktop surfaces
/// render relationship semantics from the daemon rather than deriving a branch
/// kind from nullable parent or fork fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum NativeRunRelationship {
    Root,
    NativeSubagent {
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
    },
    FreshSpawn {
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
    },
    Fork {
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentEventSeq")]
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(rename = "parentEventSeq")]
        #[ts(type = "string")]
        parent_event_seq: u64,
    },
    RouteSwitchedContinuation {
        route: RunExecutionRoute,
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentEventSeq")]
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        parent_event_seq: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunListEntry {
    pub id: RunId,
    pub relationship: NativeRunRelationship,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    pub harness: RunHarnessKind,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub last_event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_summary: Option<ConflictSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ListNativeRunsResult {
    pub runs: Vec<RunListEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct RunLineageGraphRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunLineageGraphEdge {
    pub parent_run_id: RunId,
    pub child_run_id: RunId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunLineageGraphResult {
    pub nodes: Vec<RunListEntry>,
    pub edges: Vec<RunLineageGraphEdge>,
    pub orphan_run_ids: Vec<RunId>,
    pub total_count: u32,
    pub omitted_count: u32,
    pub truncated: bool,
    pub cycle_broken: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct StartRunCommand {
    pub objective: String,
    pub selection: AgentRuntimeSelection,
    pub attachments: Vec<WorkspaceFileAttachmentRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
}

impl StartRunCommand {
    pub fn new(objective: impl Into<String>, selection: AgentRuntimeSelection) -> Self {
        Self {
            objective: objective.into(),
            selection,
            attachments: Vec::new(),
            recipe_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonRunCompleteWithResultParams {
    pub run_id: RunId,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapsuleResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ResumeRunRequest {
    pub run_id: RunId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ResumeRunState {
    Live,
    Queued,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ResumeRunResult {
    pub run: RunRecord,
    pub state: ResumeRunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub latest_event_seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ForkRunRequest {
    pub session_id: SessionId,
    pub parent_run_id: RunId,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub parent_event_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ForkRunResult {
    pub run: RunRecord,
}

/// Adds one user message to an existing terminal native fork and starts that
/// same run again. This is deliberately distinct from `ResumeRunRequest`:
/// resume observes/re-attaches lifecycle state, while continuation creates the
/// next durable user turn for an already terminal branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ContinueRunRequest {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ContinueRunResult {
    pub run: RunRecord,
}

/// Creates a new native continuation from a failed, typed-exhausted parent.
/// The selected route is validated before any successor state is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SwitchRouteAndResumeRequest {
    pub session_id: SessionId,
    pub parent_run_id: RunId,
    pub selection: AgentRuntimeSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SwitchRouteAndResumeResult {
    pub run: RunRecord,
}

/// Creates an independent child with durable parent lineage and a fresh
/// conversation history. Runtime selection is explicit and provider-neutral.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SpawnRunRequest {
    pub session_id: SessionId,
    pub parent_run_id: RunId,
    pub objective: String,
    pub selection: AgentRuntimeSelection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default)]
    pub workspace_scope: WorkspaceMode,
    #[serde(default)]
    pub cleanup_policy: WorktreeCleanupPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planned_write_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SpawnRunResult {
    pub run: RunRecord,
}

/// Reads a directly-related Fresh Spawn child. The response is idempotent for
/// queued, running, and terminal children because all fields are projections of
/// existing daemon/store records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct JoinRunRequest {
    pub session_id: SessionId,
    pub parent_run_id: RunId,
    pub child_run_id: RunId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct JoinRunResult {
    pub run: RunRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapsuleResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<ContextReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<crate::wire::ArtifactSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
/// Run event subscription request after an optional durable cursor.
///
/// `daemon.run.replay_events` uses this shape for finite replay batches.
/// `daemon.run.subscribe_events` uses it for replay plus live splice streams.
pub struct SubscribeRunEventsRequest {
    pub session_id: SessionId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub after_seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
/// One run event delta returned by replay or live splice.
///
/// The sequence is the persisted daemon-event sequence, so clients can dedupe
/// replay and live deliveries with one cursor.
pub struct RunEventDelta {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub seq: u64,
    pub event: PublicDaemonEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunEventStreamError {
    Lagged,
    HistoryGap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunEventStreamPayload {
    Delta { delta: RunEventDelta },
    Error { error: RunEventStreamError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunEventStreamItem {
    pub run_id: RunId,
    pub payload: RunEventStreamPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
/// Replay-only result for durable run events.
///
/// The event list is a finite historical batch. No live stream is opened by this
/// result; live splice uses `RunEventDelta` as its stream item.
pub struct SubscribeRunEventsResult {
    pub events: Vec<RunEventDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub latest_event_seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonRunCancelParams {
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
