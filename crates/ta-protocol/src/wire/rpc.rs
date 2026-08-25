use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeModelId, AuthProfileId, CapsuleRecipe, DaemonEventCursor, DaemonEventKind,
    DaemonRuntimeMode, OutputContractKind, RuntimeExtensionId, RuntimeProfileId,
    RuntimeProfilePatch, WorkspaceMode, WorktreeCleanupPolicy,
};

pub const DAEMON_DEFAULT_SOCKET_NAME: &str = "ta-daemon";
pub const DAEMON_SOCKET_NAME_ENV_VAR: &str = "TAUGENTIC_DAEMON_SOCKET_NAME";
pub const DAEMON_PROTOCOL_VERSION: &str = "2026-04-stage3";
pub const METHOD_DAEMON_INITIALIZE: &str = "daemon.initialize";
pub const METHOD_DAEMON_SUBSCRIBE: &str = "daemon.subscribe";
pub const METHOD_DAEMON_EVENT: &str = "daemon.event";
pub const METHOD_DAEMON_STATUS: &str = "daemon.status";
pub const METHOD_DAEMON_CONTROL_STATUS: &str = "daemon.control.status";
pub const METHOD_DAEMON_DIAGNOSTICS_SNAPSHOT: &str = "daemon.diagnostics.snapshot";
pub const METHOD_DAEMON_SESSION_OPEN: &str = "daemon.session.open";
pub const METHOD_DAEMON_SESSION_LIST: &str = "daemon.session.list";
pub const METHOD_DAEMON_SESSION_GET: &str = "daemon.session.get";
pub const METHOD_DAEMON_SESSION_OVERVIEW: &str = "daemon.session.overview";
pub const METHOD_DAEMON_SESSION_ATTACH: &str = "daemon.session.attach";
pub const METHOD_DAEMON_WORKSPACE_OPEN: &str = "daemon.workspace.open";
pub const METHOD_DAEMON_WORKSPACE_LIST: &str = "daemon.workspace.list";
pub const METHOD_DAEMON_WORKSPACE_GET: &str = "daemon.workspace.get";
pub const METHOD_DAEMON_ACTIVITY_PAGE: &str = "daemon.activity.page";
pub const METHOD_DAEMON_AGENT_TURNS_PAGE: &str = "daemon.agent.turns.page";
pub const METHOD_DAEMON_APPROVAL_LIST: &str = "daemon.approval.list";
pub const METHOD_DAEMON_APPROVAL_DECIDE: &str = "daemon.approval.decide";
pub const METHOD_DAEMON_WORK_ITEM_LIST: &str = "daemon.work_item.list";
pub const METHOD_DAEMON_WORK_ITEM_REFRESH: &str = "daemon.work_item.refresh";
pub const METHOD_DAEMON_WORK_ITEM_DISMISS: &str = "daemon.work_item.dismiss";
pub const METHOD_DAEMON_WORK_ITEM_TRIGGER: &str = "daemon.work_item.trigger";
pub const METHOD_DAEMON_ARTIFACT_GET: &str = "daemon.artifact.get";
pub const METHOD_DAEMON_ARTIFACT_LIST: &str = "daemon.artifact.list";
pub const METHOD_DAEMON_CONTEXT_RECEIPTS_LIST: &str = "daemon.context.receipts.list";
pub const METHOD_DAEMON_CONTEXT_RECEIPTS_PROMOTE: &str = "daemon.context.receipts.promote";
pub const METHOD_DAEMON_CONTEXT_RECEIPTS_QUARANTINE: &str = "daemon.context.receipts.quarantine";
pub const METHOD_DAEMON_RUN_START: &str = "daemon.run.start";
pub const METHOD_DAEMON_RUN_COMPLETE_WITH_RESULT: &str = "daemon.run.complete_with_result";
pub const METHOD_DAEMON_RUN_RESUME: &str = "daemon.run.resume";
pub const METHOD_DAEMON_RUN_FORK: &str = "daemon.run.fork";
/// Replays already-durable run events only.
///
/// This method does not open a live stream.
pub const METHOD_DAEMON_RUN_REPLAY_EVENTS: &str = "daemon.run.replay_events";
/// Replays durable run events, then streams live run events without cursor gaps.
pub const METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS: &str = "daemon.run.subscribe_events";
pub const METHOD_DAEMON_RUN_EVENT: &str = "daemon.run.event";
pub const METHOD_DAEMON_RUN_CANCEL: &str = "daemon.run.cancel";
pub const METHOD_DAEMON_RUN_LIST: &str = "daemon.run.list";
pub const METHOD_DAEMON_RUN_LIST_NATIVE: &str = "daemon.run.list_native";
pub const METHOD_DAEMON_RUN_GET: &str = "daemon.run.get";
pub const METHOD_DAEMON_RUN_TIMELINE: &str = "daemon.run.timeline";
pub const METHOD_DAEMON_RECIPES_LIST: &str = "daemon.recipes.list";
pub const METHOD_DAEMON_AGENT_RUNTIME_GET: &str = "daemon.agent.runtime.get";
pub const METHOD_DAEMON_AGENT_RUNTIME_PROFILE_SELECT: &str = "daemon.agent.runtime.profile.select";
pub const METHOD_DAEMON_AGENT_RUNTIME_PROFILE_PATCH: &str = "daemon.agent.runtime.profile.patch";
pub const METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGIN: &str = "daemon.agent.runtime.auth.login";
pub const METHOD_DAEMON_AGENT_RUNTIME_AUTH_LOGOUT: &str = "daemon.agent.runtime.auth.logout";
pub const METHOD_DAEMON_AGENT_RUNTIME_EXTENSION_SET: &str = "daemon.agent.runtime.extension.set";
pub const METHOD_WORKFLOW_LOAD: &str = "workflow.load";
pub const METHOD_WORKFLOW_STATUS: &str = "workflow.status";
pub const METHOD_WORKFLOW_RELOAD: &str = "workflow.reload";
pub const METHOD_WORKFLOW_VALIDATE: &str = "workflow.validate";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct DelegateRequest {
    pub objective: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<AgentRuntimeModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default)]
    pub workspace_scope: WorkspaceMode,
    #[serde(default)]
    pub cleanup_policy: WorktreeCleanupPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planned_write_files: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct ListRecipesParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RecipeListResponse {
    pub recipes: Vec<CapsuleRecipe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonClientCapabilities {
    pub notifications: bool,
    pub event_subscriptions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonServerCapabilities {
    pub notifications: bool,
    pub event_subscriptions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonInitializeParams {
    pub client_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_credential: Option<String>,
    pub client_version: String,
    pub protocol_version: String,
    pub capabilities: DaemonClientCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonInitializeResult {
    pub daemon_instance_id: String,
    pub daemon_version: String,
    pub client_credential: String,
    pub protocol_version: String,
    pub capabilities: DaemonServerCapabilities,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct DaemonStatusParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonStatusResult {
    pub ready: bool,
    pub daemon_instance_id: String,
    pub runtime_mode: DaemonRuntimeMode,
    pub socket_path: String,
    pub log_path: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonStopResult {
    pub stopping: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonSubscribeParams {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<DaemonEventKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_cursor: Option<DaemonEventCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonSubscribeResult {
    Ready {
        #[serde(rename = "latestCursor")]
        #[ts(rename = "latestCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        latest_cursor: Option<DaemonEventCursor>,
    },
    HistoryGap {
        #[serde(rename = "latestCursor")]
        #[ts(rename = "latestCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        latest_cursor: Option<DaemonEventCursor>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonAgentRuntimeSelectProfileParams {
    pub runtime_profile_id: RuntimeProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonAgentRuntimePatchProfileParams {
    pub runtime_profile_id: RuntimeProfileId,
    pub patch: RuntimeProfilePatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonAgentRuntimeAuthLoginParams {
    pub auth_profile_id: AuthProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonAgentRuntimeAuthLogoutParams {
    pub auth_profile_id: AuthProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonAgentRuntimeSetExtensionEnabledParams {
    pub extension_id: RuntimeExtensionId,
    pub enabled: bool,
}
