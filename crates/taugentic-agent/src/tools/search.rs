use std::{
    fs,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use ignore::WalkBuilder;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ExecutionError;
use crate::tools::{
    Tool, ToolContext, ToolDescriptor, ToolOutput, relative_display, resolve_workdir_path,
};

const DEFAULT_RESULT_LIMIT: usize = 100;

#[derive(Default)]
pub struct SearchTool;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchInput {
    query: String,
    mode: SearchMode,
    #[serde(default = "default_path")]
    path: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SearchMode {
    Content,
    FileName,
}

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &str {
        "Search file contents with ripgrep or filenames with a gitignore-aware walker."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query", "mode"],
            "properties": {
                "query": { "type": "string" },
                "mode": { "type": "string", "enum": ["content", "fileName"] },
                "path": { "type": "string" },
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
        let input: SearchInput = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        let limit = input.limit.unwrap_or(DEFAULT_RESULT_LIMIT);
        let root = resolve_workdir_path(&ctx.workdir, &input.path)?;
        match input.mode {
            SearchMode::Content => search_content(&input.query, &root, &ctx, limit).await,
            SearchMode::FileName => search_filename(&input.query, &root, &ctx, limit),
        }
    }
}

async fn search_content(
    query: &str,
    root: &std::path::Path,
    ctx: &ToolContext,
    limit: usize,
) -> Result<ToolOutput, ExecutionError> {
    let timeout = if ctx.timeout.is_zero() {
        Duration::from_secs(10)
    } else {
        ctx.timeout
    };
    let started = Instant::now();
    let mut results = Vec::new();

    for entry in WalkBuilder::new(root)
        .standard_filters(true)
        .require_git(false)
        .hidden(false)
        .build()
    {
        if ctx.cancellation_token.is_cancelled() {
            return Err(ExecutionError::Cancelled("search cancelled".to_string()));
        }
        if started.elapsed() > timeout {
            return Err(ExecutionError::ProcessTimeout {
                timeout_ms: duration_millis(timeout),
                detail: "search exceeded timeout".to_string(),
            });
        }

        let entry = entry.map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }

        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => continue,
            Err(error) => {
                return Err(ExecutionError::ToolFailed(format!(
                    "{}: {error}",
                    relative_display(path, &ctx.workdir)
                )));
            }
        };

        for (line_index, line) in content.lines().enumerate() {
            let Some(column) = line.find(query) else {
                continue;
            };
            results.push(json!({
                "line": format!(
                    "{}:{}:{}:{}",
                    relative_display(path, &ctx.workdir),
                    line_index + 1,
                    column + 1,
                    line
                )
            }));
            if results.len() >= limit {
                return Ok(content_search_output(query, results));
            }
        }
    }

    Ok(content_search_output(query, results))
}

fn search_filename(
    query: &str,
    root: &std::path::Path,
    ctx: &ToolContext,
    limit: usize,
) -> Result<ToolOutput, ExecutionError> {
    let needle = query.to_lowercase();
    let mut results = Vec::new();
    for entry in WalkBuilder::new(root)
        .standard_filters(true)
        .require_git(false)
        .hidden(false)
        .build()
    {
        let entry = entry.map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        if results.len() >= limit {
            break;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.to_lowercase().contains(&needle) {
            results.push(json!({
                "path": relative_display(path, &ctx.workdir),
                "type": if path.is_dir() { "directory" } else { "file" }
            }));
        }
    }
    Ok(ToolOutput {
        content: json!({ "mode": "fileName", "query": query, "results": results }),
    })
}

fn default_path() -> String {
    ".".to_string()
}

fn content_search_output(query: &str, results: Vec<Value>) -> ToolOutput {
    ToolOutput {
        content: json!({ "mode": "content", "query": query, "results": results }),
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
