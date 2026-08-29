use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome, StreamEmission,
};
use ta_provider_llm::families::codex_app_server::{
    CodexAppServerClient, CodexAppServerEvent, CodexAppServerInput, CodexLlmClientError,
    CodexToolCallOutcome,
};
use tokio_util::sync::CancellationToken;

use crate::{ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink};

#[tracing::instrument(skip(request, sink), fields(runtime_profile = %request.runtime_profile_id.as_str()))]
pub(crate) async fn dispatch(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    dispatch_with_client(request, sink, CodexAppServerClient::default())
}

#[doc(hidden)]
pub fn dispatch_with_client(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    client: CodexAppServerClient,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let cancellation = CancellationToken::new();
    let handle_cancellation = cancellation.clone();
    let handle = std::thread::Builder::new()
        .name(format!(
            "taugentic-codex-app-server-{}",
            request.run_id.as_str()
        ))
        .spawn(move || {
            if let Err(error) = run_codex_app_server(request, sink.clone(), client, cancellation) {
                let _ = sink.fail(error);
            }
        })
        .map_err(|error| {
            ExecutionError::ProcessFailed(format!("failed to spawn Codex app-server lane: {error}"))
        })?;
    Ok(Arc::new(CodexAppServerExecutionHandle {
        cancellation: handle_cancellation,
        thread: Mutex::new(Some(handle)),
    }))
}

struct CodexAppServerExecutionHandle {
    cancellation: CancellationToken,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl ExecutionHandle for CodexAppServerExecutionHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.cancellation.cancel();
        Ok(())
    }
}

impl Drop for CodexAppServerExecutionHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut thread) = self.thread.lock()
            && thread.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(handle) = thread.take()
        {
            let _ = handle.join();
        }
    }
}

fn run_codex_app_server(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    client: CodexAppServerClient,
    cancellation: CancellationToken,
) -> Result<(), ExecutionError> {
    let mut session = client
        .start_session(CodexAppServerInput {
            execution_context: request.execution_context.clone(),
            model: request
                .model_id
                .as_ref()
                .map(|model| model.as_str().to_string()),
            auth_profile_id: request
                .auth_profile_id
                .as_ref()
                .map(|profile| profile.as_str().to_string()),
        })
        .map_err(map_codex_error)?;
    sink.push_activity("codex app-server session started")?;
    let local_images = request
        .attachments
        .iter()
        .filter(|attachment| attachment.kind == ta_protocol::wire::WorkspaceFileKind::Image)
        .map(|attachment| request.effective_cwd().join(&attachment.path))
        .collect::<Vec<PathBuf>>();
    session
        .send_user_turn(&request.objective, &local_images)
        .map_err(map_codex_error)?;
    // The app-server transport is blocking JSONL, so cancellation is polled in
    // the provider stream loop instead of using tokio::select! here.
    session
        .stream_events(&cancellation, |event| {
            push_event(event, sink.as_ref())
                .map_err(|error| CodexLlmClientError::Protocol(error.to_string()))
        })
        .map_err(map_codex_error)?;
    sink.complete("Codex app-server execution completed")
}

fn push_event(event: CodexAppServerEvent, sink: &dyn ExecutionSink) -> Result<(), ExecutionError> {
    match event {
        CodexAppServerEvent::AgentMessageDelta {
            turn_id,
            item_id,
            delta,
        } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: Some(item_id_from_string(item_id)?),
            fragment_sequence: None,
            frame: AgentStreamFrame::AssistantMessageDelta { delta },
        }),
        CodexAppServerEvent::TurnStarted { turn_id } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::AssistantTurnStarted,
        }),
        CodexAppServerEvent::ToolCallStarted {
            turn_id,
            item_id,
            tool_name,
        } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: Some(item_id_from_string(item_id)?),
            fragment_sequence: None,
            frame: AgentStreamFrame::ToolCallStarted {
                tool_name,
                input: "null".to_string(),
            },
        }),
        CodexAppServerEvent::ToolCallProgressed {
            turn_id,
            item_id,
            delta,
        }
        | CodexAppServerEvent::ReasoningDelta {
            turn_id,
            item_id,
            delta,
        } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: Some(item_id_from_string(item_id)?),
            fragment_sequence: None,
            frame: AgentStreamFrame::ToolCallProgressed { delta },
        }),
        CodexAppServerEvent::ToolCallCompleted {
            turn_id,
            item_id,
            outcome,
        } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: Some(item_id_from_string(item_id)?),
            fragment_sequence: None,
            frame: AgentStreamFrame::ToolCallCompleted {
                outcome: tool_outcome(outcome),
            },
        }),
        CodexAppServerEvent::TurnCompleted { turn_id } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::AssistantTurnCompleted,
        }),
        CodexAppServerEvent::TokenCount {
            turn_id,
            total_tokens,
            model_context_window,
        } => sink.push_stream(StreamEmission {
            turn_id: Some(turn_id_from_string(turn_id)?),
            item_id: None,
            fragment_sequence: None,
            frame: AgentStreamFrame::TokenUsageUpdated {
                total_tokens,
                model_context_window,
            },
        }),
        CodexAppServerEvent::ApprovalRequested {
            turn_id,
            item_id,
            detail,
        } => sink.push_activity(&format!(
            "codex approval requested: {detail}; turn={}; item={}",
            turn_id.unwrap_or_else(|| "unknown".to_string()),
            item_id.unwrap_or_else(|| "unknown".to_string())
        )),
        CodexAppServerEvent::Activity { message } => sink.push_activity(&message),
        CodexAppServerEvent::ImageGenerated {
            turn_id,
            item_id,
            data_base64,
        } => sink.record_image_artifact(
            turn_id_from_string(turn_id)?,
            item_id_from_string(item_id)?,
            &data_base64,
        ),
    }
}

fn turn_id_from_string(id: String) -> Result<AgentStreamTurnId, ExecutionError> {
    AgentStreamTurnId::new(id).map_err(|error| ExecutionError::ProcessFailed(error.to_string()))
}

fn item_id_from_string(id: String) -> Result<AgentStreamItemId, ExecutionError> {
    AgentStreamItemId::new(id).map_err(|error| ExecutionError::ProcessFailed(error.to_string()))
}

fn map_codex_error(error: CodexLlmClientError) -> ExecutionError {
    match error {
        CodexLlmClientError::UnknownAuthProfile(detail) => ExecutionError::InvalidConfig(detail),
        CodexLlmClientError::CliUnavailable(detail) => ExecutionError::Unsupported(detail),
        CodexLlmClientError::CommandTimedOut(detail) => ExecutionError::ProcessTimeout {
            timeout_ms: 0,
            detail,
        },
        CodexLlmClientError::CommandFailed(detail) => ExecutionError::ProcessFailed(detail),
        CodexLlmClientError::Auth(detail) => ExecutionError::Auth(detail),
        CodexLlmClientError::RateLimited {
            retry_after_ms,
            detail,
        } => ExecutionError::RateLimited {
            retry_after_ms,
            detail,
        },
        CodexLlmClientError::CreditsExhausted(detail) => ExecutionError::CreditsExhausted(detail),
        CodexLlmClientError::ContextLengthExceeded(detail) => {
            ExecutionError::ContextLengthExceeded(detail)
        }
        CodexLlmClientError::InvalidConfig(detail) => ExecutionError::InvalidConfig(detail),
        CodexLlmClientError::Protocol(detail) => ExecutionError::ProcessFailed(detail),
        CodexLlmClientError::JsonRpc {
            code,
            message,
            data,
        } => ExecutionError::ServerError(format!(
            "codex app-server JSON-RPC error {code}: {message}; data={}",
            data.map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        )),
        CodexLlmClientError::Cancelled(detail) => ExecutionError::Cancelled(detail),
    }
}

fn tool_outcome(outcome: CodexToolCallOutcome) -> AgentToolCallOutcome {
    match outcome {
        CodexToolCallOutcome::Completed => AgentToolCallOutcome::Completed,
        CodexToolCallOutcome::Failed => AgentToolCallOutcome::Failed,
        CodexToolCallOutcome::Cancelled => AgentToolCallOutcome::Cancelled,
    }
}
