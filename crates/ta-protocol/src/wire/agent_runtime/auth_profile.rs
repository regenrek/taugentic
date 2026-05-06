use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{AgentRuntimeStrategyId, identifier};

identifier!(AuthProfileId, "auth profile");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileConnectionState {
    LoggedOut,
    PendingLogin,
    Connected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileLoginMethod {
    Browser,
    DeviceCode,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileRef {
    pub id: AuthProfileId,
    pub provider_id: AgentRuntimeStrategyId,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileManagementMode {
    Interactive,
    NativeAcpAuth,
    TerminalCliDelegated,
    Environment,
    None,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileActionHint {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileMethodInfo {
    pub id: String,
    pub display_name: String,
    pub management_mode: AuthProfileManagementMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileState {
    pub profile: AuthProfileRef,
    pub connection_state: AuthProfileConnectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub management_mode: AuthProfileManagementMode,
    pub can_login: bool,
    pub can_logout: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_org_linked: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup_steps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<AuthProfileActionHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<AuthProfileMethodInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileLoginChallenge {
    pub auth_profile_id: AuthProfileId,
    pub method: AuthProfileLoginMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_browser_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorize_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileLoginResult {
    pub auth_profile: AuthProfileState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge: Option<AuthProfileLoginChallenge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileLogoutResult {
    pub auth_profile_id: AuthProfileId,
    pub disconnected: bool,
}
