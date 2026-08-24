#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use ta_protocol::wire::{
    AgentRuntimeModelId, ApprovalRequest, ApprovalResolution, ArtifactKind, AuthProfileId,
    EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy,
    RuntimeProfileId, SandboxProfile, SessionId, StreamEmission, WorkspaceId, WorkspacePath,
    WorkspaceScope,
};
use ta_provider_llm::client::{
    LlmClient, LlmStream, StopReason, StreamEvent, StreamMessage, StreamRequest, VecLlmStream,
};
use ta_provider_llm::error::LlmClientError;
use taugentic_agent::approval::{ApprovalBridge, ApprovalOutcome};
use taugentic_agent::artifacts::ArtifactWriter;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::tools::{Registry, Tool, ToolContext, ToolDescriptor, ToolOutput};
use taugentic_agent::turn_loop::{TurnLoop, TurnLoopConfig};
use taugentic_agent::{ExecutionError, ExecutionRequest, ExecutionSink};
use tokio_util::sync::CancellationToken;

pub enum MockStart {
    Stream(Vec<Result<StreamEvent, LlmClientError>>),
    Error(LlmClientError),
}

pub struct MockClient {
    starts: Mutex<VecDeque<MockStart>>,
    requests: Mutex<Vec<StreamRequest>>,
    parallel: bool,
}

impl MockClient {
    pub fn new(starts: Vec<MockStart>, parallel: bool) -> Arc<Self> {
        Arc::new(Self {
            starts: Mutex::new(VecDeque::from(starts)),
            requests: Mutex::new(Vec::new()),
            parallel,
        })
    }

    pub fn requests(&self) -> Vec<StreamRequest> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl LlmClient for MockClient {
    async fn start_stream(
        &self,
        request: StreamRequest,
        _cancellation: CancellationToken,
    ) -> Result<Box<dyn LlmStream>, LlmClientError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request);
        }
        let start = self
            .starts
            .lock()
            .map_err(|_| LlmClientError::ProcessFailed("mock start lock poisoned".to_string()))?
            .pop_front()
            .unwrap_or_else(|| MockStart::Stream(end_turn()));
        match start {
            MockStart::Stream(events) => Ok(Box::new(VecLlmStream::new(events))),
            MockStart::Error(error) => Err(error),
        }
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.parallel
    }
}

#[derive(Default)]
pub struct TestSink {
    pub streams: Mutex<Vec<StreamEmission>>,
    pub activities: Mutex<Vec<String>>,
    pub approval_requests: Mutex<Vec<ApprovalRequest>>,
    pub approval_resolutions: Mutex<Vec<ApprovalResolution>>,
    pub artifacts: Mutex<Vec<(ArtifactKind, String)>>,
    pub completed: Mutex<Vec<String>>,
    pub failed: Mutex<Vec<ExecutionError>>,
    fail_approval_resolution: AtomicBool,
}

impl TestSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn stream_frames(&self) -> Vec<StreamEmission> {
        self.streams
            .lock()
            .map(|streams| streams.clone())
            .unwrap_or_default()
    }

    pub fn approval_requests(&self) -> Vec<ApprovalRequest> {
        self.approval_requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }

    pub fn activities(&self) -> Vec<String> {
        self.activities
            .lock()
            .map(|activities| activities.clone())
            .unwrap_or_default()
    }

    pub fn approval_resolutions(&self) -> Vec<ApprovalResolution> {
        self.approval_resolutions
            .lock()
            .map(|resolutions| resolutions.clone())
            .unwrap_or_default()
    }

    pub fn artifacts(&self) -> Vec<(ArtifactKind, String)> {
        self.artifacts
            .lock()
            .map(|artifacts| artifacts.clone())
            .unwrap_or_default()
    }

    pub fn set_approval_resolution_failure(&self, fail: bool) {
        self.fail_approval_resolution.store(fail, Ordering::SeqCst);
    }
}

impl ExecutionSink for TestSink {
    fn push_stream(&self, emission: StreamEmission) -> Result<(), ExecutionError> {
        self.streams
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("stream lock poisoned".to_string()))?
            .push(emission);
        Ok(())
    }

    fn record_token_usage(
        &self,
        _: ta_provider_llm::client::LlmTokenUsage,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn push_activity(&self, detail: &str) -> Result<(), ExecutionError> {
        self.activities
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("activity lock poisoned".to_string()))?
            .push(detail.to_string());
        Ok(())
    }

    fn push_provider_session_id(&self, _id: String) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn request_approval(&self, request: ApprovalRequest) -> Result<(), ExecutionError> {
        self.approval_requests
            .lock()
            .map_err(|_| {
                ExecutionError::ProcessFailed("approval request lock poisoned".to_string())
            })?
            .push(request);
        Ok(())
    }

    fn resolve_approval(&self, resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        if self.fail_approval_resolution.load(Ordering::SeqCst) {
            return Err(ExecutionError::ToolFailed(
                "approval resolution sink failure".to_string(),
            ));
        }
        self.approval_resolutions
            .lock()
            .map_err(|_| {
                ExecutionError::ProcessFailed("approval resolution lock poisoned".to_string())
            })?
            .push(resolution);
        Ok(())
    }

    fn record_artifact(
        &self,
        kind: ArtifactKind,
        storage_path: &str,
    ) -> Result<(), ExecutionError> {
        self.artifacts
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("artifact lock poisoned".to_string()))?
            .push((kind, storage_path.to_string()));
        Ok(())
    }

    fn complete(&self, detail: &str) -> Result<(), ExecutionError> {
        self.completed
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("complete lock poisoned".to_string()))?
            .push(detail.to_string());
        Ok(())
    }

    fn fail(&self, error: ExecutionError) -> Result<(), ExecutionError> {
        self.failed
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("fail lock poisoned".to_string()))?
            .push(error);
        Ok(())
    }
}

#[derive(Default)]
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo input."
    }

    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":true})
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: None,
            read_only: true,
            parallel_safe: true,
        }
    }

    async fn run(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        Ok(ToolOutput { content: input })
    }
}

pub struct CountingTool {
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
}

impl CountingTool {
    pub fn new(active: Arc<AtomicUsize>, max_active: Arc<AtomicUsize>) -> Self {
        Self { active, max_active }
    }
}

#[async_trait]
impl Tool for CountingTool {
    fn name(&self) -> &'static str {
        "count"
    }

    fn description(&self) -> &str {
        "Count concurrent calls."
    }

    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":true})
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: None,
            read_only: true,
            parallel_safe: true,
        }
    }

    async fn run(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(25)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(ToolOutput { content: input })
    }
}

pub struct UnsafeCountingTool {
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
}

impl UnsafeCountingTool {
    pub fn new(active: Arc<AtomicUsize>, max_active: Arc<AtomicUsize>) -> Self {
        Self { active, max_active }
    }
}

#[async_trait]
impl Tool for UnsafeCountingTool {
    fn name(&self) -> &'static str {
        "unsafe_count"
    }

    fn description(&self) -> &str {
        "Count non-parallel-safe calls."
    }

    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":true})
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: None,
            read_only: false,
            parallel_safe: false,
        }
    }

    async fn run(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(25)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(ToolOutput { content: input })
    }
}

#[derive(Default)]
pub struct DelayTool;

#[async_trait]
impl Tool for DelayTool {
    fn name(&self) -> &'static str {
        "delay"
    }

    fn description(&self) -> &str {
        "Delay for a requested duration."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type":"object",
            "properties":{"delay_ms":{"type":"integer"}},
            "additionalProperties":true
        })
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: None,
            read_only: true,
            parallel_safe: true,
        }
    }

    async fn run(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let delay_ms = input
            .get("delay_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        Ok(ToolOutput { content: input })
    }
}

#[derive(Default)]
pub struct ApprovalTool;

#[async_trait]
impl Tool for ApprovalTool {
    fn name(&self) -> &'static str {
        "approval_tool"
    }

    fn description(&self) -> &str {
        "Requires approval."
    }

    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":true})
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: Some(ta_protocol::wire::ApprovalScope::ProcessExec),
            read_only: false,
            parallel_safe: false,
        }
    }

    async fn run(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        Ok(ToolOutput { content: input })
    }
}

pub struct FailingTool;

#[async_trait]
impl Tool for FailingTool {
    fn name(&self) -> &'static str {
        "fail"
    }

    fn description(&self) -> &str {
        "Always fails."
    }

    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":true})
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: None,
            read_only: true,
            parallel_safe: true,
        }
    }

    async fn run(&self, _input: Value, _ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        Err(ExecutionError::ToolFailed(
            "intentional failure".to_string(),
        ))
    }
}

pub fn request() -> ExecutionRequest {
    ExecutionRequest {
        session_id: must(SessionId::new("session-test")),
        run_id: must(ta_protocol::wire::RunId::new("run-test")),
        runtime_profile_id: must(RuntimeProfileId::new("runtime-openai-safe")),
        provider_id: must(ta_protocol::wire::AgentRuntimeStrategyId::new("openai")),
        execution_harness: taugentic_agent::AgentExecutionHarness::NativeLoop,
        system_prompt: None,
        objective: "test objective".to_string(),
        model_id: Some(must(AgentRuntimeModelId::new("test-model"))),
        auth_profile_id: Some(must(AuthProfileId::new("auth-test"))),
        resume_provider_session_id: None,
        runtime_extensions: Vec::new(),
        execution_context: Arc::new(test_execution_context(Path::new("/tmp"))),
        fork_initial_state: None,
        output_contract: None,
        subagent_recipes: Vec::new(),
    }
}

pub fn configure_codex_app_server_request(request: &mut ExecutionRequest) {
    request.runtime_profile_id = must(ta_protocol::wire::RuntimeProfileId::new(
        "runtime-codex-api-key",
    ));
    request.provider_id = must(ta_protocol::wire::AgentRuntimeStrategyId::new("codex"));
    request.execution_harness = taugentic_agent::AgentExecutionHarness::CodexAppServer;
    request.auth_profile_id = None;
    set_request_cwd(request, Path::new("/tmp"));
}

pub fn set_request_cwd(request: &mut ExecutionRequest, cwd: &Path) {
    let cwd = workspace_path(cwd);
    let context = Arc::make_mut(&mut request.execution_context);
    context.workspace_root = cwd.clone();
    context.effective_cwd = cwd.clone();
    context.workspace_scope = WorkspaceScope::Local { root: cwd.clone() };
    context.sandbox_profile.read_roots = vec![cwd.clone()];
    context.sandbox_profile.write_roots = vec![cwd];
}

pub fn set_request_artifact_root(request: &mut ExecutionRequest, artifact_root: &Path) {
    let artifact_root = workspace_path(artifact_root);
    let context = Arc::make_mut(&mut request.execution_context);
    context.artifact_root = artifact_root.clone();
    if !context.sandbox_profile.write_roots.contains(&artifact_root) {
        context.sandbox_profile.write_roots.push(artifact_root);
    }
}

fn test_execution_context(cwd: &Path) -> ExecutionContext {
    let root = workspace_path(cwd);
    ExecutionContext {
        workspace_id: WorkspaceId::new("workspace-test").expect("workspace id"),
        workspace_root: root.clone(),
        effective_cwd: root.clone(),
        artifact_root: WorkspacePath::from_canonical_wire_value("/tmp/taugentic-agent-artifacts")
            .expect("artifact root"),
        workspace_scope: WorkspaceScope::Local { root: root.clone() },
        sandbox_profile: SandboxProfile {
            read_roots: vec![root.clone()],
            write_roots: vec![root.clone()],
            denied_roots: Vec::new(),
            process_exec: ProcessExecPolicy::AllowAll,
        },
        permission_policy: PermissionPolicy::Unrestricted,
        network_policy: NetworkPolicy::Open,
        env_policy: EnvPolicy::workspace_default(),
    }
}

fn workspace_path(path: &Path) -> WorkspacePath {
    WorkspacePath::from_canonical_wire_value(path.to_string_lossy().into_owned())
        .expect("test path must be absolute and canonical")
}

pub fn sandbox_safe_temp_dir(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in("/tmp")
        .expect("sandbox-safe temp dir")
}

pub fn configure_codex_acp_request(request: &mut ExecutionRequest) {
    request.runtime_profile_id = must(ta_protocol::wire::RuntimeProfileId::new(
        "runtime-codex-acp-safe",
    ));
    request.provider_id = must(ta_protocol::wire::AgentRuntimeStrategyId::new("codex-acp"));
    request.execution_harness = taugentic_agent::AgentExecutionHarness::Acp {
        provider: ta_provider_acp::descriptor::AcpProviderSpec::from_builtin(
            ta_provider_acp::descriptor::AcpLaunchKind::Codex,
        ),
    };
}

pub fn registry_with_echo() -> Registry {
    let mut registry = Registry::new();
    let _ = registry.add(EchoTool);
    registry
}

pub fn run_loop(
    client: Arc<dyn LlmClient>,
    registry: Registry,
    queues: MessageQueue,
    session: Session,
    cancellation: CancellationToken,
    sink: Arc<TestSink>,
) -> TurnLoop {
    let request = request();
    run_loop_with_request(
        request,
        client,
        registry,
        queues,
        session,
        cancellation,
        sink,
    )
}

pub fn run_loop_with_request(
    request: ExecutionRequest,
    client: Arc<dyn LlmClient>,
    registry: Registry,
    queues: MessageQueue,
    session: Session,
    cancellation: CancellationToken,
    sink: Arc<TestSink>,
) -> TurnLoop {
    let approval_bridge = Arc::new(ApprovalBridge::new(
        request.run_id.clone(),
        sink.clone(),
        cancellation.clone(),
    ));
    let context = LoopApprovalContext {
        request,
        session,
        cancellation,
        sink,
        approval_bridge,
    };
    run_loop_with_bridge(client, registry, queues, context)
}

#[derive(Clone)]
pub struct LoopApprovalContext {
    pub request: ExecutionRequest,
    pub session: Session,
    pub cancellation: CancellationToken,
    pub sink: Arc<TestSink>,
    pub approval_bridge: Arc<ApprovalBridge>,
}

impl LoopApprovalContext {
    pub fn new(
        request: ExecutionRequest,
        sink: Arc<TestSink>,
        cancellation: CancellationToken,
    ) -> Self {
        let session = Session::new(&request);
        let approval_bridge = Arc::new(ApprovalBridge::new(
            request.run_id.clone(),
            sink.clone(),
            cancellation.clone(),
        ));
        Self {
            request,
            session,
            cancellation,
            sink,
            approval_bridge,
        }
    }
}

pub fn run_loop_with_bridge(
    client: Arc<dyn LlmClient>,
    registry: Registry,
    queues: MessageQueue,
    context: LoopApprovalContext,
) -> TurnLoop {
    context
        .session
        .attach_approval_bridge(context.approval_bridge.clone())
        .expect("test session approval bridge should attach");
    let artifact_writer = Arc::new(
        ArtifactWriter::new(
            context.request.execution_context.artifact_root.as_path(),
            context.request.run_id.clone(),
        )
        .expect("test artifact writer should initialize"),
    );
    TurnLoop::from_config(TurnLoopConfig {
        request: context.request,
        sink: context.sink,
        client,
        registry,
        queues,
        session: context.session,
        cancellation: context.cancellation,
        approval_bridge: context.approval_bridge,
        artifact_writer,
    })
}

pub fn end_turn() -> Vec<Result<StreamEvent, LlmClientError>> {
    vec![Ok(StreamEvent::TurnCompleted {
        stop_reason: StopReason::EndTurn,
        provider_session_id: None,
    })]
}

pub fn max_tokens_turn() -> Vec<Result<StreamEvent, LlmClientError>> {
    vec![Ok(StreamEvent::TurnCompleted {
        stop_reason: StopReason::MaxTokens,
        provider_session_id: None,
    })]
}

pub fn tool_turn(name: &str, count: usize) -> Vec<Result<StreamEvent, LlmClientError>> {
    let calls = (0..count)
        .map(|index| (name.to_string(), format!(r#"{{"n":{index}}}"#)))
        .collect::<Vec<_>>();
    tool_turn_sequence(calls)
}

pub fn tool_turn_sequence(
    calls: Vec<(String, String)>,
) -> Vec<Result<StreamEvent, LlmClientError>> {
    let mut events = Vec::new();
    for (index, (name, input)) in calls.into_iter().enumerate() {
        events.push(Ok(StreamEvent::ToolCallStarted {
            id: format!("tool-call-{index}"),
            index: index as u64,
            name,
        }));
        events.push(Ok(StreamEvent::ToolInputDelta {
            id: format!("tool-call-{index}"),
            index: index as u64,
            delta: input,
        }));
        events.push(Ok(StreamEvent::ToolCallCompleted {
            id: format!("tool-call-{index}"),
            index: index as u64,
        }));
    }
    events.push(Ok(StreamEvent::ToolCallBatchCompleted));
    events
}

pub fn user_message(content: &str) -> StreamMessage {
    StreamMessage::user(content.to_string())
}

pub async fn wait_for_approval_request(sink: &TestSink) -> ApprovalRequest {
    for _ in 0..100 {
        if let Some(request) = sink.approval_requests().into_iter().next() {
            return request;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("approval request was not emitted");
}

pub async fn resolve_first_approval(
    sink: &TestSink,
    bridge: &ApprovalBridge,
    outcome: ApprovalOutcome,
) -> ApprovalRequest {
    let request = wait_for_approval_request(sink).await;
    bridge.resolve(request.id.clone(), outcome);
    request
}

fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("invalid test fixture: {error}"),
    }
}
