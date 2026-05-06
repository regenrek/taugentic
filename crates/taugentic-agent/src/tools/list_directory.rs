use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ExecutionError;
use crate::tools::{
    Tool, ToolContext, ToolDescriptor, ToolOutput, relative_display, resolve_workdir_path,
};

#[derive(Default)]
pub struct ListDirectoryTool;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListDirectoryInput {
    path: String,
    #[serde(default)]
    recursive: bool,
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List directory entries with entry type and size metadata."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" },
                "recursive": { "type": "boolean" }
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
            read_only: true,
            parallel_safe: true,
        }
    }

    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let input: ListDirectoryInput = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        let root = resolve_workdir_path(&ctx.workdir, &input.path)?;
        let mut entries = Vec::new();
        collect_entries(&root, &ctx.workdir, input.recursive, &mut entries)?;
        entries.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
        Ok(ToolOutput {
            content: json!({ "path": input.path, "recursive": input.recursive, "entries": entries }),
        })
    }
}

fn collect_entries(
    root: &std::path::Path,
    workdir: &std::path::Path,
    recursive: bool,
    entries: &mut Vec<Value>,
) -> Result<(), ExecutionError> {
    for entry in
        std::fs::read_dir(root).map_err(|error| ExecutionError::ToolFailed(error.to_string()))?
    {
        let entry = entry.map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        let file_type = if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else if metadata.file_type().is_symlink() {
            "symlink"
        } else {
            "other"
        };
        entries.push(json!({
            "path": relative_display(&path, workdir),
            "type": file_type,
            "size": metadata.len()
        }));
        if recursive && metadata.is_dir() {
            collect_entries(&path, workdir, recursive, entries)?;
        }
    }
    Ok(())
}
