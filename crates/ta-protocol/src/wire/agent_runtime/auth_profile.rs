use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{AgentRuntimeStrategyId, identifier};

identifier!(AuthMethodId, "auth method");
identifier!(AuthProfileId, "auth profile");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileConnectionState {
    Loading,
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
    pub auth_method_id: AuthMethodId,
    pub provider_id: AgentRuntimeStrategyId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_tier: Option<String>,
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
pub struct AuthMethodRef {
    pub id: AuthMethodId,
    pub provider_id: AgentRuntimeStrategyId,
    pub display_name: String,
    pub management_mode: AuthProfileManagementMode,
    pub supports_multiple_profiles: bool,
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

/// Daemon-owned presentation preferences for one concrete external account.
/// The credential backend never receives these fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfilePreferences {
    pub label: String,
    pub order: u32,
    pub is_default: bool,
}

/// Sanitized account-availability fact recorded when a selected provider
/// account cannot continue a run. It deliberately excludes provider payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileExhaustion {
    RateLimited,
    CreditsExhausted,
}

/// Account-scoped provider usage. Providers that do not expose a supported
/// account-usage contract are explicitly unavailable rather than reported as
/// zero or inferred from local run history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AuthProfileUsage {
    Unavailable,
    Observed {
        #[serde(rename = "observedAtMs")]
        #[ts(rename = "observedAtMs")]
        observed_at_ms: String,
        windows: Vec<AuthProfileUsageWindow>,
    },
}

/// A usage window reported directly by the authenticated provider account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileUsageWindow {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining: Option<String>,
    #[serde(
        rename = "resetsAtMs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(rename = "resetsAtMs")]
    pub resets_at_ms: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AuthProfileState {
    pub profile: AuthProfileRef,
    pub preferences: AuthProfilePreferences,
    pub usage: AuthProfileUsage,
    pub connection_state: AuthProfileConnectionState,
    pub exhaustion: Option<AuthProfileExhaustion>,
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
