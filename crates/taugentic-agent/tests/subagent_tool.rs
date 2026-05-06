use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use ta_protocol::wire::{
    AgentStreamTurnId, ApprovalRequest, ApprovalResolution, ArtifactKind, OutputContractKind,
    RunId, RunStatus, StreamEmission,
};
use taugentic_agent::tools::{SubagentTool, Tool, ToolContext};
use taugentic_agent::{ExecutionError, ExecutionSink, NativeChildRunRequest, NativeChildRunResult};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn subagent_tool_delegates_child_run_start_to_sink() {
    let parent_run_id = RunId::new("run-parent").expect("parent run id");
    let parent_turn_id = AgentStreamTurnId::new("turn-parent").expect("parent turn id");
    let sink = Arc::new(RecordingSink::default());
    let tool = SubagentTool::new(parent_run_id.clone(), sink.clone(), Vec::new());

    let output = tool
        .run(
            json!({ "objective": "Inspect focused files" }),
            ToolContext {
                workdir: ".".into(),
                cancellation_token: CancellationToken::new(),
                timeout: Duration::from_secs(1),
                parent_turn_id: Some(parent_turn_id.clone()),
            },
        )
        .await
        .expect("subagent tool should start child run through sink");

    assert_eq!(
        sink.request
            .lock()
            .expect("request should not poison")
            .clone(),
        Some(NativeChildRunRequest {
            parent_run_id,
            parent_turn_id,
            objective: "Inspect focused files".to_string(),
            output_contract: None,
            model_id: None,
            sandbox_profile: None,
            recipe_id: None,
            workspace_scope: Default::default(),
            cleanup_policy: Default::default(),
            planned_write_files: Vec::new(),
        })
    );
    assert_eq!(
        output.content,
        json!({
            "runId": "run-child",
            "status": "queued"
        })
    );
}

#[tokio::test]
async fn subagent_tool_passes_output_contract_to_child_request() {
    let parent_run_id = RunId::new("run-parent").expect("parent run id");
    let parent_turn_id = AgentStreamTurnId::new("turn-parent").expect("parent turn id");
    let sink = Arc::new(RecordingSink::default());
    let tool = SubagentTool::new(parent_run_id.clone(), sink.clone(), Vec::new());

    tool.run(
        json!({
            "objective": "Produce patch result",
            "outputContract": "patch"
        }),
        ToolContext {
            workdir: ".".into(),
            cancellation_token: CancellationToken::new(),
            timeout: Duration::from_secs(1),
            parent_turn_id: Some(parent_turn_id.clone()),
        },
    )
    .await
    .expect("subagent tool should accept output contract");

    assert_eq!(
        sink.request
            .lock()
            .expect("request should not poison")
            .as_ref()
            .map(|request| request.output_contract),
        Some(Some(OutputContractKind::Patch))
    );
}

#[derive(Default)]
struct RecordingSink {
    request: Mutex<Option<NativeChildRunRequest>>,
}

impl ExecutionSink for RecordingSink {
    fn push_stream(&self, _: StreamEmission) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn record_token_usage(
        &self,
        _: ta_provider_llm::client::LlmTokenUsage,
    ) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn push_activity(&self, _: &str) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn push_provider_session_id(&self, _: String) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn request_approval(&self, _: ApprovalRequest) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn resolve_approval(&self, _: ApprovalResolution) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn record_artifact(&self, _: ArtifactKind, _: &str) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn start_native_child_run(
        &self,
        request: NativeChildRunRequest,
    ) -> Result<NativeChildRunResult, ExecutionError> {
        *self.request.lock().expect("request should not poison") = Some(request);
        Ok(NativeChildRunResult {
            run_id: RunId::new("run-child").expect("child run id"),
            status: RunStatus::Queued,
        })
    }

    fn complete(&self, _: &str) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn fail(&self, _: ExecutionError) -> Result<(), ExecutionError> {
        Ok(())
    }
}
