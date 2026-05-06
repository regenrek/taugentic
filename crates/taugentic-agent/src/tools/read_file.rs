use std::fs;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ExecutionError;
use crate::tools::{Tool, ToolContext, ToolDescriptor, ToolOutput, resolve_workdir_path};

const DEFAULT_LIMIT: usize = 2_000;

#[derive(Default)]
pub struct ReadFileTool;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadFileInput {
    path: String,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a UTF-8 file with optional line offset and limit."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" },
                "offset": { "type": "integer", "minimum": 0 },
                "limit": { "type": "integer", "minimum": 1 }
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
        let input: ReadFileInput = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        let path = resolve_workdir_path(&ctx.workdir, &input.path)?;
        let content = fs::read_to_string(&path)
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
        let lines: Vec<&str> = content.lines().collect();
        let selected = lines.iter().enumerate().skip(input.offset).take(limit);
        let annotated = selected
            .map(|(index, line)| format!("{:>6}\t{line}", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let end = (input.offset + limit).min(lines.len());

        Ok(ToolOutput {
            content: json!({
                "path": input.path,
                "offset": input.offset,
                "limit": limit,
                "startLine": input.offset + 1,
                "endLine": end,
                "totalLines": lines.len(),
                "truncated": end < lines.len(),
                "content": annotated
            }),
        })
    }
}
