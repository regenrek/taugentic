use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentStreamItemId, ApprovalId, DaemonEventCursor, DomainError, RunId, RunSummary,
    WorkspaceMode, u64_string,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalScope {
    FileWrite,
    ProcessExec,
    NetworkAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalResolutionReason {
    User,
    Expired,
    Cancelled,
    BudgetExceeded,
    RuntimePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalTarget {
    ToolCall {
        #[serde(rename = "toolName")]
        #[ts(rename = "toolName")]
        tool_name: String,
    },
    FileWrite {
        paths: Vec<String>,
    },
    ProcessExec {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },
    NetworkAccess {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        protocol: Option<String>,
    },
    CapsuleDispatch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "childRunId")]
        #[ts(rename = "childRunId")]
        child_run_id: Option<RunId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "workspaceScope")]
        #[ts(rename = "workspaceScope")]
        workspace_scope: Option<WorkspaceMode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ApprovalActor {
    pub principal_id: String,
}

impl ApprovalActor {
    pub fn new(principal_id: impl Into<String>) -> Result<Self, DomainError> {
        let principal_id = principal_id.into();
        if principal_id.trim().is_empty() {
            return Err(DomainError::EmptyApprovalActorPrincipalId);
        }

        Ok(Self { principal_id })
    }
}

impl<'de> Deserialize<'de> for ApprovalActor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawApprovalActor {
            principal_id: String,
        }

        let raw = RawApprovalActor::deserialize(deserializer)?;
        Self::new(raw.principal_id).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ApprovalResolution {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<AgentStreamItemId>,
    pub decision: ApprovalDecision,
    pub reason: ApprovalResolutionReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<ApprovalActor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commentary: Option<String>,
}

impl ApprovalResolution {
    pub fn new(
        approval_id: ApprovalId,
        run_id: RunId,
        decision: ApprovalDecision,
        reason: ApprovalResolutionReason,
        actor: ApprovalActor,
        commentary: Option<String>,
    ) -> Self {
        Self {
            approval_id,
            run_id,
            tool_call_id: None,
            decision,
            reason,
            actor: Some(actor),
            commentary: commentary
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }

    pub fn redact_for_public(self) -> PublicApprovalResolution {
        self.into()
    }

    pub fn with_tool_call_id(mut self, tool_call_id: AgentStreamItemId) -> Self {
        self.tool_call_id = Some(tool_call_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PublicApprovalResolution {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<AgentStreamItemId>,
    pub decision: ApprovalDecision,
    pub reason: ApprovalResolutionReason,
}

impl From<ApprovalResolution> for PublicApprovalResolution {
    fn from(value: ApprovalResolution) -> Self {
        Self {
            approval_id: value.approval_id,
            run_id: value.run_id,
            tool_call_id: value.tool_call_id,
            decision: value.decision,
            reason: value.reason,
        }
    }
}

impl<'de> Deserialize<'de> for ApprovalResolution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawApprovalResolution {
            approval_id: ApprovalId,
            run_id: RunId,
            #[serde(default)]
            tool_call_id: Option<AgentStreamItemId>,
            decision: ApprovalDecision,
            reason: ApprovalResolutionReason,
            #[serde(default)]
            actor: Option<ApprovalActor>,
            #[serde(default)]
            commentary: Option<String>,
        }

        let raw = RawApprovalResolution::deserialize(deserializer)?;
        Ok(Self {
            approval_id: raw.approval_id,
            run_id: raw.run_id,
            tool_call_id: raw.tool_call_id,
            decision: raw.decision,
            reason: raw.reason,
            actor: raw.actor,
            commentary: raw
                .commentary
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<AgentStreamItemId>,
    pub scope: ApprovalScope,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub requested_at_ms: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub expires_at_ms: u64,
    pub target: ApprovalTarget,
    pub reason: String,
}

impl ApprovalRequest {
    pub fn new(
        id: ApprovalId,
        run_id: RunId,
        scope: ApprovalScope,
        requested_at_ms: u64,
        expires_at_ms: u64,
        target: ApprovalTarget,
        reason: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(DomainError::EmptyApprovalReason);
        }
        if expires_at_ms <= requested_at_ms {
            return Err(DomainError::InvalidApprovalTtl);
        }

        Ok(Self {
            id,
            run_id,
            tool_call_id: None,
            scope,
            requested_at_ms,
            expires_at_ms,
            target,
            reason,
        })
    }

    pub fn with_tool_call_id(mut self, tool_call_id: AgentStreamItemId) -> Self {
        self.tool_call_id = Some(tool_call_id);
        self
    }
}

impl<'de> Deserialize<'de> for ApprovalRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawApprovalRequest {
            id: ApprovalId,
            run_id: RunId,
            #[serde(default)]
            tool_call_id: Option<AgentStreamItemId>,
            scope: ApprovalScope,
            #[serde(with = "u64_string")]
            requested_at_ms: u64,
            #[serde(with = "u64_string")]
            expires_at_ms: u64,
            target: ApprovalTarget,
            reason: String,
        }

        let raw = RawApprovalRequest::deserialize(deserializer)?;
        let mut request = Self::new(
            raw.id,
            raw.run_id,
            raw.scope,
            raw.requested_at_ms,
            raw.expires_at_ms,
            raw.target,
            raw.reason,
        )
        .map_err(serde::de::Error::custom)?;
        request.tool_call_id = raw.tool_call_id;
        Ok(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ApprovalSnapshotResult {
    pub items: Vec<ApprovalRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<DaemonEventCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonApprovalDecideParams {
    pub approval_id: ApprovalId,
    pub decision: ApprovalDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commentary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonApprovalDecideResult {
    pub run: RunSummary,
}
