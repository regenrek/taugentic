use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, CallToolResult, JsonObject};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, async_rw::AsyncRwTransport};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Map, Value, json};
use ta_exec::{
    ExecEngine, ExecError, LocalExecEngine, ProcessGroupPolicy, SpawnRequest, StdioPolicy,
    terminate_child_tree,
};
use ta_protocol::wire::{RuntimeExtensionMcpHttpServer, RuntimeExtensionMcpStdioServer};
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::ExecutionError;
use crate::mcp::perimeter::build_mcp_perimeter_profile;

const MCP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const MCP_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const MCP_TOOL_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
const MCP_TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct McpToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub dangerous: bool,
}

#[derive(Clone)]
pub struct McpClient {
    server_id: String,
    service: Arc<Mutex<Option<RunningService<RoleClient, TaugenticMcpHandler>>>>,
    process: Option<Arc<Mutex<Child>>>,
}

#[derive(Clone)]
struct TaugenticMcpHandler;

impl ClientHandler for TaugenticMcpHandler {}

impl McpClient {
    #[tracing::instrument(skip(spec, workdir), fields(server_id = %server_id, command = %spec.command))]
    pub async fn connect_stdio(
        server_id: String,
        spec: &RuntimeExtensionMcpStdioServer,
        workdir: &Path,
    ) -> Result<Self, ExecutionError> {
        let command = PathBuf::from(&spec.command);
        let sandbox_profile = build_mcp_perimeter_profile(&server_id, workdir, &command)?;
        let mut request = SpawnRequest::new(command.as_os_str())
            .args(&spec.args)
            .cwd(workdir)
            .stdin(StdioPolicy::Piped)
            .stdout(StdioPolicy::Piped)
            .stderr(StdioPolicy::Inherit)
            .process_group(mcp_process_group_policy())
            .sandbox_profile(sandbox_profile);
        // MCP spec.env is the user-controlled secret bridge; it intentionally
        // bypasses the profile's base env allowlist.
        for env in &spec.env {
            request = request.env(&env.name, &env.value);
        }
        let mut child = LocalExecEngine
            .spawn(request)
            .map_err(|error| map_mcp_spawn_error(&spec.name, error))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ExecutionError::ProcessFailed(format!("MCP server {} stdout pipe missing", spec.name))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            ExecutionError::ProcessFailed(format!("MCP server {} stdin pipe missing", spec.name))
        })?;
        let transport = AsyncRwTransport::new(stdout, stdin);
        let process = Arc::new(Mutex::new(child));
        Self::connect_transport(
            server_id,
            TaugenticMcpHandler.serve(transport),
            Some(process),
        )
        .await
    }

    #[tracing::instrument(skip(spec), fields(server_id = %server_id, url = %spec.url))]
    pub async fn connect_http(
        server_id: String,
        spec: &RuntimeExtensionMcpHttpServer,
    ) -> Result<Self, ExecutionError> {
        let mut custom_headers = HashMap::new();
        for header in &spec.headers {
            let name = header.name.parse().map_err(|error| {
                ExecutionError::InvalidConfig(format!(
                    "invalid MCP HTTP header {}: {error}",
                    header.name
                ))
            })?;
            let value = header.value.parse().map_err(|error| {
                ExecutionError::InvalidConfig(format!(
                    "invalid MCP HTTP header value for {}: {error}",
                    header.name
                ))
            })?;
            custom_headers.insert(name, value);
        }
        let config = StreamableHttpClientTransportConfig::with_uri(spec.url.clone())
            .custom_headers(custom_headers);
        let transport = StreamableHttpClientTransport::from_config(config);
        Self::connect_transport(server_id, TaugenticMcpHandler.serve(transport), None).await
    }

    async fn connect_transport<F>(
        server_id: String,
        service: F,
        process: Option<Arc<Mutex<Child>>>,
    ) -> Result<Self, ExecutionError>
    where
        F: Future<
            Output = Result<
                RunningService<RoleClient, TaugenticMcpHandler>,
                rmcp::service::ClientInitializeError,
            >,
        >,
    {
        let service = match tokio::time::timeout(MCP_INITIALIZE_TIMEOUT, service).await {
            Ok(Ok(service)) => service,
            Ok(Err(error)) => {
                terminate_mcp_process(&process, &server_id).await;
                return Err(ExecutionError::ProcessFailed(format!(
                    "failed to initialize MCP server {server_id}: {error}"
                )));
            }
            Err(_) => {
                terminate_mcp_process(&process, &server_id).await;
                return Err(ExecutionError::ProcessFailed(format!(
                    "timed out initializing MCP server {server_id} after {}s",
                    MCP_INITIALIZE_TIMEOUT.as_secs()
                )));
            }
        };
        Ok(Self {
            server_id,
            service: Arc::new(Mutex::new(Some(service))),
            process,
        })
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    #[tracing::instrument(skip(self), fields(server_id = %self.server_id))]
    pub async fn list_tools(&self) -> Result<Vec<McpToolSpec>, ExecutionError> {
        let mut guard = self.service.lock().await;
        let service = active_service(&mut guard, &self.server_id)?;
        let result = tokio::time::timeout(
            MCP_TOOL_DISCOVERY_TIMEOUT,
            service.peer().list_tools(Default::default()),
        )
        .await
        .map_err(|_| {
            ExecutionError::ToolFailed(format!(
                "timed out listing tools for MCP server {} after {}s",
                self.server_id,
                MCP_TOOL_DISCOVERY_TIMEOUT.as_secs()
            ))
        })?
        .map_err(map_mcp_error)?;
        Ok(result
            .tools
            .into_iter()
            .map(|tool| McpToolSpec {
                name: tool.name.into_owned(),
                description: tool
                    .description
                    .map(|value| value.into_owned())
                    .unwrap_or_default(),
                input_schema: Value::Object(tool.input_schema.as_ref().clone()),
                dangerous: tool
                    .annotations
                    .as_ref()
                    .is_some_and(|annotations| annotations.destructive_hint.unwrap_or(false)),
            })
            .collect())
    }

    #[tracing::instrument(skip(self, arguments), fields(server_id = %self.server_id, tool = %name))]
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        timeout: Duration,
        cancellation_token: CancellationToken,
    ) -> Result<Value, ExecutionError> {
        let arguments = match arguments {
            Value::Object(map) => Some(json_object(map)),
            Value::Null => None,
            other => {
                return Err(ExecutionError::InvalidToolInput(format!(
                    "MCP tool {name} arguments must be an object or null, got {}",
                    type_name(&other)
                )));
            }
        };
        let mut params = CallToolRequestParams::new(name.to_string());
        if let Some(arguments) = arguments {
            params = params.with_arguments(arguments);
        }
        let timeout = normalize_tool_call_timeout(timeout);
        let mut call_task = tokio::spawn(self.clone().call_tool_request(params));

        let result = tokio::select! {
            result = &mut call_task => result
                .map_err(|error| ExecutionError::ToolFailed(format!("MCP tool call task failed: {error}")))??,
            () = tokio::time::sleep(timeout) => {
                abort_call_task(call_task).await;
                self.disconnect_after_aborted_call(name, "timed out").await;
                return Err(ExecutionError::ProcessTimeout {
                    timeout_ms: duration_millis(timeout),
                    detail: format!(
                        "timed out calling MCP tool {name} on server {}",
                        self.server_id
                    ),
                });
            }
            () = cancellation_token.cancelled() => {
                abort_call_task(call_task).await;
                self.disconnect_after_aborted_call(name, "cancelled").await;
                return Err(ExecutionError::Cancelled(format!(
                    "MCP tool {name} on server {} cancelled",
                    self.server_id
                )));
            }
        };
        if result.is_error == Some(true) {
            return Err(ExecutionError::ToolFailed(
                serde_json::to_string(&result).unwrap_or_else(|_| "MCP tool failed".to_string()),
            ));
        }
        Ok(result
            .structured_content
            .unwrap_or_else(|| json!(result.content)))
    }

    async fn call_tool_request(
        self,
        params: CallToolRequestParams,
    ) -> Result<CallToolResult, ExecutionError> {
        let mut guard = self.service.lock().await;
        let service = active_service(&mut guard, &self.server_id)?;
        service
            .peer()
            .call_tool(params)
            .await
            .map_err(map_mcp_error)
    }

    async fn disconnect_after_aborted_call(&self, name: &str, reason: &'static str) {
        if let Err(error) = self.disconnect().await {
            tracing::warn!(
                server_id = %self.server_id,
                tool = %name,
                %reason,
                %error,
                "failed to disconnect MCP server after aborted tool call"
            );
        }
    }

    #[tracing::instrument(skip(self), fields(server_id = %self.server_id))]
    pub async fn disconnect(&self) -> Result<(), ExecutionError> {
        let mut guard = self.service.lock().await;
        let Some(mut service) = guard.take() else {
            return Ok(());
        };
        let close_result = service.close_with_timeout(MCP_SHUTDOWN_TIMEOUT).await;
        drop(service);
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(process) = &self.process {
            let mut child = process.lock().await;
            terminate_child_tree(&mut child, Duration::from_secs(1))
                .await
                .map_err(|error| {
                    ExecutionError::ProcessFailed(format!(
                        "failed to terminate MCP server {}: {error}",
                        self.server_id
                    ))
                })?;
        }
        match close_result {
            Ok(_) => Ok(()),
            Err(error) => Err(ExecutionError::ProcessFailed(format!(
                "failed to close MCP server {}: {error}",
                self.server_id
            ))),
        }
    }
}

async fn terminate_mcp_process(process: &Option<Arc<Mutex<Child>>>, server_id: &str) {
    let Some(process) = process else {
        return;
    };
    let mut child = process.lock().await;
    if let Err(error) = terminate_child_tree(&mut child, Duration::from_secs(1)).await {
        tracing::warn!(
            %server_id,
            %error,
            "failed to terminate MCP process after initialization failure"
        );
    }
}

#[cfg(unix)]
fn mcp_process_group_policy() -> ProcessGroupPolicy {
    ProcessGroupPolicy::New
}

#[cfg(not(unix))]
fn mcp_process_group_policy() -> ProcessGroupPolicy {
    ProcessGroupPolicy::Inherit
}

fn active_service<'a>(
    service: &'a mut Option<RunningService<RoleClient, TaugenticMcpHandler>>,
    server_id: &str,
) -> Result<&'a mut RunningService<RoleClient, TaugenticMcpHandler>, ExecutionError> {
    service.as_mut().ok_or_else(|| {
        ExecutionError::ProcessFailed(format!("MCP server {server_id} is already disconnected"))
    })
}

fn json_object(map: Map<String, Value>) -> JsonObject {
    map.into_iter().collect()
}

fn normalize_tool_call_timeout(timeout: Duration) -> Duration {
    if timeout.is_zero() {
        MCP_TOOL_CALL_TIMEOUT
    } else {
        timeout
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

async fn abort_call_task(task: tokio::task::JoinHandle<Result<CallToolResult, ExecutionError>>) {
    task.abort();
    let _ = task.await;
}

fn map_mcp_error(error: rmcp::ServiceError) -> ExecutionError {
    ExecutionError::ToolFailed(error.to_string())
}

fn map_mcp_spawn_error(server_name: &str, error: ExecError) -> ExecutionError {
    match error {
        ExecError::Sandbox(ta_exec::SandboxError::Unsupported { kind, reason }) => {
            ExecutionError::Unsupported(format!(
                "MCP stdio sandbox backend is unsupported for {server_name} (kind: {kind}): {reason}"
            ))
        }
        other => ExecutionError::ProcessFailed(format!(
            "failed to spawn MCP server {server_name}: {other}"
        )),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
