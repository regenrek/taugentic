use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::RunId;

macro_rules! identifier {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema, TS)]
        #[schemars(transparent)]
        #[ts(export_to = "generated/")]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, crate::wire::DomainError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(crate::wire::DomainError::EmptyIdentifier($label));
                }

                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identifier!(AgentStreamTurnId, "agent stream turn");
identifier!(AgentStreamItemId, "agent stream item");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AgentToolCallOutcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RuntimeLanePendingState {
    Queued,
    WaitingForApproval,
    WaitingForInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentStreamEvent {
    pub run_id: RunId,
    #[serde(flatten)]
    pub emission: StreamEmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct StreamEmission {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<AgentStreamTurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<AgentStreamItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment_sequence: Option<u64>,
    pub frame: AgentStreamFrame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AgentStreamFrame {
    AssistantTurnStarted,
    AssistantMessageDelta {
        delta: String,
    },
    AssistantTurnCompleted,
    ToolCallStarted {
        #[serde(rename = "toolName")]
        #[ts(rename = "toolName")]
        tool_name: String,
        input: String,
    },
    ToolCallProgressed {
        delta: String,
    },
    ToolCallCompleted {
        outcome: AgentToolCallOutcome,
    },
    PendingStateChanged {
        state: RuntimeLanePendingState,
    },
    TokenUsageUpdated {
        #[serde(rename = "totalTokens")]
        #[ts(rename = "totalTokens")]
        total_tokens: Option<u64>,
        #[serde(rename = "modelContextWindow")]
        #[ts(rename = "modelContextWindow")]
        model_context_window: Option<u64>,
    },
}
