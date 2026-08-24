pub mod client;
pub mod perimeter;

use ta_protocol::wire::{
    ApprovalScope, RuntimeExtensionAvailability, RuntimeExtensionMcpServer, RuntimeExtensionState,
};

use crate::tools::{McpTool, Registry};
use crate::{ExecutionError, ExecutionRequest};

use client::McpClient;

#[derive(Default)]
pub struct McpToolRegistry {
    clients: Vec<McpClient>,
}

impl McpToolRegistry {
    #[tracing::instrument(skip(registry, request), fields(runtime_profile = %request.runtime_profile_id.as_str()))]
    pub async fn mount_from_request(
        registry: &mut Registry,
        request: &ExecutionRequest,
    ) -> Result<Self, ExecutionError> {
        let mut mounted = Self::default();
        for extension in enabled_mcp_extensions(&request.runtime_extensions) {
            let server_id = extension.descriptor.id.as_str().to_string();
            tracing::debug!(server_id = %server_id, "mounting MCP runtime extension");
            let client = match extension.mcp_server.as_ref().ok_or_else(|| {
                ExecutionError::InvalidConfig(format!(
                    "runtime extension {} does not define an MCP server",
                    extension.descriptor.id.as_str()
                ))
            })? {
                RuntimeExtensionMcpServer::Stdio(spec) => {
                    McpClient::connect_stdio(
                        server_id.clone(),
                        spec,
                        request.execution_context.effective_cwd.as_path(),
                    )
                    .await?
                }
                RuntimeExtensionMcpServer::Http(spec) => {
                    McpClient::connect_http(server_id.clone(), spec).await?
                }
            };
            for spec in client.list_tools().await? {
                let plain_name = spec.name.clone();
                let registered_name = if registry.get(&plain_name).is_some() {
                    format!("mcp/{server_id}/{plain_name}")
                } else {
                    plain_name
                };
                tracing::debug!(
                    server_id = %server_id,
                    tool = %spec.name,
                    registered_tool = %registered_name,
                    "registering MCP tool"
                );
                registry.add(McpTool::new(
                    registered_name,
                    spec.name,
                    spec.description,
                    spec.input_schema,
                    approval_scope(spec.dangerous),
                    client.clone(),
                ))?;
            }
            mounted.clients.push(client);
        }
        Ok(mounted)
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn disconnect_blocking(&mut self) {
        if self.clients.is_empty() {
            return;
        }
        let clients = std::mem::take(&mut self.clients);
        let join = std::thread::Builder::new()
            .name("taugentic-mcp-unmount".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::warn!(%error, "failed to create runtime for MCP unmount");
                        return;
                    }
                };
                for client in clients {
                    let server_id = client.server_id().to_string();
                    if let Err(error) = runtime.block_on(client.disconnect()) {
                        tracing::warn!(%server_id, %error, "failed to disconnect MCP server");
                    }
                }
            });
        match join {
            Ok(handle) => {
                if handle.join().is_err() {
                    tracing::warn!("MCP unmount thread panicked");
                }
            }
            Err(error) => tracing::warn!(%error, "failed to spawn MCP unmount thread"),
        }
    }
}

impl Drop for McpToolRegistry {
    fn drop(&mut self) {
        self.disconnect_blocking();
    }
}

fn enabled_mcp_extensions(
    extensions: &[RuntimeExtensionState],
) -> impl Iterator<Item = &RuntimeExtensionState> {
    extensions.iter().filter(|extension| {
        extension.enabled
            && extension.availability == RuntimeExtensionAvailability::Available
            && extension.mcp_server.is_some()
    })
}

fn approval_scope(dangerous: bool) -> Option<ApprovalScope> {
    dangerous.then_some(ApprovalScope::ProcessExec)
}
