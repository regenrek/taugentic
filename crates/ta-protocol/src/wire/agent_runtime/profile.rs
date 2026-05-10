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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum LocalModelApiStandard {
    OpenAiChatCompletions,
    OllamaOpenAi,
    LmStudioOpenAi,
    LlamaCppOpenAi,
    VllmOpenAi,
    TgiMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum LocalModelAuthMode {
    None,
    BearerEnv,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct LocalModelEndpointCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responses_api: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct LocalModelEndpointConfig {
    pub base_url: String,
    pub api_standard: LocalModelApiStandard,
    pub auth_mode: LocalModelAuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<AgentRuntimeModelId>,
    #[serde(default)]
    pub model_discovery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<LocalModelEndpointCapabilities>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RuntimeProfileLocalEndpointPatch {
    Set { value: LocalModelEndpointConfig },
    Clear,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_endpoint: Option<LocalModelEndpointConfig>,
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
    pub local_endpoint: Option<RuntimeProfileLocalEndpointPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_mode: Option<RuntimePolicyMode>,
}
