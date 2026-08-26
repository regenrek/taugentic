use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonRuntimeMode {
    Local,
    Background,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonActualRuntimeMode {
    Stopped,
    Local,
    Background,
    Foreign,
}

/// Redacted connection state for the desktop lifecycle projection.
///
/// The desktop receives only whether it may render current snapshots. Daemon
/// identity, cursors, transport details, and recovery mechanics remain in the
/// Rust client boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DesktopDaemonLifecycleStatus {
    Ready,
    SnapshotRehydrationRequired,
    Disconnected,
}

/// Safe desktop-facing lifecycle signal.
///
/// `invalidated` tells the presentation cache to refresh daemon snapshots;
/// it intentionally carries neither event payload nor resume lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DesktopDaemonLifecycleProjection {
    pub status: DesktopDaemonLifecycleStatus,
    pub invalidated: bool,
    pub foreign_runtime_restricted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonTransitionStatus {
    Idle,
    Applying,
    DegradedReconcileRequired,
    FailedNoStateChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonControlAction {
    Start,
    Stop,
    EnableBackground,
    DisableBackground,
    Reconcile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonControlErrorCode {
    UnsupportedPlatform,
    ExternalRuntime,
    OwnershipMismatch,
    ReconcileRequired,
    TransitionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonPendingTransitionKind {
    EnableBackground,
    DisableBackground,
    RecoverToLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonPendingTransitionView {
    pub kind: DaemonPendingTransitionKind,
    pub op_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonControlStatusResult {
    pub background_opt_in: bool,
    pub desired_mode: DaemonRuntimeMode,
    pub actual_mode: DaemonActualRuntimeMode,
    pub transition_status: DaemonTransitionStatus,
    pub reconcile_required: bool,
    pub allowed_actions: Vec<DaemonControlAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<DaemonControlErrorCode>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_transition: Option<DaemonPendingTransitionView>,
    pub socket_path: String,
    /// Canonical daemon log path for this host/runtime configuration.
    pub log_path: String,
    /// Running daemon version observed via daemon status.
    /// `None` means the daemon version is currently not observable, so UIs should omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_version: Option<String>,
    pub protocol_version: String,
}
