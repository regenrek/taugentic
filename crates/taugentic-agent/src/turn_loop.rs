use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome, StreamEmission,
};
use ta_provider_llm::client::{
    LlmClient, LlmStream, StopReason, StreamEvent, StreamMessage, StreamRequest, StreamTool,
    StreamToolCallRecord,
};
use ta_provider_llm::stream_model::StreamModel;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, debug_span, instrument, trace_span};

use crate::approval::{ApprovalBridge, ApprovalDescriptor, ApprovalOutcome};
use crate::artifacts::{ArtifactWriter, record_tool_artifact};
use crate::completion_result::parse_completion_result;
use crate::queues::MessageQueue;
use crate::session::Session;
use crate::tools::{Registry, ToolContext, ToolDescriptor, ToolOutput};
use crate::{ExecutionError, ExecutionRequest, ExecutionSink};

pub const MAX_CONTEXT_LIMIT_RETRIES: usize = 5;
pub const MAX_INCOMPLETE_CONTINUATION_ATTEMPTS: usize = 3;
pub const MAX_CONCURRENT_TOOLS: usize = 8;
pub const APPROVAL_INTERRUPT_DEADLINE: Duration = Duration::from_secs(30);

const MAX_TURNS: usize = 32;
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) async fn run(config: TurnLoopConfig) -> Result<(), ExecutionError> {
    config
        .session
        .attach_approval_bridge(config.approval_bridge.clone())?;
    let mut loop_state = TurnLoop::from_config(config);
    loop_state.execute().await
}

pub struct TurnLoopConfig {
    pub request: ExecutionRequest,
    pub sink: Arc<dyn ExecutionSink>,
    pub client: Arc<dyn LlmClient>,
    pub registry: Registry,
    pub queues: MessageQueue,
    pub session: Session,
    pub cancellation: CancellationToken,
    pub approval_bridge: Arc<ApprovalBridge>,
    pub artifact_writer: Arc<ArtifactWriter>,
}

pub struct TurnLoop {
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
    client: Arc<dyn LlmClient>,
    registry: Registry,
    queues: MessageQueue,
    session: Session,
    cancellation: CancellationToken,
    approval_bridge: Arc<ApprovalBridge>,
    artifact_writer: Arc<ArtifactWriter>,
    turns: usize,
    context_retries: usize,
    continuation_retries: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
    raw_input: String,
    input_parse_error: Option<String>,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        raw_input: impl Into<String>,
    ) -> Self {
        let raw_input = raw_input.into();
        let (input, input_parse_error) = if raw_input.trim().is_empty() {
            (json!({}), None)
        } else {
            match serde_json::from_str(&raw_input) {
                Ok(input) => (input, None),
                Err(error) => (Value::Null, Some(error.to_string())),
            }
        };
        Self {
            id: id.into(),
            name: name.into(),
            input,
            raw_input,
            input_parse_error,
        }
    }

    fn input_parse_error(&self) -> Option<&str> {
        self.input_parse_error.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolResultMessage {
    call_id: String,
    content: String,
    outcome: AgentToolCallOutcome,
}

struct ToolExecutionContext {
    registry: BTreeMap<String, Arc<dyn crate::tools::Tool>>,
    workdir: PathBuf,
    cancellation: CancellationToken,
    session: Session,
    sink: Arc<dyn ExecutionSink>,
    approval_bridge: Arc<ApprovalBridge>,
    artifact_writer: Arc<ArtifactWriter>,
    turn_id: AgentStreamTurnId,
}

impl TurnLoop {
    pub fn from_config(config: TurnLoopConfig) -> Self {
        Self {
            request: config.request,
            sink: config.sink,
            client: config.client,
            registry: config.registry,
            queues: config.queues,
            session: config.session,
            cancellation: config.cancellation,
            approval_bridge: config.approval_bridge,
            artifact_writer: config.artifact_writer,
            turns: 0,
            context_retries: 0,
            continuation_retries: 0,
        }
    }

    pub fn session(&self) -> Session {
        self.session.clone()
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn execute(&mut self) -> Result<(), ExecutionError> {
        loop {
            if self.turns > 0 {
                self.queues.drain_steering_into(&self.session).await?;
            }
            if self.turns >= MAX_TURNS {
                let model = StreamModel::<CollectedTurn>::Finish {
                    reason: "max turns reached".to_string(),
                };
                if let StreamModel::Finish { reason } = model {
                    self.sink.complete(&reason)?;
                }
                return Ok(());
            }
            if self.cancellation.is_cancelled() {
                self.session.reject_pending_approvals("turn_interrupted")?;
                return Err(ExecutionError::Cancelled("turn interrupted".to_string()));
            }
            self.turns = self.turns.saturating_add(1);

            let turn_id = turn_id(self.turns)?;
            self.push(StreamEmission {
                turn_id: Some(turn_id.clone()),
                item_id: None,
                fragment_sequence: None,
                frame: AgentStreamFrame::AssistantTurnStarted,
            })?;

            self.session.repair_missing_tool_outputs()?;
            let tools = self
                .session
                .lock_tool_list_if_unlocked(&mut self.registry)?;
            let turn = match self.stream_turn(&tools, &turn_id).await? {
                StreamModel::Stream(turn) => turn,
                StreamModel::Compact { .. } => continue,
                StreamModel::Finish { reason } => {
                    self.sink.complete(&reason)?;
                    return Ok(());
                }
            };
            self.context_retries = 0;

            let tool_calls = finalize_tool_calls(turn.stop_reason.clone(), turn.tool_calls)?;

            self.session.append_message(StreamMessage::assistant(
                turn.assistant_text.clone(),
                tool_calls
                    .iter()
                    .map(|call| StreamToolCallRecord {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        input: call.input.clone(),
                    })
                    .collect(),
            ))?;

            self.push(StreamEmission {
                turn_id: Some(turn_id.clone()),
                item_id: None,
                fragment_sequence: None,
                frame: AgentStreamFrame::AssistantTurnCompleted,
            })?;

            if tool_calls.is_empty() {
                if matches!(turn.stop_reason, StopReason::MaxTokens) {
                    if self.continuation_retries < MAX_INCOMPLETE_CONTINUATION_ATTEMPTS {
                        self.continuation_retries = self.continuation_retries.saturating_add(1);
                        continue;
                    }
                    return Err(ExecutionError::ProcessFailed(
                        "max continuation attempts reached".to_string(),
                    ));
                }
                let follow_up = self.queues.drain_follow_up_into(&self.session).await?;
                if !follow_up.is_empty() {
                    continue;
                }
                let result =
                    parse_completion_result(self.request.output_contract, &turn.assistant_text);
                self.sink.complete_with_result("normal end", result)?;
                return Ok(());
            }
            self.continuation_retries = 0;

            let results = self.execute_tools(tool_calls, &turn_id).await?;
            for result in results {
                self.session.append_message(StreamMessage::tool(
                    result.call_id.clone(),
                    result.content.clone(),
                ))?;
                self.push(StreamEmission {
                    turn_id: Some(turn_id.clone()),
                    item_id: Some(item_id(&result.call_id)?),
                    fragment_sequence: Some(0),
                    frame: AgentStreamFrame::ToolCallProgressed {
                        delta: result.content.clone(),
                    },
                })?;
                self.push(StreamEmission {
                    turn_id: Some(turn_id.clone()),
                    item_id: Some(item_id(&result.call_id)?),
                    fragment_sequence: None,
                    frame: AgentStreamFrame::ToolCallCompleted {
                        outcome: result.outcome,
                    },
                })?;
            }
        }
    }

    fn stream_request(&self, tools: &[ToolDescriptor]) -> Result<StreamRequest, ExecutionError> {
        Ok(StreamRequest {
            model: self
                .request
                .model_id
                .as_ref()
                .map(|id| id.as_str().to_string())
                .unwrap_or_default(),
            messages: self.session.history()?,
            tools: tools.iter().map(stream_tool).collect(),
            provider_session_id: self.session.provider_session_id()?,
        })
    }

    #[instrument(level = "trace", skip_all)]
    async fn stream_turn(
        &mut self,
        tools: &[ToolDescriptor],
        turn_id: &AgentStreamTurnId,
    ) -> Result<StreamModel<CollectedTurn>, ExecutionError> {
        let request = self.stream_request(tools)?;
        let stream_span = debug_span!("llm_start_stream");
        let mut stream = match self
            .client
            .start_stream(request, self.cancellation.clone())
            .instrument(stream_span)
            .await
        {
            Ok(stream) => stream,
            Err(error) => return self.compact_after_context_limit(error.into()),
        };

        let collect_span = debug_span!("llm_collect_stream");
        match self
            .collect_turn(&mut *stream, turn_id)
            .instrument(collect_span)
            .await
        {
            Ok(turn) => Ok(StreamModel::Stream(turn)),
            Err(error) => self.compact_after_context_limit(error),
        }
    }

    async fn collect_turn(
        &self,
        stream: &mut dyn LlmStream,
        turn_id: &AgentStreamTurnId,
    ) -> Result<CollectedTurn, ExecutionError> {
        let mut assistant_text = String::new();
        let mut tool_calls = BTreeMap::<u64, PartialToolCall>::new();
        let mut stop_reason = StopReason::EndTurn;
        let mut fragment_sequence = 0u64;

        loop {
            let event = match stream.next_event().await {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => return Err(error.into()),
            };

            match event {
                StreamEvent::AssistantTextDelta(delta) => {
                    assistant_text.push_str(&delta);
                    self.push(StreamEmission {
                        turn_id: Some(turn_id.clone()),
                        item_id: None,
                        fragment_sequence: Some(fragment_sequence),
                        frame: AgentStreamFrame::AssistantMessageDelta { delta },
                    })?;
                    fragment_sequence = fragment_sequence.saturating_add(1);
                }
                StreamEvent::ToolCallStarted { id, index, name } => {
                    tool_calls.insert(
                        index,
                        PartialToolCall {
                            id,
                            name,
                            input: String::new(),
                        },
                    );
                }
                StreamEvent::ToolInputDelta { id, index, delta } => {
                    let entry = tool_calls.entry(index).or_insert_with(|| PartialToolCall {
                        id,
                        name: String::new(),
                        input: String::new(),
                    });
                    entry.input.push_str(&delta);
                }
                StreamEvent::ToolCallCompleted { .. } => {}
                StreamEvent::ToolCallBatchCompleted => {
                    stop_reason = StopReason::ToolCalls;
                }
                StreamEvent::TokenUsage(usage) => {
                    self.sink.record_token_usage(usage)?;
                }
                StreamEvent::TurnCompleted {
                    stop_reason: reason,
                    provider_session_id,
                } => {
                    if let Some(id) = provider_session_id {
                        self.session.set_provider_session_id(Some(id.clone()))?;
                        self.sink.push_provider_session_id(id)?;
                    }
                    stop_reason = reason;
                    break;
                }
            }
        }

        let tool_calls = tool_calls
            .into_values()
            .map(PartialToolCall::into_tool_call)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CollectedTurn {
            assistant_text,
            tool_calls,
            stop_reason,
        })
    }

    #[instrument(level = "trace", skip_all, fields(tool_calls = tool_calls.len()))]
    async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        turn_id: &AgentStreamTurnId,
    ) -> Result<Vec<ToolResultMessage>, ExecutionError> {
        for call in &tool_calls {
            self.push(StreamEmission {
                turn_id: Some(turn_id.clone()),
                item_id: Some(item_id(&call.id)?),
                fragment_sequence: None,
                frame: AgentStreamFrame::ToolCallStarted {
                    tool_name: call.name.clone(),
                    input: serde_json::to_string(&call.input)
                        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?,
                },
            })?;
        }

        if self.client.supports_parallel_tool_calls() {
            self.execute_tools_batched(tool_calls, turn_id).await
        } else {
            self.execute_tools_serial(tool_calls, turn_id).await
        }
    }

    async fn execute_tools_serial(
        &self,
        tool_calls: Vec<ToolCall>,
        turn_id: &AgentStreamTurnId,
    ) -> Result<Vec<ToolResultMessage>, ExecutionError> {
        let mut results = Vec::with_capacity(tool_calls.len());
        for call in tool_calls {
            results.push(self.execute_one_tool(call, turn_id).await?);
        }
        Ok(results)
    }

    async fn execute_tools_batched(
        &self,
        tool_calls: Vec<ToolCall>,
        turn_id: &AgentStreamTurnId,
    ) -> Result<Vec<ToolResultMessage>, ExecutionError> {
        let mut results = Vec::with_capacity(tool_calls.len());
        let mut parallel_batch = Vec::new();

        for call in tool_calls {
            if self.tool_supports_parallel(&call)? {
                parallel_batch.push(call);
                continue;
            }
            results.extend(
                self.execute_tools_parallel(std::mem::take(&mut parallel_batch), turn_id)
                    .await?,
            );
            results.push(self.execute_one_tool(call, turn_id).await?);
        }

        results.extend(self.execute_tools_parallel(parallel_batch, turn_id).await?);
        Ok(results)
    }

    async fn execute_tools_parallel(
        &self,
        tool_calls: Vec<ToolCall>,
        turn_id: &AgentStreamTurnId,
    ) -> Result<Vec<ToolResultMessage>, ExecutionError> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TOOLS));
        let mut handles = Vec::with_capacity(tool_calls.len());
        for call in tool_calls {
            let call_id = call.id.clone();
            let tool_name = call.name.clone();
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?;
            let ctx = ToolExecutionContext {
                registry: self.registry.clone_tools(),
                workdir: self.request.effective_cwd().to_path_buf(),
                cancellation: self.cancellation.clone(),
                session: self.session.clone(),
                sink: self.sink.clone(),
                approval_bridge: self.approval_bridge.clone(),
                artifact_writer: self.artifact_writer.clone(),
                turn_id: turn_id.clone(),
            };
            handles.push(tokio::spawn(
                async move {
                    let _permit = permit;
                    execute_tool_call(call, ctx).await
                }
                .instrument(trace_span!(
                    "parallel_tool_execution",
                    call_id,
                    tool_name
                )),
            ));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(
                handle
                    .await
                    .map_err(|error| ExecutionError::ToolFailed(error.to_string()))??,
            );
        }
        Ok(results)
    }

    async fn execute_one_tool(
        &self,
        call: ToolCall,
        turn_id: &AgentStreamTurnId,
    ) -> Result<ToolResultMessage, ExecutionError> {
        let ctx = ToolExecutionContext {
            registry: self.registry.clone_tools(),
            workdir: self.request.effective_cwd().to_path_buf(),
            cancellation: self.cancellation.clone(),
            session: self.session.clone(),
            sink: self.sink.clone(),
            approval_bridge: self.approval_bridge.clone(),
            artifact_writer: self.artifact_writer.clone(),
            turn_id: turn_id.clone(),
        };
        execute_tool_call(call, ctx).await
    }

    fn tool_supports_parallel(&self, call: &ToolCall) -> Result<bool, ExecutionError> {
        let tool = self
            .registry
            .get(&call.name)
            .ok_or_else(|| ExecutionError::ToolFailed(format!("unknown tool {}", call.name)))?;
        Ok(tool.descriptor().parallel_safe)
    }

    fn compact_after_context_limit(
        &mut self,
        error: ExecutionError,
    ) -> Result<StreamModel<CollectedTurn>, ExecutionError> {
        match error {
            ExecutionError::ContextLengthExceeded(detail) => {
                if self.context_retries < MAX_CONTEXT_LIMIT_RETRIES {
                    self.context_retries = self.context_retries.saturating_add(1);
                    self.session.compact()?;
                    return Ok(StreamModel::Compact {
                        attempt: self.context_retries,
                    });
                }
                Err(ExecutionError::ContextLengthExceeded(detail))
            }
            other => Err(other),
        }
    }

    fn push(&self, emission: StreamEmission) -> Result<(), ExecutionError> {
        self.sink.push_stream(emission)
    }
}

pub fn finalize_tool_calls(
    stop_reason: StopReason,
    tool_calls: Vec<ToolCall>,
) -> Result<Vec<ToolCall>, ExecutionError> {
    if matches!(stop_reason, StopReason::MaxTokens) {
        return Ok(filter_truncated_tool_calls(stop_reason, tool_calls));
    }

    if let Some(call) = tool_calls
        .iter()
        .find(|call| call.input_parse_error().is_some())
    {
        return Err(ExecutionError::InvalidToolInput(format!(
            "tool call {} input is invalid JSON: {}",
            call.id,
            call.input_parse_error().unwrap_or("unknown parse error")
        )));
    }
    Ok(tool_calls)
}

pub fn filter_truncated_tool_calls(
    stop_reason: StopReason,
    tool_calls: Vec<ToolCall>,
) -> Vec<ToolCall> {
    if !matches!(stop_reason, StopReason::MaxTokens) {
        return tool_calls;
    }

    tool_calls
        .into_iter()
        .filter(|call| {
            let input = call.raw_input.trim();
            !(input.is_empty() || input == "null" || call.input_parse_error().is_some())
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CollectedTurn {
    assistant_text: String,
    tool_calls: Vec<ToolCall>,
    stop_reason: StopReason,
}

#[derive(Debug, Clone)]
struct PartialToolCall {
    id: String,
    name: String,
    input: String,
}

impl PartialToolCall {
    fn into_tool_call(self) -> Result<ToolCall, ExecutionError> {
        Ok(ToolCall::new(self.id, self.name, self.input))
    }
}

#[instrument(level = "trace", skip_all, fields(call_id = %call.id, tool_name = %call.name))]
async fn execute_tool_call(
    call: ToolCall,
    ctx: ToolExecutionContext,
) -> Result<ToolResultMessage, ExecutionError> {
    let tool = ctx
        .registry
        .get(&call.name)
        .cloned()
        .ok_or_else(|| ExecutionError::ToolFailed(format!("unknown tool {}", call.name)))?;
    let descriptor = tool.descriptor();
    if let Some(scope) = descriptor.approval_scope {
        let approval = ApprovalDescriptor::new(
            call.id.clone(),
            call.name.clone(),
            format!("tool {} requires approval", call.name),
        );
        let id = ctx.approval_bridge.request(scope, &approval)?;
        let outcome = match ctx.approval_bridge.wait(id).await {
            Ok(outcome) => outcome,
            Err(ExecutionError::Cancelled(detail)) => {
                push_tool_terminal(
                    &ctx.sink,
                    &ctx.turn_id,
                    &call.id,
                    AgentToolCallOutcome::Cancelled,
                )?;
                ctx.session.reject_pending_approvals("turn_interrupted")?;
                return Err(ExecutionError::Cancelled(detail));
            }
            Err(error) => return Err(error),
        };
        match outcome {
            ApprovalOutcome::Allow => {}
            ApprovalOutcome::Deny => {
                ctx.sink
                    .push_activity(&format!("tool {} denied by approval", call.name))?;
                push_tool_terminal(
                    &ctx.sink,
                    &ctx.turn_id,
                    &call.id,
                    AgentToolCallOutcome::Cancelled,
                )?;
                return Err(ExecutionError::Cancelled(format!(
                    "approval denied for tool {}",
                    call.name
                )));
            }
            ApprovalOutcome::TurnInterrupted => {
                push_tool_terminal(
                    &ctx.sink,
                    &ctx.turn_id,
                    &call.id,
                    AgentToolCallOutcome::Cancelled,
                )?;
                ctx.session.reject_pending_approvals("turn_interrupted")?;
                return Err(ExecutionError::Cancelled("turn_interrupted".to_string()));
            }
        }
    }

    let tool_ctx = ToolContext {
        workdir: ctx.workdir.clone(),
        cancellation_token: ctx.cancellation.clone(),
        timeout: DEFAULT_TOOL_TIMEOUT,
        parent_turn_id: Some(ctx.turn_id.clone()),
    };
    let started = Instant::now();
    let output = tool.run(call.input, tool_ctx).await;
    let duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let tool_name = call.name.clone();
    Ok(tool_result(
        call.id,
        tool_name.clone(),
        duration_ms,
        output.and_then(|output| {
            record_tool_artifact(&ctx.artifact_writer, &ctx.sink, &tool_name, &output)?;
            Ok(output)
        }),
    ))
}

fn tool_result(
    call_id: String,
    tool_name: String,
    duration_ms: u64,
    output: Result<ToolOutput, ExecutionError>,
) -> ToolResultMessage {
    match output {
        Ok(output) => ToolResultMessage {
            call_id,
            content: json!({
                "tool": tool_name,
                "duration_ms": duration_ms,
                "output": output.content,
            })
            .to_string(),
            outcome: AgentToolCallOutcome::Completed,
        },
        Err(error) => {
            let outcome = if matches!(error, ExecutionError::Cancelled(_)) {
                AgentToolCallOutcome::Cancelled
            } else {
                AgentToolCallOutcome::Failed
            };
            ToolResultMessage {
                call_id,
                content: json!({
                    "tool": tool_name,
                    "duration_ms": duration_ms,
                    "error": error.to_string(),
                })
                .to_string(),
                outcome,
            }
        }
    }
}

fn push_tool_terminal(
    sink: &Arc<dyn ExecutionSink>,
    turn_id: &AgentStreamTurnId,
    call_id: &str,
    outcome: AgentToolCallOutcome,
) -> Result<(), ExecutionError> {
    sink.push_stream(StreamEmission {
        turn_id: Some(turn_id.clone()),
        item_id: Some(item_id(call_id)?),
        fragment_sequence: None,
        frame: AgentStreamFrame::ToolCallCompleted { outcome },
    })
}

fn stream_tool(tool: &ToolDescriptor) -> StreamTool {
    StreamTool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
    }
}

fn turn_id(turn: usize) -> Result<AgentStreamTurnId, ExecutionError> {
    AgentStreamTurnId::new(format!("turn-{turn}"))
        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))
}

fn item_id(id: &str) -> Result<AgentStreamItemId, ExecutionError> {
    AgentStreamItemId::new(id.to_string())
        .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))
}
