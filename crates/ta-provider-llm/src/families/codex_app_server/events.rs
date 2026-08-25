use serde_json::Value;

use super::CodexLlmClientError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexToolCallOutcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexAppServerEvent {
    TurnStarted {
        turn_id: String,
    },
    AgentMessageDelta {
        turn_id: String,
        item_id: String,
        delta: String,
    },
    ToolCallStarted {
        turn_id: String,
        item_id: String,
        tool_name: String,
    },
    ToolCallProgressed {
        turn_id: String,
        item_id: String,
        delta: String,
    },
    ToolCallCompleted {
        turn_id: String,
        item_id: String,
        outcome: CodexToolCallOutcome,
    },
    ReasoningDelta {
        turn_id: String,
        item_id: String,
        delta: String,
    },
    TokenCount {
        turn_id: String,
        total_tokens: Option<u64>,
        model_context_window: Option<u64>,
    },
    ApprovalRequested {
        turn_id: Option<String>,
        item_id: Option<String>,
        detail: String,
    },
    TurnCompleted {
        turn_id: String,
    },
    Activity {
        message: String,
    },
}

pub fn event_from_notification(
    method: &str,
    message: &Value,
) -> Result<Option<CodexAppServerEvent>, CodexLlmClientError> {
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "turn/started" => {
            let turn = params.get("turn").cloned().unwrap_or(Value::Null);
            Ok(Some(CodexAppServerEvent::TurnStarted {
                turn_id: required_string(&turn, "id")?,
            }))
        }
        "item/agentMessage/delta" => Ok(Some(CodexAppServerEvent::AgentMessageDelta {
            turn_id: required_string(&params, "turnId")?,
            item_id: required_string(&params, "itemId")?,
            delta: required_string(&params, "delta")?,
        })),
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            Ok(Some(CodexAppServerEvent::ReasoningDelta {
                turn_id: required_string(&params, "turnId")?,
                item_id: required_string(&params, "itemId")?,
                delta: required_string(&params, "delta")?,
            }))
        }
        "item/commandExecution/outputDelta"
        | "item/fileChange/outputDelta"
        | "item/mcpToolCall/progress" => Ok(Some(CodexAppServerEvent::ToolCallProgressed {
            turn_id: required_string(&params, "turnId")?,
            item_id: required_string(&params, "itemId")?,
            delta: notification_delta(method, &params)?,
        })),
        "thread/tokenUsage/updated" => {
            let usage = params.get("tokenUsage").cloned().unwrap_or(Value::Null);
            let total = usage.get("total").cloned().unwrap_or(Value::Null);
            Ok(Some(CodexAppServerEvent::TokenCount {
                turn_id: required_string(&params, "turnId")?,
                total_tokens: total.get("totalTokens").and_then(Value::as_u64),
                model_context_window: usage.get("modelContextWindow").and_then(Value::as_u64),
            }))
        }
        "item/autoApprovalReview/started" => Ok(Some(CodexAppServerEvent::ApprovalRequested {
            turn_id: params
                .get("turnId")
                .and_then(Value::as_str)
                .map(str::to_string),
            item_id: params
                .get("targetItemId")
                .and_then(Value::as_str)
                .map(str::to_string),
            detail: "codex approval requested".to_string(),
        })),
        "item/started" => item_started_event(&params),
        "item/completed" => item_completed_event(&params),
        "turn/completed" => {
            let turn = params.get("turn").cloned().unwrap_or(Value::Null);
            if let Some(error) = turn.get("error").filter(|value| !value.is_null()) {
                return Err(codex_error_from_turn_error(error));
            }
            Ok(Some(CodexAppServerEvent::TurnCompleted {
                turn_id: required_string(&turn, "id")?,
            }))
        }
        "error" => {
            let error = params.get("error").cloned().unwrap_or(Value::Null);
            Err(codex_error_from_turn_error(&error))
        }
        "thread/started" | "thread/status/changed" | "item/reasoning/summaryPartAdded" => {
            Ok(Some(CodexAppServerEvent::Activity {
                message: method.to_string(),
            }))
        }
        other => {
            tracing::trace!(method = other, "ignored codex app-server notification");
            Ok(Some(CodexAppServerEvent::Activity {
                message: other.to_string(),
            }))
        }
    }
}

pub fn required_string(value: &Value, field: &str) -> Result<String, CodexLlmClientError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            CodexLlmClientError::Protocol(format!("codex app-server field {field} missing"))
        })
}

pub fn codex_error_from_turn_error(error: &Value) -> CodexLlmClientError {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("codex app-server error")
        .to_string();
    let info = error
        .get("codexErrorInfo")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match info {
        "contextWindowExceeded" => CodexLlmClientError::ContextLengthExceeded(message),
        "usageLimitExceeded" => CodexLlmClientError::CreditsExhausted(message),
        "serverOverloaded" => CodexLlmClientError::RateLimited {
            retry_after_ms: None,
            detail: message,
        },
        "unauthorized" => {
            CodexLlmClientError::Auth("ChatGPT session expired, run `codex login`".to_string())
        }
        "badRequest" => CodexLlmClientError::InvalidConfig(message),
        "internalServerError" => CodexLlmClientError::CommandFailed(message),
        _ if message.to_ascii_lowercase().contains("context") => {
            CodexLlmClientError::ContextLengthExceeded(message)
        }
        _ => CodexLlmClientError::CommandFailed(message),
    }
}

fn item_started_event(params: &Value) -> Result<Option<CodexAppServerEvent>, CodexLlmClientError> {
    let item = params.get("item").cloned().unwrap_or(Value::Null);
    let Some(tool_name) = tool_name_from_item(&item) else {
        return Ok(Some(CodexAppServerEvent::Activity {
            message: format!("item/started:{}", item_type(&item)),
        }));
    };
    Ok(Some(CodexAppServerEvent::ToolCallStarted {
        turn_id: required_string(params, "turnId")?,
        item_id: required_string(&item, "id")?,
        tool_name,
    }))
}

fn item_completed_event(
    params: &Value,
) -> Result<Option<CodexAppServerEvent>, CodexLlmClientError> {
    let item = params.get("item").cloned().unwrap_or(Value::Null);
    let turn_id = required_string(params, "turnId")?;
    match item.get("type").and_then(Value::as_str) {
        // Agent message text is streamed canonically through
        // item/agentMessage/delta. The completed item contains the full text
        // again and must not be projected as another delta.
        Some("agentMessage") => Ok(None),
        Some("commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall") => {
            Ok(Some(CodexAppServerEvent::ToolCallCompleted {
                turn_id,
                item_id: required_string(&item, "id")?,
                outcome: outcome_from_item(&item),
            }))
        }
        Some(kind) => Ok(Some(CodexAppServerEvent::Activity {
            message: format!("item/completed:{kind}"),
        })),
        None => Ok(Some(CodexAppServerEvent::Activity {
            message: "item/completed".to_string(),
        })),
    }
}

fn tool_name_from_item(item: &Value) -> Option<String> {
    match item.get("type").and_then(Value::as_str)? {
        "commandExecution" => Some("codex/command_execution".to_string()),
        "fileChange" => Some("codex/file_change".to_string()),
        "mcpToolCall" => {
            let server = item.get("server").and_then(Value::as_str).unwrap_or("mcp");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            Some(format!("codex/mcp/{server}/{tool}"))
        }
        "dynamicToolCall" => item
            .get("tool")
            .and_then(Value::as_str)
            .map(|tool| format!("codex/{tool}")),
        _ => None,
    }
}

fn outcome_from_item(item: &Value) -> CodexToolCallOutcome {
    match item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "completed" => CodexToolCallOutcome::Completed,
        "declined" => CodexToolCallOutcome::Cancelled,
        "failed" => CodexToolCallOutcome::Failed,
        _ => CodexToolCallOutcome::Failed,
    }
}

fn notification_delta(method: &str, params: &Value) -> Result<String, CodexLlmClientError> {
    if method == "item/mcpToolCall/progress" {
        required_string(params, "message")
    } else {
        required_string(params, "delta")
    }
}

fn item_type(item: &Value) -> String {
    item.get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}
