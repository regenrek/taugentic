use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ta_protocol::wire::{ApprovalResolution, LocalModelAuthMode};
use ta_provider_llm::client::LlmClient;
use ta_provider_llm::client::anthropic_messages::AnthropicMessagesClient;
use ta_provider_llm::client::openai_compatible::{OpenAiCompatibleAuth, OpenAiCompatibleClient};
use ta_provider_llm::client::openai_responses::OpenAiResponsesClient;
use ta_provider_llm::declarative;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::approval::ApprovalBridge;
use crate::artifacts::ArtifactWriter;
use crate::mcp::McpToolRegistry;
use crate::queues::MessageQueue;
use crate::session::Session;
use crate::tools::{Registry, SubagentTool};
use crate::turn_loop::{self, TurnLoopConfig};
use crate::{ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink};
use ta_provider_llm::families::{anthropic::ANTHROPIC_PROVIDER_ID, openai::OPENAI_PROVIDER_ID};

#[instrument(level = "debug", skip_all, fields(runtime_profile_id = %request.runtime_profile_id.as_str()))]
pub(crate) async fn dispatch(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    let client = client_for_request(&request)?;
    let cancellation = CancellationToken::new();
    let mut registry = Registry::with_all_builtins();
    registry.add(SubagentTool::new(
        request.run_id.clone(),
        sink.clone(),
        request.subagent_recipes.clone(),
    ))?;
    let mcp_registry = McpToolRegistry::mount_from_request(&mut registry, &request).await?;
    let approval_bridge = Arc::new(ApprovalBridge::new(
        request.run_id.clone(),
        sink.clone(),
        cancellation.clone(),
    ));
    let handle = Arc::new(NativeExecutionHandle {
        cancellation: cancellation.clone(),
        thread: Mutex::new(None),
        mcp_registry,
        approval_bridge: approval_bridge.clone(),
    });
    let thread_handle = spawn_loop(
        request,
        sink,
        client,
        registry,
        cancellation,
        approval_bridge,
    )?;
    *handle.thread.lock().map_err(|_| {
        ExecutionError::ProcessFailed("execution handle lock poisoned".to_string())
    })? = Some(thread_handle);
    Ok(handle)
}

struct NativeExecutionHandle {
    cancellation: CancellationToken,
    thread: Mutex<Option<JoinHandle<()>>>,
    mcp_registry: McpToolRegistry,
    approval_bridge: Arc<ApprovalBridge>,
}

impl ExecutionHandle for NativeExecutionHandle {
    fn cancel(&self) -> Result<(), ExecutionError> {
        self.cancellation.cancel();
        Ok(())
    }

    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        self.approval_bridge.resolve_from_runtime(resolution)
    }
}

impl Drop for NativeExecutionHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        let _ = self.mcp_registry.is_empty();
        if let Ok(mut thread) = self.thread.lock()
            && thread.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(handle) = thread.take()
        {
            let _ = handle.join();
        }
    }
}

fn spawn_loop(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    client: Arc<dyn LlmClient>,
    registry: Registry,
    cancellation: CancellationToken,
    approval_bridge: Arc<ApprovalBridge>,
) -> Result<JoinHandle<()>, ExecutionError> {
    let session = session_for_request(&request);
    spawn_loop_with_parts(
        request,
        sink,
        client,
        registry,
        cancellation,
        LoopRuntimeParts {
            queues: MessageQueue::default(),
            session,
            approval_bridge,
        },
    )
}

/// Builds the native session, appending the fork objective to caller-provided
/// fork history as the next user message.
fn session_for_request(request: &ExecutionRequest) -> Session {
    let Some(initial_state) = request.fork_initial_state.clone() else {
        return Session::new(request);
    };
    let mut history = initial_state.messages;
    history.push(ta_provider_llm::client::StreamMessage::user(
        request.objective.clone(),
    ));
    Session::from_request_history(
        history,
        initial_state.provider_session_id,
        request.system_prompt.as_deref(),
    )
}

struct LoopRuntimeParts {
    queues: MessageQueue,
    session: Session,
    approval_bridge: Arc<ApprovalBridge>,
}

fn spawn_loop_with_parts(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    client: Arc<dyn LlmClient>,
    registry: Registry,
    cancellation: CancellationToken,
    parts: LoopRuntimeParts,
) -> Result<JoinHandle<()>, ExecutionError> {
    let artifact_writer = Arc::new(ArtifactWriter::new(
        &request.artifact_root,
        request.run_id.clone(),
    )?);
    std::thread::Builder::new()
        .name(format!("taugentic-native-loop-{}", request.run_id.as_str()))
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let error = ExecutionError::ProcessFailed(error.to_string());
                    let _ = sink.fail(error);
                    return;
                }
            };
            let result = runtime.block_on(turn_loop::run(TurnLoopConfig {
                request,
                sink: sink.clone(),
                client,
                registry,
                queues: parts.queues,
                session: parts.session,
                cancellation,
                approval_bridge: parts.approval_bridge,
                artifact_writer,
            }));
            if let Err(error) = result {
                let _ = sink.fail(error);
            }
        })
        .map_err(|error| {
            ExecutionError::ProcessFailed(format!("failed to spawn native loop: {error}"))
        })
}

fn client_for_request(request: &ExecutionRequest) -> Result<Arc<dyn LlmClient>, ExecutionError> {
    let model = request
        .model_id
        .as_ref()
        .map(|model| model.as_str().to_string())
        .unwrap_or_default();
    let provider_id = request.provider_id.as_str();
    if provider_id == ANTHROPIC_PROVIDER_ID {
        return Ok(Arc::new(AnthropicMessagesClient::from_env(model)?));
    }
    if provider_id == OPENAI_PROVIDER_ID {
        return Ok(Arc::new(OpenAiResponsesClient::from_auth_profile(
            model,
            request.auth_profile_id.as_ref(),
        )?));
    }
    if let Some(endpoint) = request.local_endpoint.as_ref() {
        let model = if model.trim().is_empty() {
            endpoint
                .default_model
                .as_ref()
                .map(|model| model.as_str().to_string())
                .unwrap_or_default()
        } else {
            model
        };
        let auth = match endpoint.auth_mode {
            LocalModelAuthMode::None => None,
            LocalModelAuthMode::BearerEnv => {
                let env = endpoint.api_key_env.as_deref().ok_or_else(|| {
                    ExecutionError::InvalidConfig(
                        "local model bearer-env auth requires apiKeyEnv".to_string(),
                    )
                })?;
                Some(OpenAiCompatibleAuth::BearerEnv(env.to_string()))
            }
        };
        let supports_parallel_tool_calls = endpoint
            .capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.parallel_tool_calls)
            .unwrap_or(false);
        return Ok(Arc::new(OpenAiCompatibleClient::new_local(
            endpoint.base_url.as_str(),
            auth,
            model,
            supports_parallel_tool_calls,
        )?));
    }
    if let Some(spec) = declarative_spec_for_provider(provider_id) {
        let model = if model.trim().is_empty() {
            spec.default_model.as_ref().to_string()
        } else {
            model
        };
        let client = OpenAiCompatibleClient::new(spec.base_url.as_ref(), spec.auth.clone(), model)?;
        let chat_path = if spec.completions_prefix.trim().is_empty() {
            "chat/completions".to_string()
        } else {
            format!(
                "{}/chat/completions",
                spec.completions_prefix.trim().trim_matches('/')
            )
        };
        return Ok(Arc::new(client.with_chat_completions_path(chat_path)?));
    }
    Err(ExecutionError::Unsupported(format!(
        "native loop client is not configured for provider {} on runtime profile {}",
        provider_id,
        request.runtime_profile_id.as_str()
    )))
}

fn declarative_spec_for_provider(
    provider_id: &str,
) -> Option<&'static declarative::DeclarativeProviderSpec> {
    declarative::specs()
        .iter()
        .find(|spec| spec.id.as_ref() == provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{
        AgentRuntimeModelId, AgentRuntimeStrategyId, LocalModelApiStandard, LocalModelAuthMode,
        LocalModelEndpointConfig, RunId, RuntimePolicyMode, RuntimeProfileId, SessionId,
    };

    #[test]
    fn declarative_openai_compatible_providers_resolve_to_native_specs() {
        let spec = declarative_spec_for_provider("openrouter")
            .expect("openrouter provider should resolve");

        assert_eq!(spec.id.as_ref(), "openrouter");
        assert_eq!(spec.base_url.as_ref(), "https://openrouter.ai/api/v1");
    }

    #[test]
    fn local_endpoint_request_resolves_to_native_client() {
        let request = ExecutionRequest {
            session_id: SessionId::new("session-test").expect("session id"),
            run_id: RunId::new("run-test").expect("run id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-local-custom")
                .expect("runtime profile id"),
            provider_id: AgentRuntimeStrategyId::new("local-model").expect("provider id"),
            execution_harness: crate::AgentExecutionHarness::NativeLoop,
            system_prompt: None,
            objective: "test".to_string(),
            model_id: Some(AgentRuntimeModelId::new("local-model").expect("model id")),
            auth_profile_id: None,
            local_endpoint: Some(LocalModelEndpointConfig {
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                api_standard: LocalModelApiStandard::OllamaOpenAi,
                auth_mode: LocalModelAuthMode::None,
                api_key_env: None,
                default_model: None,
                model_discovery: true,
                capabilities: None,
            }),
            policy_mode: RuntimePolicyMode::RequireApproval,
            resume_provider_session_id: None,
            runtime_extensions: Vec::new(),
            working_directory: ".".into(),
            artifact_root: "target/test-artifacts".into(),
            fork_initial_state: None,
            output_contract: None,
            sandbox_profile: None,
            subagent_recipes: Vec::new(),
        };

        let client = client_for_request(&request).expect("local endpoint client");
        assert!(!client.supports_parallel_tool_calls());
    }
}
