use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ta_protocol::wire::ApprovalScope;

use crate::ExecutionError;
use crate::patch::{
    ApplyError, FileChangeKind, FileOp, apply_patch, parse_patch, write_applied_patch,
};
use crate::tools::{Tool, ToolContext, ToolDescriptor, ToolOutput};

#[derive(Default)]
pub struct ApplyPatchTool;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyPatchInput {
    input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyPatchResult {
    pub files_added: Vec<PathBuf>,
    pub files_modified: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub files_moved: Vec<(PathBuf, PathBuf)>,
    pub diff_text: String,
    pub bytes_written: u64,
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a Codex-style patch to files under the current workdir."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["input"],
            "properties": {
                "input": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: Some(ApprovalScope::FileWrite),
            read_only: false,
            parallel_safe: false,
        }
    }

    #[tracing::instrument(
        name = "tool.apply_patch.run",
        skip_all,
        fields(tool = "apply_patch", workdir = %ctx.workdir().display())
    )]
    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        if ctx.cancellation_token.is_cancelled() {
            return Err(ExecutionError::Cancelled(
                "apply_patch cancelled before execution".to_string(),
            ));
        }

        let input: ApplyPatchInput = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        let patch = parse_patch(&input.input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        ensure_patch_contained(&patch.operations, ctx.workdir())
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;

        if ctx.cancellation_token.is_cancelled() {
            return Err(ExecutionError::Cancelled(
                "apply_patch cancelled before write".to_string(),
            ));
        }

        let applied = apply_patch(&patch, ctx.workdir())
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        crate::patch::writer::validate_changes(&applied, ctx.workdir())
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;

        if ctx.cancellation_token.is_cancelled() {
            return Err(ExecutionError::Cancelled(
                "apply_patch cancelled before write".to_string(),
            ));
        }

        let report = write_applied_patch(&applied, ctx.workdir())
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        let result = ApplyPatchResult {
            files_added: applied
                .changed_files
                .iter()
                .filter(|change| change.kind == FileChangeKind::Added)
                .map(|change| change.path.clone())
                .collect(),
            files_modified: applied
                .changed_files
                .iter()
                .filter(|change| change.kind == FileChangeKind::Updated && change.move_to.is_none())
                .map(|change| change.path.clone())
                .collect(),
            files_deleted: applied
                .changed_files
                .iter()
                .filter(|change| change.kind == FileChangeKind::Deleted)
                .map(|change| change.path.clone())
                .collect(),
            files_moved: applied
                .changed_files
                .iter()
                .filter_map(|change| {
                    change
                        .move_to
                        .as_ref()
                        .map(|move_to| (change.path.clone(), move_to.clone()))
                })
                .collect(),
            diff_text: applied.diff,
            bytes_written: report.bytes_written,
        };

        Ok(ToolOutput {
            content: serde_json::to_value(result)
                .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?,
        })
    }
}

fn ensure_patch_contained(
    operations: &[FileOp],
    workdir: &std::path::Path,
) -> Result<(), ApplyError> {
    let workdir = workdir.canonicalize().map_err(|_| ApplyError::PathEscape {
        path: workdir.to_path_buf(),
    })?;
    for operation in operations {
        match operation {
            FileOp::AddFile { path, .. } | FileOp::DeleteFile { path } => {
                crate::patch::writer::ensure_contained(&workdir, path)
                    .map_err(|_| ApplyError::PathEscape { path: path.clone() })?;
            }
            FileOp::UpdateFile { path, move_to, .. } => {
                crate::patch::writer::ensure_contained(&workdir, path)
                    .map_err(|_| ApplyError::PathEscape { path: path.clone() })?;
                if let Some(move_to) = move_to {
                    crate::patch::writer::ensure_contained(&workdir, move_to).map_err(|_| {
                        ApplyError::PathEscape {
                            path: move_to.clone(),
                        }
                    })?;
                }
            }
        }
    }
    Ok(())
}
