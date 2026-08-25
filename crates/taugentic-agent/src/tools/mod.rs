pub mod apply_patch;
pub mod descriptor;
pub mod list_directory;
pub mod mcp;
pub mod read_file;
pub mod search;
pub mod shell;
pub mod subagent;
pub mod subagent_description;

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use ta_protocol::wire::{AgentStreamTurnId, ExecutionContext};

use crate::ExecutionError;

pub use apply_patch::{ApplyPatchResult, ApplyPatchTool};
pub use descriptor::ToolDescriptor;
pub use list_directory::ListDirectoryTool;
pub use mcp::McpTool;
pub use read_file::ReadFileTool;
pub use search::SearchTool;
pub use shell::{ShellResult, ShellTool, TruncatedBy};
pub use subagent::SubagentTool;
pub use subagent_description::render_subagent_tool_description;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub execution_context: Arc<ExecutionContext>,
    pub cancellation_token: CancellationToken,
    pub timeout: Duration,
    pub parent_turn_id: Option<AgentStreamTurnId>,
}

impl ToolContext {
    pub fn new(execution_context: Arc<ExecutionContext>) -> Self {
        Self {
            execution_context,
            cancellation_token: CancellationToken::new(),
            timeout: Duration::from_secs(10),
            parent_turn_id: None,
        }
    }

    pub fn workdir(&self) -> &Path {
        self.execution_context.effective_cwd.as_path()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    pub content: Value,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn descriptor(&self) -> ToolDescriptor;
    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError>;
}

#[derive(Default)]
pub struct Registry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    locked: bool,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_read_only_builtins() -> Self {
        let mut registry = Self::new();
        registry.add_builtin(ReadFileTool);
        registry.add_builtin(ListDirectoryTool);
        registry.add_builtin(SearchTool);
        registry
    }

    pub fn with_all_builtins() -> Self {
        let mut registry = Self::with_read_only_builtins();
        registry.add_builtin(ShellTool);
        registry.add_builtin(ApplyPatchTool);
        registry
    }

    pub fn add<T>(&mut self, tool: T) -> Result<(), ExecutionError>
    where
        T: Tool + 'static,
    {
        if self.locked {
            return Err(ExecutionError::ToolListLocked(format!(
                "cannot add tool {} after first API call",
                tool.name()
            )));
        }
        self.insert_tool(tool);
        Ok(())
    }

    pub fn lock_tool_list(&mut self) -> Vec<ToolDescriptor> {
        self.locked = true;
        self.descriptors()
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn Tool>)> {
        self.tools.iter().map(|(name, tool)| (name.as_str(), tool))
    }

    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.iter().map(|(_, tool)| tool.descriptor()).collect()
    }

    pub(crate) fn clone_tools(&self) -> BTreeMap<String, Arc<dyn Tool>> {
        self.tools.clone()
    }

    fn add_builtin<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        self.insert_tool(tool);
    }

    fn insert_tool<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }
}

pub(crate) fn resolve_workdir_path(
    workdir: &Path,
    input_path: &str,
) -> Result<PathBuf, ExecutionError> {
    let relative = PathBuf::from(input_path);
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(ExecutionError::InvalidToolInput(
            "path must be a non-empty relative path".to_string(),
        ));
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ExecutionError::InvalidToolInput(
            "path must stay inside the workdir".to_string(),
        ));
    }
    Ok(workdir.join(relative))
}

pub(crate) fn relative_display(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
