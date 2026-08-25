use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeModelId, AgentStreamTurnId, CapsuleResult, ConflictSummary, ContextReceipt,
    ExecutionContext, OutputContractKind, PublicDaemonEvent, RunId, RuntimeProfileId, SessionId,
    TokenUsageTotals, ValidationError, WorkspaceMode, WorktreeCleanupPolicy, WorktreeInfo,
    u64_string,
};

/// Default page size for native run-list requests.
pub const NATIVE_RUN_LIST_DEFAULT_LIMIT: u32 = 50;
/// Hard server-side cap for native run-list requests.
pub const NATIVE_RUN_LIST_MAX_LIMIT: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunSource {
    User {
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
    },
    NativeSubagent {
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
    Forked {
        #[serde(rename = "parentRunId")]
        #[ts(rename = "parentRunId")]
        parent_run_id: RunId,
        #[serde(rename = "parentEventSeq")]
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(rename = "parentEventSeq")]
        #[ts(as = "u64")]
        parent_event_seq: u64,
    },
}

impl Default for RunSource {
    fn default() -> Self {
        Self::User {
            output_contract: None,
            model_id: None,
            recipe_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunHarnessKind {
    Unknown,
    Native,
    Acp,
    CodexAppServer,
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
    #[ts(type = "bigint | null")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub last_event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_summary: Option<ConflictSummary>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunListEntry {
    pub id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    pub harness: RunHarnessKind,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
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
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct StartRunCommand {
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<AgentRuntimeModelId>,
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
    #[ts(type = "bigint | null")]
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
    #[ts(as = "u64")]
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
    #[ts(type = "bigint | null")]
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
    #[ts(as = "u64")]
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
    #[ts(type = "bigint | null")]
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
