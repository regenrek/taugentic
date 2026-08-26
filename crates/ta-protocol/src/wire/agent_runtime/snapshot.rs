use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeModelId, AgentRuntimeStrategyInfo, AuthMethodRef, AuthProfileId, AuthProfileState,
    RuntimeExtensionState, RuntimeProfileId, RuntimeProfileSummary,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeSelection {
    pub runtime_profile_id: RuntimeProfileId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile_id: Option<AuthProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<AgentRuntimeModelId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentRuntimeSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<AgentRuntimeStrategyInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_methods: Vec<AuthMethodRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_profiles: Vec<AuthProfileState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_profiles: Vec<RuntimeProfileSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_extensions: Vec<RuntimeExtensionState>,
}
