use std::sync::{Arc, Mutex, MutexGuard};

use ta_provider_llm::client::{StreamMessage, StreamRole, StreamToolCallRecord};

use crate::approval::ApprovalBridge;
use crate::tools::{Registry, ToolDescriptor};
use crate::{ExecutionError, ExecutionRequest};

#[derive(Clone)]
pub struct Session {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<SessionState>,
}

#[derive(Clone)]
struct SessionState {
    history: Vec<StreamMessage>,
    provider_session_id: Option<String>,
    locked_tools: Option<Vec<ToolDescriptor>>,
    compact_count: usize,
    approval_bridge: Option<Arc<ApprovalBridge>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: String,
    pub reason: String,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Allowed,
    Rejected { reason: String },
}

impl Session {
    pub fn new(request: &ExecutionRequest) -> Self {
        Self::from_request_history(
            vec![StreamMessage::user(request.objective.clone())],
            request.resume_provider_session_id.clone(),
            request.system_prompt.as_deref(),
        )
    }

    pub fn from_history(history: Vec<StreamMessage>, provider_session_id: Option<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(SessionState {
                    history,
                    provider_session_id,
                    locked_tools: None,
                    compact_count: 0,
                    approval_bridge: None,
                }),
            }),
        }
    }

    pub fn from_request_history(
        history: Vec<StreamMessage>,
        provider_session_id: Option<String>,
        system_prompt: Option<&str>,
    ) -> Self {
        Self::from_history(
            prepend_system_prompt(history, system_prompt),
            provider_session_id,
        )
    }

    pub fn history(&self) -> Result<Vec<StreamMessage>, ExecutionError> {
        Ok(self.state()?.history.clone())
    }

    pub fn provider_session_id(&self) -> Result<Option<String>, ExecutionError> {
        Ok(self.state()?.provider_session_id.clone())
    }

    pub fn set_provider_session_id(&self, id: Option<String>) -> Result<(), ExecutionError> {
        self.state()?.provider_session_id = id;
        Ok(())
    }

    pub fn append_message(&self, message: StreamMessage) -> Result<(), ExecutionError> {
        self.state()?.history.push(message);
        Ok(())
    }

    pub fn append_messages(&self, messages: Vec<StreamMessage>) -> Result<(), ExecutionError> {
        if messages.is_empty() {
            return Ok(());
        }
        self.state()?.history.extend(messages);
        Ok(())
    }

    pub fn lock_tool_list_if_unlocked(
        &self,
        registry: &mut Registry,
    ) -> Result<Vec<ToolDescriptor>, ExecutionError> {
        let mut state = self.state()?;
        if let Some(tools) = &state.locked_tools {
            return Ok(tools.clone());
        }
        let tools = registry.lock_tool_list();
        state.locked_tools = Some(tools.clone());
        Ok(tools)
    }

    pub fn locked_tools(&self) -> Result<Option<Vec<ToolDescriptor>>, ExecutionError> {
        Ok(self.state()?.locked_tools.clone())
    }

    pub fn compact(&self) -> Result<(), ExecutionError> {
        let mut state = self.state()?;
        if state.history.len() > 2 {
            state.history.remove(1);
        }
        state.compact_count = state.compact_count.saturating_add(1);
        Ok(())
    }

    pub fn compact_count(&self) -> Result<usize, ExecutionError> {
        Ok(self.state()?.compact_count)
    }

    pub fn attach_approval_bridge(
        &self,
        bridge: Arc<ApprovalBridge>,
    ) -> Result<(), ExecutionError> {
        self.state()?.approval_bridge = Some(bridge);
        Ok(())
    }

    pub fn repair_missing_tool_outputs(&self) -> Result<usize, ExecutionError> {
        let mut state = self.state()?;
        let missing = missing_tool_results(&state.history);
        let count = missing.len();
        state
            .history
            .extend(missing.into_iter().map(|tool_call_id| {
                StreamMessage::tool(
                    tool_call_id,
                    r#"{"marker":"missing_tool_output_repaired","interrupted":true}"#,
                )
            }));
        Ok(count)
    }

    pub fn reject_pending_approvals(
        &self,
        reason: impl Into<String>,
    ) -> Result<Vec<PendingApproval>, ExecutionError> {
        let reason = reason.into();
        let bridge = self.approval_bridge()?;
        bridge.reject_all(if reason == "turn_interrupted" {
            "turn_interrupted"
        } else {
            "approval_rejected"
        });
        bridge.pending_approvals()
    }

    pub fn pending_approvals(&self) -> Result<Vec<PendingApproval>, ExecutionError> {
        let Some(bridge) = self.state()?.approval_bridge.clone() else {
            return Ok(Vec::new());
        };
        bridge.pending_approvals()
    }

    fn state(&self) -> Result<MutexGuard<'_, SessionState>, ExecutionError> {
        self.inner
            .state
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("session state lock poisoned".to_string()))
    }

    fn approval_bridge(&self) -> Result<Arc<ApprovalBridge>, ExecutionError> {
        self.state()?.approval_bridge.clone().ok_or_else(|| {
            ExecutionError::Unsupported("approval bridge is not attached".to_string())
        })
    }
}

fn missing_tool_results(history: &[StreamMessage]) -> Vec<String> {
    let mut calls = Vec::<String>::new();
    let mut results = Vec::<String>::new();
    for message in history {
        for call in &message.tool_calls {
            calls.push(call.id.clone());
        }
        if let Some(tool_call_id) = &message.tool_call_id {
            results.push(tool_call_id.clone());
        }
    }
    calls
        .into_iter()
        .filter(|call| !results.iter().any(|result| result == call))
        .collect()
}

fn prepend_system_prompt(
    mut history: Vec<StreamMessage>,
    system_prompt: Option<&str>,
) -> Vec<StreamMessage> {
    let Some(system_prompt) = system_prompt
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
    else {
        return history;
    };
    if let Some(first) = history.first_mut()
        && matches!(first.role, StreamRole::System)
    {
        if first.content == system_prompt {
            return history;
        }
        first.content = format!("{system_prompt}\n\n{}", first.content);
        return history;
    }
    history.insert(0, StreamMessage::system(system_prompt.to_string()));
    history
}

pub fn assistant_tool_message(tool_calls: Vec<StreamToolCallRecord>) -> StreamMessage {
    StreamMessage::assistant("", tool_calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_request_history_prepends_system_prompt() {
        let session = Session::from_request_history(
            vec![StreamMessage::user("ship it")],
            None,
            Some("# Delegation guidelines"),
        );

        let history = session.history().expect("history");
        assert_eq!(history[0].role, StreamRole::System);
        assert_eq!(history[0].content, "# Delegation guidelines");
        assert_eq!(history[1].role, StreamRole::User);
    }

    #[test]
    fn from_request_history_prepends_to_existing_system_prompt() {
        let session = Session::from_request_history(
            vec![
                StreamMessage::system("Existing system prompt"),
                StreamMessage::user("ship it"),
            ],
            None,
            Some("# Delegation guidelines"),
        );

        let history = session.history().expect("history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, StreamRole::System);
        assert_eq!(
            history[0].content,
            "# Delegation guidelines\n\nExisting system prompt"
        );
    }

    #[test]
    fn from_request_history_without_system_prompt_preserves_history() {
        let session =
            Session::from_request_history(vec![StreamMessage::user("ship it")], None, None);

        assert_eq!(
            session.history().expect("history"),
            vec![StreamMessage::user("ship it")]
        );
    }
}
