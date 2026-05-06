use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentStreamEvent, ApprovalRequest, ApprovalResolution, ArtifactSummary, BudgetEvent,
    CapsuleResult, ConflictWarning, ContextReceipt, OutputContractKind, PublicApprovalResolution,
    ReceiptId, ReceiptKind, ReceiptProvenance, ReceiptState, RunId, RunStatus, SessionId,
    SessionStatus, u64_string,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonEventKind {
    Session,
    Run,
    RunReconciledOnStartup,
    Approval,
    Artifact,
    ContextReceipt,
    AgentStream,
    TokenUsageRecorded,
    Conflict,
    Budget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum DaemonEvent {
    Session(SessionEvent),
    Run(RunEvent),
    RunReconciledOnStartup(RunReconciledOnStartupEvent),
    Approval(ApprovalEvent),
    Artifact(ArtifactEvent),
    ContextReceipt(ContextReceiptEvent),
    AgentStream(AgentStreamEvent),
    TokenUsageRecorded(TokenUsageRecordedEvent),
    Conflict(ConflictEvent),
    Budget(BudgetEvent),
}

impl DaemonEvent {
    pub fn redact_for_public(self) -> PublicDaemonEvent {
        self.into()
    }

    pub fn kind(&self) -> DaemonEventKind {
        match self {
            Self::Session(_) => DaemonEventKind::Session,
            Self::Run(_) => DaemonEventKind::Run,
            Self::RunReconciledOnStartup(_) => DaemonEventKind::RunReconciledOnStartup,
            Self::Approval(_) => DaemonEventKind::Approval,
            Self::Artifact(_) => DaemonEventKind::Artifact,
            Self::ContextReceipt(_) => DaemonEventKind::ContextReceipt,
            Self::AgentStream(_) => DaemonEventKind::AgentStream,
            Self::TokenUsageRecorded(_) => DaemonEventKind::TokenUsageRecorded,
            Self::Conflict(_) => DaemonEventKind::Conflict,
            Self::Budget(_) => DaemonEventKind::Budget,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub enum PublicDaemonEvent {
    Session(SessionEvent),
    Run(RunEvent),
    RunReconciledOnStartup(RunReconciledOnStartupEvent),
    Approval(PublicApprovalEvent),
    Artifact(ArtifactEvent),
    ContextReceipt(PublicContextReceiptEvent),
    AgentStream(AgentStreamEvent),
    TokenUsageRecorded(TokenUsageRecordedEvent),
    Conflict(ConflictEvent),
    Budget(BudgetEvent),
}

impl From<DaemonEvent> for PublicDaemonEvent {
    fn from(value: DaemonEvent) -> Self {
        match value {
            DaemonEvent::Session(event) => Self::Session(event),
            DaemonEvent::Run(event) => Self::Run(event),
            DaemonEvent::RunReconciledOnStartup(event) => Self::RunReconciledOnStartup(event),
            DaemonEvent::Approval(ApprovalEvent::Requested { request }) => {
                Self::Approval(PublicApprovalEvent::Requested { request })
            }
            DaemonEvent::Approval(ApprovalEvent::Resolved { resolution }) => {
                Self::Approval(PublicApprovalEvent::Resolved {
                    resolution: resolution.redact_for_public(),
                })
            }
            DaemonEvent::Artifact(event) => Self::Artifact(event),
            DaemonEvent::ContextReceipt(event) => {
                Self::ContextReceipt(PublicContextReceiptEvent::from(event))
            }
            DaemonEvent::AgentStream(event) => Self::AgentStream(event),
            DaemonEvent::TokenUsageRecorded(event) => Self::TokenUsageRecorded(event),
            DaemonEvent::Conflict(event) => Self::Conflict(event),
            DaemonEvent::Budget(event) => Self::Budget(event),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SessionEvent {
    pub session_id: SessionId,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunEvent {
    pub run_id: RunId,
    pub status: RunStatus,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<CapsuleResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunFailureKind {
    DaemonRestartedWhileRunning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunReconciledOnStartupEvent {
    pub run_id: RunId,
    pub prev_status: RunStatus,
    pub reason: RunFailureKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct TokenUsageRecordedEvent {
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_id: Option<RunId>,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub prompt_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub completion_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub cached_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub reasoning_tokens: Option<u64>,
    pub model: String,
    pub provider: String,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub recorded_at_ms: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct TokenUsageTotals {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub prompt_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub completion_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub cached_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub reasoning_tokens: u64,
}

impl TokenUsageTotals {
    pub fn is_zero(&self) -> bool {
        self.prompt_tokens == 0
            && self.completion_tokens == 0
            && self.cached_tokens == 0
            && self.reasoning_tokens == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalEvent {
    Requested { request: ApprovalRequest },
    Resolved { resolution: ApprovalResolution },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "phase", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub enum PublicApprovalEvent {
    Requested {
        request: ApprovalRequest,
    },
    Resolved {
        resolution: PublicApprovalResolution,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ArtifactEvent {
    pub artifact: ArtifactSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ConflictEvent {
    Warning {
        run_id: RunId,
        warning: ConflictWarning,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ContextReceiptEvent {
    Created { receipt: ContextReceipt },
    Promoted { receipt: ContextReceipt },
    Quarantined { receipt: ContextReceipt },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PublicContextReceipt {
    pub id: ReceiptId,
    pub kind: ReceiptKind,
    pub state: ReceiptState,
    pub provenance: ReceiptProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl From<ContextReceipt> for PublicContextReceipt {
    fn from(value: ContextReceipt) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            state: value.state,
            provenance: value.provenance,
            summary: value.summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "phase", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub enum PublicContextReceiptEvent {
    Created { receipt: PublicContextReceipt },
    Promoted { receipt: PublicContextReceipt },
    Quarantined { receipt: PublicContextReceipt },
}

impl From<ContextReceiptEvent> for PublicContextReceiptEvent {
    fn from(value: ContextReceiptEvent) -> Self {
        match value {
            ContextReceiptEvent::Created { receipt } => Self::Created {
                receipt: receipt.into(),
            },
            ContextReceiptEvent::Promoted { receipt } => Self::Promoted {
                receipt: receipt.into(),
            },
            ContextReceiptEvent::Quarantined { receipt } => Self::Quarantined {
                receipt: receipt.into(),
            },
        }
    }
}

/// Resume cursor for `daemon.subscribe` and the `latestCursor` returned from
/// `daemon.session.open` / `daemon.session.attach`.
///
/// This cursor is daemon-epoch-aware and scoped to one attached session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonEventCursor {
    pub daemon_instance_id: String,
    pub session_id: SessionId,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonEventEnvelope {
    pub daemon_instance_id: String,
    pub session_id: SessionId,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub sequence: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub occurred_at_ms: u64,
    pub event: DaemonEvent,
}

impl DaemonEventEnvelope {
    pub fn redact_for_public(self) -> PublicDaemonEventEnvelope {
        self.into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PublicDaemonEventEnvelope {
    pub daemon_instance_id: String,
    pub session_id: SessionId,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub sequence: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub occurred_at_ms: u64,
    pub event: PublicDaemonEvent,
}

impl From<DaemonEventEnvelope> for PublicDaemonEventEnvelope {
    fn from(value: DaemonEventEnvelope) -> Self {
        Self {
            daemon_instance_id: value.daemon_instance_id,
            session_id: value.session_id,
            sequence: value.sequence,
            occurred_at_ms: value.occurred_at_ms,
            event: value.event.redact_for_public(),
        }
    }
}
