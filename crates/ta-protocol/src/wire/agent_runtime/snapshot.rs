use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeStrategyInfo, AuthProfileState, RuntimeExtensionState, RuntimeProfileId,
    RuntimeProfileSummary,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeSelection {
    pub runtime_profile_id: RuntimeProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeSnapshot {
    pub selection: AgentRuntimeSelection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<AgentRuntimeStrategyInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_profiles: Vec<AuthProfileState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_profiles: Vec<RuntimeProfileSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_extensions: Vec<RuntimeExtensionState>,
}
