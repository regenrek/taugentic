use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::PluginId;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum PluginCapability {
    WorkspaceRead,
    WorkspaceWrite,
    ProcessExecute,
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum PluginLifecycleState {
    Disabled,
    Activating,
    Active,
    Failed,
}

/// Stable public vocabulary for the future out-of-process host. Installation
/// creates only `Disabled`; this foundation never launches package code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum PluginLifecycleFailure {
    StaleActivation,
    BootInterrupted,
    HostLaunch,
    HostExit,
    HostProtocol,
    HostDisposal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PluginInspection {
    pub plugin_id: PluginId,
    pub version: String,
    pub digest_sha256: String,
    pub requested_capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PluginInstallation {
    pub plugin_id: PluginId,
    pub version: String,
    pub digest_sha256: String,
    pub requested_capabilities: Vec<PluginCapability>,
    pub granted_capabilities: Vec<PluginCapability>,
    pub lifecycle_state: PluginLifecycleState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct InspectPluginPackageRequest {
    /// Transient local directory input. It is never persisted or projected.
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct InstallPluginPackageRequest {
    /// Transient local directory input. It is never persisted or projected.
    pub source_path: String,
    pub inspection: PluginInspection,
    pub granted_capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct InstallPluginPackageResult {
    pub installation: PluginInstallation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ListPluginInstallationsRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ListPluginInstallationsResult {
    pub installations: Vec<PluginInstallation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct UninstallPluginRequest {
    pub plugin_id: PluginId,
    pub version: String,
    pub digest_sha256: String,
}
