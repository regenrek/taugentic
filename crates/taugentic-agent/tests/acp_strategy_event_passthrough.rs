mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    AgentStreamFrame, AgentToolCallOutcome, ApprovalRequest, ApprovalResolution, ArtifactKind,
    StreamEmission,
};
use ta_provider_acp::{
    adapter::{AcpProcessConfig, DEFAULT_CANCEL_GRACE},
    descriptor::{AcpLaunchKind, AcpProviderSpec},
    launch::build_perimeter_profile,
};
use taugentic_agent::execution_strategy::acp::dispatch_with_config;
use taugentic_agent::{ExecutionError, ExecutionSink};

#[test]
fn acp_strategy_passes_adapter_events_to_sink_in_order() {
    let dir = unique_dir("acp-strategy-event-passthrough");
    fs::create_dir_all(&dir).expect("dir");
    let script = dir.join("mock-acp.py");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}), flush=True)
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1"}}), flush=True)
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hello"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call","toolCallId":"t1","title":"read_file"}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call_update","toolCallId":"t1","fields":{"status":"completed","rawOutput":"done"}}}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}), flush=True)
"#,
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");

    let mut request = support::request();
    support::configure_codex_acp_request(&mut request);
    support::set_request_cwd(&mut request, &dir);
    let (completed_tx, completed_rx) = mpsc::channel();
    let sink = RecordingSink::new(completed_tx);
    let handle = dispatch_with_config(
        request,
        sink.clone(),
        AcpProcessConfig {
            flavor_id: "codex-acp".to_string(),
            command: script.clone(),
            sandbox_profile: test_perimeter_profile(&dir, &script),
            args: Vec::new(),
            env: Vec::new(),
            env_remove: Vec::new(),
            work_dir: dir,
            mcp_servers: Vec::new(),
            session_mode_id: None,
            session_model_id: None,
            cancel_grace: DEFAULT_CANCEL_GRACE,
        },
    )
    .expect("handle");

    completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("ACP execution should complete");
    drop(handle);
    let emissions = sink.stream_frames();
    let frames = emissions
        .iter()
        .map(|emission| emission.frame.clone())
        .collect::<Vec<_>>();
    let turn_ids = emissions
        .iter()
        .map(|emission| {
            emission
                .turn_id
                .as_ref()
                .expect("ACP stream emissions carry a turn id")
                .as_str()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        frames,
        vec![
            AgentStreamFrame::AssistantTurnStarted,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "hello".to_string()
            },
            AgentStreamFrame::ToolCallStarted {
                tool_name: "read_file".to_string(),
                input: "null".to_string()
            },
            AgentStreamFrame::ToolCallProgressed {
                delta: "done".to_string()
            },
            AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed
            },
            AgentStreamFrame::AssistantTurnCompleted
        ]
    );
    assert!(turn_ids.iter().all(|turn_id| turn_id == "acp-prompt-3"));
    assert!(
        emissions
            .iter()
            .find(|emission| matches!(
                emission.frame,
                AgentStreamFrame::AssistantMessageDelta { .. }
            ))
            .and_then(|emission| emission.item_id.as_ref())
            .is_none()
    );
}

#[test]
fn acp_strategy_treats_missing_set_model_method_as_soft_activity() {
    let dir = unique_dir("acp-strategy-set-model-unsupported");
    fs::create_dir_all(&dir).expect("dir");
    let script = dir.join("mock-acp.py");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}), flush=True)
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","models":{"currentModelId":"provider-default","availableModels":[{"modelId":"provider-default","name":"Default"},{"modelId":"test-model","name":"Test"}]}}}), flush=True)
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"method not found"}}), flush=True)
sys.stdin.readline()
print(json.dumps({"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}), flush=True)
"#,
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");

    let mut request = support::request();
    support::configure_codex_acp_request(&mut request);
    support::set_request_cwd(&mut request, &dir);
    let (completed_tx, completed_rx) = mpsc::channel();
    let sink = RecordingSink::new(completed_tx);
    let handle = dispatch_with_config(
        request,
        sink.clone(),
        AcpProcessConfig {
            flavor_id: "codex-acp".to_string(),
            command: script.clone(),
            sandbox_profile: test_perimeter_profile(&dir, &script),
            args: Vec::new(),
            env: Vec::new(),
            env_remove: Vec::new(),
            work_dir: dir,
            mcp_servers: Vec::new(),
            session_mode_id: None,
            session_model_id: Some("test-model".to_string()),
            cancel_grace: DEFAULT_CANCEL_GRACE,
        },
    )
    .expect("handle");

    completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("ACP execution should complete");
    drop(handle);

    assert_eq!(
        sink.activities(),
        vec![
            "ACP provider does not support session/set_model; using provider-selected model"
                .to_string()
        ]
    );
}

fn test_perimeter_profile(
    work_dir: &std::path::Path,
    command: &std::path::Path,
) -> ta_exec::SandboxProfile {
    let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
    build_perimeter_profile(
        &provider,
        &support::test_execution_context(work_dir),
        command,
    )
    .expect("test ACP perimeter profile")
}

fn unique_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{suffix}"))
}

struct RecordingSink {
    streams: Mutex<Vec<StreamEmission>>,
    activities: Mutex<Vec<String>>,
    completed_tx: Mutex<Option<Sender<()>>>,
}

impl RecordingSink {
    fn new(completed_tx: Sender<()>) -> Arc<Self> {
        Arc::new(Self {
            streams: Mutex::new(Vec::new()),
            activities: Mutex::new(Vec::new()),
            completed_tx: Mutex::new(Some(completed_tx)),
        })
    }

    fn stream_frames(&self) -> Vec<StreamEmission> {
        self.streams.lock().expect("streams").clone()
    }

    fn activities(&self) -> Vec<String> {
        self.activities.lock().expect("activities").clone()
    }
}

impl ExecutionSink for RecordingSink {
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

    fn request_approval(&self, _request: ApprovalRequest) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn resolve_approval(&self, _resolution: ApprovalResolution) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn record_artifact(
        &self,
        _kind: ArtifactKind,
        _storage_path: &str,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn record_image_artifact(
        &self,
        _: ta_protocol::wire::AgentStreamTurnId,
        _: ta_protocol::wire::AgentStreamItemId,
        _: &str,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn complete(&self, _detail: &str) -> Result<(), ExecutionError> {
        if let Some(sender) = self
            .completed_tx
            .lock()
            .map_err(|_| ExecutionError::ProcessFailed("complete lock poisoned".to_string()))?
            .take()
        {
            let _ = sender.send(());
        }
        Ok(())
    }

    fn fail(&self, error: ExecutionError) -> Result<(), ExecutionError> {
        Err(error)
    }
}
