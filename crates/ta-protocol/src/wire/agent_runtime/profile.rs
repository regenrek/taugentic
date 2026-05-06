use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{AgentRuntimeModelId, AgentRuntimeStrategyId, AuthProfileId, identifier};

identifier!(RuntimeProfileId, "runtime profile");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RuntimePolicyMode {
    RequireApproval,
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RuntimeProfileSummary {
    pub id: RuntimeProfileId,
    pub display_name: String,
    pub provider_id: AgentRuntimeStrategyId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<AgentRuntimeModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile_id: Option<AuthProfileId>,
    pub policy_mode: RuntimePolicyMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RuntimeProfileModelIdPatch {
    Set { value: AgentRuntimeModelId },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RuntimeProfileAuthProfilePatch {
    Set { value: AuthProfileId },
    Clear,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct RuntimeProfilePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<AgentRuntimeStrategyId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<RuntimeProfileModelIdPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile: Option<RuntimeProfileAuthProfilePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_mode: Option<RuntimePolicyMode>,
}
