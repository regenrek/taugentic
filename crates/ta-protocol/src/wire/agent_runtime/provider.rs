use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{identifier, u64_string};

identifier!(AgentRuntimeStrategyId, "agent runtime provider");
identifier!(AgentRuntimeModelId, "agent runtime model");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeModelRef {
    pub id: AgentRuntimeModelId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub context_limit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub input_cost_per_million_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub output_cost_per_million_micros: Option<u64>,
    pub reasoning: bool,
    pub tool_call: bool,
    pub structured_output: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_modalities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AgentRuntimeStrategyHealthStatus {
    Unknown,
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeStrategyHealth {
    pub status: AgentRuntimeStrategyHealthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AgentRuntimeModelAvailability {
    Enumerated,
    CurrentOnly,
    Unsupported,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeModelCapability {
    pub availability: AgentRuntimeModelAvailability,
    pub can_set_model: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_model_id: Option<AgentRuntimeModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeStrategyInfo {
    pub id: AgentRuntimeStrategyId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<AgentRuntimeModelRef>,
    pub model_capability: AgentRuntimeModelCapability,
    pub health: AgentRuntimeStrategyHealth,
}
