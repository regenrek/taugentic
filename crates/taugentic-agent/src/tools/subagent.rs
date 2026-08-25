use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use ta_protocol::wire::{CapsuleRecipe, DelegateRequest, RunId, RunStatus};

use crate::tools::subagent_description::render_subagent_tool_description;
use crate::tools::{Tool, ToolContext, ToolDescriptor, ToolOutput};
use crate::{ExecutionError, ExecutionSink, NativeChildRunRequest};

pub struct SubagentTool {
    parent_run_id: RunId,
    sink: Arc<dyn ExecutionSink>,
    description: String,
}

impl SubagentTool {
    pub fn new(
        parent_run_id: RunId,
        sink: Arc<dyn ExecutionSink>,
        recipes: Vec<CapsuleRecipe>,
    ) -> Self {
        Self {
            parent_run_id,
            sink,
            description: render_subagent_tool_description(&recipes),
        }
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &'static str {
        "subagent"
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["objective"],
            "properties": {
                "objective": { "type": "string" },
                "outputContract": {
                    "type": "string",
                    "enum": ["debug", "patch", "review", "test", "plan", "custom"]
                },
                "modelId": { "type": "string" },
                "recipeId": { "type": "string" },
                "workspaceScope": {
                    "type": "string",
                    "enum": ["readonly", "workspaceWrite", "worktreeWrite", "repoWriteWithApproval", "remoteWorker", "containerized", "ephemeral"],
                    "default": "worktreeWrite"
                },
                "cleanupPolicy": {
                    "type": "string",
                    "enum": ["deleteOnSuccess", "deleteOnTerminal", "keep", "manual"],
                    "default": "deleteOnSuccess"
                },
                "plannedWriteFiles": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": []
                }
            },
            "additionalProperties": false
        })
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

    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let input: DelegateRequest = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        let parent_turn_id = ctx.parent_turn_id.ok_or_else(|| {
            ExecutionError::InvalidToolInput(
                "subagent tool requires a parent turn id from the native turn loop".to_string(),
            )
        })?;
        let result = self.sink.start_native_child_run(
            NativeChildRunRequest::new(
                self.parent_run_id.clone(),
                parent_turn_id,
                input.objective,
                input.output_contract,
                input.model_id,
                input.recipe_id,
            )?
            .with_workspace_scope(input.workspace_scope)
            .with_cleanup_policy(input.cleanup_policy)
            .with_planned_write_files(input.planned_write_files),
        )?;

        Ok(ToolOutput {
            content: json!({
                "runId": result.run_id,
                "status": run_status_json(result.status),
            }),
        })
    }
}

fn run_status_json(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::WaitingForApproval => "waitingForApproval",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::BudgetExceeded => "budgetExceeded",
        RunStatus::Cancelled => "cancelled",
    }
}
