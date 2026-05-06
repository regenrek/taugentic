use std::{future::Future, pin::Pin};

use serde_json::{Value, json};

use super::string_field;
use crate::error::AcpClientError;

pub type AcpPermissionDecisionFuture =
    Pin<Box<dyn Future<Output = Result<AcpPermissionDecision, AcpClientError>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPermissionRequest {
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_kind: Option<String>,
    pub reason: String,
    pub options: Vec<AcpPermissionOption>,
}

impl AcpPermissionRequest {
    pub fn allow_once_option_id(&self) -> Option<&str> {
        self.options
            .iter()
            .find(|option| {
                matches!(
                    option.kind,
                    AcpPermissionOptionKind::AllowOnce | AcpPermissionOptionKind::AllowAlways
                )
            })
            .map(|option| option.id.as_str())
    }

    pub fn reject_once_option_id(&self) -> Option<&str> {
        self.options
            .iter()
            .find(|option| {
                matches!(
                    option.kind,
                    AcpPermissionOptionKind::RejectOnce | AcpPermissionOptionKind::RejectAlways
                )
            })
            .map(|option| option.id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPermissionOption {
    pub id: String,
    pub name: String,
    pub kind: AcpPermissionOptionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpPermissionDecision {
    Selected { option_id: String },
    Cancelled,
}

pub(super) fn parse_permission_request(
    params: Option<Value>,
) -> Result<AcpPermissionRequest, AcpClientError> {
    let params = params.ok_or_else(|| {
        AcpClientError::ProcessFailed("ACP permission request missing params".to_string())
    })?;
    let tool_call = params.get("toolCall").ok_or_else(|| {
        AcpClientError::ProcessFailed("ACP permission request missing toolCall".to_string())
    })?;
    let session_id = string_field(&params, "sessionId").ok_or_else(|| {
        AcpClientError::ProcessFailed("ACP permission request missing sessionId".to_string())
    })?;
    let tool_call_id = string_field(tool_call, "toolCallId")
        .or_else(|| string_field(tool_call, "id"))
        .ok_or_else(|| {
            AcpClientError::ProcessFailed("ACP permission request missing toolCallId".to_string())
        })?;
    let tool_kind = string_field(tool_call, "kind");
    let tool_name = string_field(tool_call, "title")
        .or_else(|| tool_kind.clone())
        .unwrap_or_else(|| "acp/tool".to_string());
    let reason = string_field(tool_call, "title")
        .or_else(|| string_field(tool_call, "kind"))
        .map(|value| format!("ACP permission requested for {value}"))
        .unwrap_or_else(|| "ACP permission requested".to_string());
    let options = params
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(parse_permission_option)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(AcpPermissionRequest {
        session_id,
        tool_call_id,
        tool_name,
        tool_kind,
        reason,
        options,
    })
}

fn parse_permission_option(value: &Value) -> Option<AcpPermissionOption> {
    let id = string_field(value, "optionId").or_else(|| string_field(value, "id"))?;
    let name = string_field(value, "name").unwrap_or_else(|| id.clone());
    let kind = match string_field(value, "kind").as_deref() {
        Some("allow_once") => AcpPermissionOptionKind::AllowOnce,
        Some("allow_always") => AcpPermissionOptionKind::AllowAlways,
        Some("reject_once") => AcpPermissionOptionKind::RejectOnce,
        Some("reject_always") => AcpPermissionOptionKind::RejectAlways,
        _ => AcpPermissionOptionKind::Unknown,
    };
    Some(AcpPermissionOption { id, name, kind })
}

pub(super) fn permission_decision_result(decision: AcpPermissionDecision) -> Value {
    match decision {
        AcpPermissionDecision::Selected { option_id } => {
            json!({ "outcome": "selected", "optionId": option_id })
        }
        AcpPermissionDecision::Cancelled => json!({ "outcome": "cancelled" }),
    }
}

pub(super) fn unexpected_permission(_: AcpPermissionRequest) -> AcpPermissionDecisionFuture {
    Box::pin(async { Ok(AcpPermissionDecision::Cancelled) })
}
