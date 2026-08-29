use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentStreamEvent, ApprovalRequest, ApprovalResolution, ArtifactSummary, AuthProfileExhaustion,
    BudgetEvent, CapsuleResult, ConflictWarning, ContextReceipt, DomainError, OutputContractKind,
    PublicApprovalResolution, ReceiptId, ReceiptKind, ReceiptProvenance, ReceiptState, RunId,
    RunStatus, SessionId, SessionStatus, u64_string,
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
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunEvent {
    Status(RunStatusEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunStatusEvent {
    run_id: RunId,
    status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<RunStatusReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recipe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<CapsuleResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth_profile_exhaustion: Option<AuthProfileExhaustion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(transparent)]
#[ts(export_to = "generated/")]
pub struct RunStatusReason(String);

impl RunStatusReason {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyRunStatusReason);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RunStatusEvent {
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn status(&self) -> RunStatus {
        self.status
    }

    pub fn reason(&self) -> Option<&RunStatusReason> {
        self.reason.as_ref()
    }

    pub fn output_contract(&self) -> Option<&OutputContractKind> {
        self.output_contract.as_ref()
    }

    pub fn recipe_id(&self) -> Option<&str> {
        self.recipe_id.as_deref()
    }

    pub fn result(&self) -> Option<&CapsuleResult> {
        self.result.as_ref()
    }

    pub fn auth_profile_exhaustion(&self) -> Option<AuthProfileExhaustion> {
        self.auth_profile_exhaustion
    }

    fn new(
        run_id: RunId,
        status: RunStatus,
        reason: Option<RunStatusReason>,
        output_contract: Option<OutputContractKind>,
        recipe_id: Option<String>,
        result: Option<CapsuleResult>,
        auth_profile_exhaustion: Option<AuthProfileExhaustion>,
    ) -> Result<Self, DomainError> {
        if status.is_active() && reason.is_some() {
            return Err(DomainError::ActiveRunStatusHasReason);
        }
        if status.is_terminal() && reason.is_none() {
            return Err(DomainError::TerminalRunStatusMissingReason);
        }
        if auth_profile_exhaustion.is_some() && status != RunStatus::Failed {
            return Err(DomainError::RunStatusMustBeTerminal);
        }
        Ok(Self {
            run_id,
            status,
            reason,
            output_contract,
            recipe_id,
            result,
            auth_profile_exhaustion,
        })
    }
}

impl<'de> Deserialize<'de> for RunStatusEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            run_id: RunId,
            status: RunStatus,
            #[serde(default)]
            reason: Option<RunStatusReason>,
            #[serde(default)]
            output_contract: Option<OutputContractKind>,
            #[serde(default)]
            recipe_id: Option<String>,
            #[serde(default)]
            result: Option<CapsuleResult>,
            auth_profile_exhaustion: Option<AuthProfileExhaustion>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(
            raw.run_id,
            raw.status,
            raw.reason,
            raw.output_contract,
            raw.recipe_id,
            raw.result,
            raw.auth_profile_exhaustion,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for RunStatusReason {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl RunEvent {
    pub fn run_id(&self) -> &RunId {
        match self {
            Self::Status(event) => &event.run_id,
        }
    }

    pub fn active(
        run_id: RunId,
        status: RunStatus,
        output_contract: Option<OutputContractKind>,
        recipe_id: Option<String>,
        result: Option<CapsuleResult>,
    ) -> Result<Self, DomainError> {
        if !status.is_active() {
            return Err(DomainError::RunStatusMustBeActive);
        }
        Ok(Self::Status(RunStatusEvent::new(
            run_id,
            status,
            None,
            output_contract,
            recipe_id,
            result,
            None,
        )?))
    }

    pub fn terminal(
        run_id: RunId,
        status: RunStatus,
        reason: RunStatusReason,
        output_contract: Option<OutputContractKind>,
        recipe_id: Option<String>,
        result: Option<CapsuleResult>,
    ) -> Result<Self, DomainError> {
        if !status.is_terminal() {
            return Err(DomainError::RunStatusMustBeTerminal);
        }
        Ok(Self::Status(RunStatusEvent::new(
            run_id,
            status,
            Some(reason),
            output_contract,
            recipe_id,
            result,
            None,
        )?))
    }

    pub fn terminal_with_auth_profile_exhaustion(
        run_id: RunId,
        reason: RunStatusReason,
        exhaustion: AuthProfileExhaustion,
    ) -> Result<Self, DomainError> {
        Ok(Self::Status(RunStatusEvent::new(
            run_id,
            RunStatus::Failed,
            Some(reason),
            None,
            None,
            None,
            Some(exhaustion),
        )?))
    }
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
    #[ts(type = "string")]
    pub prompt_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub completion_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub cached_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub reasoning_tokens: Option<u64>,
    pub model: String,
    pub provider: String,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub recorded_at_ms: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct TokenUsageTotals {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub prompt_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub completion_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub cached_tokens: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
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
    #[ts(type = "string")]
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
    #[ts(type = "string")]
    pub sequence: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
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
    #[ts(type = "string")]
    pub sequence: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
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
