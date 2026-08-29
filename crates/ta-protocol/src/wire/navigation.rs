use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{SessionId, SessionStatus, WorkspaceId, identifier};

identifier!(SpaceId, "space");
identifier!(ProjectId, "project");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NavigationSpace {
    pub id: SpaceId,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NavigationProject {
    pub id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<SpaceId>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_ids: Vec<WorkspaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum ConversationPlacement {
    Project { project_id: ProjectId },
    Standalone,
    Temporary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NavigationConversation {
    pub session_id: SessionId,
    /// Derived from the canonical session projection when the snapshot is
    /// read. Navigation persistence deliberately does not retain this value.
    pub workspace_id: WorkspaceId,
    /// Derived from the canonical session projection when the snapshot is read.
    /// Navigation persistence deliberately does not retain this value.
    pub title: String,
    /// Derived from the canonical session projection when the snapshot is read.
    /// Navigation persistence deliberately does not retain this value.
    pub status: SessionStatus,
    pub placement: ConversationPlacement,
    pub archived: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NavigationAgentRow {
    pub session_id: SessionId,
    pub title: String,
    pub active: bool,
    pub awaiting_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NavigationSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spaces: Vec<NavigationSpace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<NavigationProject>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversations: Vec<NavigationConversation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<NavigationAgentRow>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationSnapshotParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationSnapshotResult {
    pub snapshot: NavigationSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum DaemonNavigationIntent {
    CreateSpace {
        title: String,
    },
    CreateProject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        space_id: Option<SpaceId>,
        title: String,
        workspace_ids: Vec<WorkspaceId>,
    },
    SetProjectWorkspaces {
        project_id: ProjectId,
        workspace_ids: Vec<WorkspaceId>,
    },
    SetProjectSpace {
        project_id: ProjectId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        space_id: Option<SpaceId>,
    },
    PlaceConversation {
        session_id: SessionId,
        placement: ConversationPlacement,
    },
    SetPinned {
        session_id: SessionId,
        pinned: bool,
    },
    SetArchived {
        session_id: SessionId,
        archived: bool,
    },
    CloseTemporaryConversation {
        session_id: SessionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationIntentParams {
    pub intent: DaemonNavigationIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationIntentResult {
    pub snapshot: NavigationSnapshot,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationSubscribeParams {}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationSubscribeResult {}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct DaemonNavigationInvalidatedParams {}
