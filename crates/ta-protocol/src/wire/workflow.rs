use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{AgentRuntimeModelId, AgentRuntimeStrategyId, CodeHostAccountId, u64_string};

pub const WORKFLOW_KIND_V1: &str = "taugentic.workflow/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowDefinition {
    pub kind: String,
    pub name: String,
    pub source: WorkflowSourceBinding,
    pub orchestrator: WorkflowOrchestratorPolicy,
    pub policy: WorkflowPolicy,
    pub runtime_profiles: BTreeMap<String, WorkflowRuntimeProfileRef>,
    pub outputs: WorkflowOutputsPolicy,
    pub budgets: WorkflowBudgets,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowSourceBinding {
    pub kind: WorkflowSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_host_account_id: Option<CodeHostAccountId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    pub active_states: Vec<String>,
    pub terminal_states: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "generated/")]
pub enum WorkflowSourceKind {
    Linear,
    GithubIssues,
    GithubPrReviews,
    LocalTasks,
    MissionBoard,
    Cli,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowOrchestratorPolicy {
    pub max_concurrent_missions: u32,
    pub max_capsules_per_mission: u32,
    pub retry: WorkflowRetryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowRetryPolicy {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub initial_ms: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowPolicy {
    pub approvals: WorkflowApprovalPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowApprovalPolicy {
    pub file_write: WorkflowFileWriteApproval,
    pub process: WorkflowProcessApproval,
    pub network: WorkflowNetworkApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "generated/")]
pub enum WorkflowFileWriteApproval {
    Ask,
    Auto,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "generated/")]
pub enum WorkflowProcessApproval {
    Ask,
    AskForSensitive,
    Auto,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "generated/")]
pub enum WorkflowNetworkApproval {
    Allowlist,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowRuntimeProfileRef {
    pub provider: AgentRuntimeStrategyId,
    pub model: AgentRuntimeModelId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowOutputsPolicy {
    pub required: Vec<WorkflowOutputRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "generated/")]
pub enum WorkflowOutputRequirement {
    Evidence,
    Tests,
    PatchOrBlocker,
    RiskSummary,
    Plan,
    ReviewFindings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowBudgets {
    pub per_capsule: WorkflowBudgetLimits,
    pub per_orchestrator: WorkflowBudgetLimits,
    pub per_workflow: WorkflowBudgetLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowBudgetLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub max_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "optional_f64_json_schema")]
    pub max_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub max_wall_time_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowLoadParams {
    pub path: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
#[ts(export_to = "generated/")]
pub struct WorkflowReloadParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkflowValidateParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkflowValidationReport {
    pub valid: bool,
    pub errors: Vec<WorkflowValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkflowValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkflowStatusResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded: Option<WorkflowLoadedStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reload: Option<WorkflowReloadOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkflowLoadedStatus {
    pub name: String,
    pub path: String,
    pub source_kind: WorkflowSourceKind,
    pub runtime_profile_count: u32,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkflowReloadOutcome {
    Reloaded {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prev_name: Option<String>,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        version: u64,
    },
    Failed {
        errors: Vec<WorkflowValidationError>,
    },
}

fn optional_f64_json_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [
            { "type": "number" },
            { "type": "null" }
        ]
    })
}
