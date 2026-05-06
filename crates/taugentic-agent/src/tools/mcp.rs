use async_trait::async_trait;
use serde_json::Value;
use ta_protocol::wire::ApprovalScope;

use crate::mcp::client::McpClient;
use crate::{ExecutionError, tools::ToolContext};

use super::{Tool, ToolDescriptor, ToolOutput};

#[derive(Clone)]
pub struct McpTool {
    registered_name: &'static str,
    description_static: &'static str,
    remote_name: String,
    description: String,
    input_schema: Value,
    approval_scope: Option<ApprovalScope>,
    client: McpClient,
}

impl McpTool {
    pub fn new(
        registered_name: String,
        remote_name: String,
        description: String,
        input_schema: Value,
        approval_scope: Option<ApprovalScope>,
        client: McpClient,
    ) -> Self {
        let description_static = Box::leak(description.clone().into_boxed_str());
        Self {
            registered_name: Box::leak(registered_name.into_boxed_str()),
            description_static,
            remote_name,
            description,
            input_schema,
            approval_scope,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &'static str {
        self.registered_name
    }

    fn description(&self) -> &str {
        self.description_static
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description.clone(),
            input_schema: self.input_schema(),
            approval_scope: self.approval_scope,
            read_only: self.approval_scope.is_none(),
            parallel_safe: true,
        }
    }

    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let content = self
            .client
            .call_tool(
                &self.remote_name,
                input,
                ctx.timeout,
                ctx.cancellation_token.clone(),
            )
            .await?;
        Ok(ToolOutput { content })
    }
}
