use serde::Deserialize;
use serde_json::{Value, json};
use ta_protocol::wire::{AgentStreamFrame, StreamEmission};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt, BufReader, Lines},
    process::ChildStdout,
};

use super::{
    AcpClient, AcpClientEvent, AcpPermissionDecisionFuture, AcpPermissionRequest, RpcState,
    TraceContext,
    errors::{JsonRpcError, format_json_rpc_error},
    permissions::{parse_permission_request, permission_decision_result, unexpected_permission},
    spawn::terminate_child,
    stream::{AcpStreamEmissionMapper, session_update_events, turn_id},
};
use crate::{
    error::AcpClientError,
    mcp::{AcpMcpCapabilities, filter_supported_mcp_servers},
    session::AcpSession,
};

const ACP_PROTOCOL_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpSessionModelUpdate {
    NotNeeded,
    Updated,
    Unsupported,
}

impl AcpClient {
    pub async fn initialize(&mut self) -> Result<AcpMcpCapabilities, AcpClientError> {
        let init_id = self.rpc.next_id();
        send_request(
            &mut self.writer,
            init_id,
            "initialize",
            initialize_params(),
            &self.rpc.trace,
        )
        .await?;
        let mut ignore_events = |_| Ok(());
        let mut reject_permission = unexpected_permission;
        let initialize_result = read_response(
            &mut self.lines,
            &mut self.writer,
            init_id,
            &mut self.rpc,
            &mut ignore_events,
            &mut reject_permission,
        )
        .await?;
        Ok(AcpMcpCapabilities::from_initialize_result(
            &initialize_result,
        ))
    }

    pub async fn create_session(
        &mut self,
        capabilities: &AcpMcpCapabilities,
    ) -> Result<AcpSession, AcpClientError> {
        let session_id = self.rpc.next_id();
        let mcp_servers = filter_supported_mcp_servers(&self.config.mcp_servers, capabilities);
        send_request(
            &mut self.writer,
            session_id,
            "session/new",
            json!({ "cwd": self.config.work_dir, "mcpServers": mcp_servers }),
            &self.rpc.trace,
        )
        .await?;
        let mut ignore_events = |_| Ok(());
        let mut reject_permission = unexpected_permission;
        let result = read_response(
            &mut self.lines,
            &mut self.writer,
            session_id,
            &mut self.rpc,
            &mut ignore_events,
            &mut reject_permission,
        )
        .await?;
        AcpSession::from_new_session_result(&result)
    }

    pub async fn set_session_mode_if_needed(
        &mut self,
        session: &AcpSession,
    ) -> Result<(), AcpClientError> {
        let Some(mode_id) = self.config.session_mode_id.clone() else {
            return Ok(());
        };
        if !session.needs_mode_update(&mode_id)? {
            return Ok(());
        }
        let request_id = self.rpc.next_id();
        send_request(
            &mut self.writer,
            request_id,
            "session/set_mode",
            json!({ "sessionId": session.id.clone(), "modeId": mode_id }),
            &self.rpc.trace,
        )
        .await?;
        let mut ignore_events = |_| Ok(());
        let mut reject_permission = unexpected_permission;
        read_response(
            &mut self.lines,
            &mut self.writer,
            request_id,
            &mut self.rpc,
            &mut ignore_events,
            &mut reject_permission,
        )
        .await
        .map(|_| ())
        .map_err(|error| {
            AcpClientError::InvalidConfig(format!(
                "ACP agent rejected session mode {mode_id}: {error}"
            ))
        })
    }

    pub async fn set_session_model_if_needed(
        &mut self,
        session: &AcpSession,
    ) -> Result<AcpSessionModelUpdate, AcpClientError> {
        let Some(model_id) = self.config.session_model_id.clone() else {
            return Ok(AcpSessionModelUpdate::NotNeeded);
        };
        if !session.needs_model_update(&model_id)? {
            return Ok(AcpSessionModelUpdate::NotNeeded);
        }
        let request_id = self.rpc.next_id();
        send_request(
            &mut self.writer,
            request_id,
            "session/set_model",
            json!({ "sessionId": session.id.clone(), "modelId": model_id }),
            &self.rpc.trace,
        )
        .await?;
        let mut ignore_events = |_| Ok(());
        let mut reject_permission = unexpected_permission;
        read_response(
            &mut self.lines,
            &mut self.writer,
            request_id,
            &mut self.rpc,
            &mut ignore_events,
            &mut reject_permission,
        )
        .await
        .map(|_| AcpSessionModelUpdate::Updated)
        .or_else(|error| {
            if error.is_method_not_found() {
                Ok(AcpSessionModelUpdate::Unsupported)
            } else {
                Err(AcpClientError::InvalidConfig(format!(
                    "ACP agent rejected session model {model_id}: {error}"
                )))
            }
        })
    }

    pub async fn prompt(
        &mut self,
        session: &AcpSession,
        objective: &str,
        on_event: &mut impl FnMut(AcpClientEvent) -> Result<(), AcpClientError>,
    ) -> Result<Value, AcpClientError> {
        let mut reject_permission = unexpected_permission;
        self.prompt_with_permissions(session, objective, on_event, &mut reject_permission)
            .await
    }

    pub async fn prompt_with_permissions(
        &mut self,
        session: &AcpSession,
        objective: &str,
        on_event: &mut impl FnMut(AcpClientEvent) -> Result<(), AcpClientError>,
        on_permission: &mut impl FnMut(AcpPermissionRequest) -> AcpPermissionDecisionFuture,
    ) -> Result<Value, AcpClientError> {
        if objective.trim().is_empty() {
            return Err(AcpClientError::InvalidConfig(
                "ACP prompt objective must not be empty".to_string(),
            ));
        }
        let prompt_id = self.rpc.next_id();
        send_request(
            &mut self.writer,
            prompt_id,
            "session/prompt",
            json!({
                "sessionId": session.id.clone(),
                "prompt": [{ "type": "text", "text": objective }]
            }),
            &self.rpc.trace,
        )
        .await?;
        read_response(
            &mut self.lines,
            &mut self.writer,
            prompt_id,
            &mut self.rpc,
            on_event,
            on_permission,
        )
        .await
    }

    pub async fn prompt_stream(
        &mut self,
        session: &AcpSession,
        objective: &str,
        on_emission: &mut impl FnMut(StreamEmission) -> Result<(), AcpClientError>,
    ) -> Result<Value, AcpClientError> {
        let mut reject_permission = unexpected_permission;
        self.prompt_stream_with_permissions(session, objective, on_emission, &mut reject_permission)
            .await
    }

    pub async fn prompt_stream_with_permissions(
        &mut self,
        session: &AcpSession,
        objective: &str,
        on_emission: &mut impl FnMut(StreamEmission) -> Result<(), AcpClientError>,
        on_permission: &mut impl FnMut(AcpPermissionRequest) -> AcpPermissionDecisionFuture,
    ) -> Result<Value, AcpClientError> {
        let prompt_turn_id = turn_id(&format!("acp-prompt-{}", self.rpc.next_id))?;
        let mut mapper = AcpStreamEmissionMapper::new(prompt_turn_id);
        on_emission(mapper.lifecycle(AgentStreamFrame::AssistantTurnStarted))?;
        let mut on_event = |event| {
            let emission = mapper.map(event)?;
            on_emission(emission)
        };
        let result = self
            .prompt_with_permissions(session, objective, &mut on_event, on_permission)
            .await;
        if result.is_ok() {
            on_emission(mapper.lifecycle(AgentStreamFrame::AssistantTurnCompleted))?;
        }
        result
    }

    pub async fn cancel_session(&mut self, session: &AcpSession) -> Result<(), AcpClientError> {
        send_notification(
            &mut self.writer,
            "session/cancel",
            json!({ "sessionId": session.id.clone() }),
            &self.rpc.trace,
        )
        .await
    }

    pub async fn shutdown(mut self) -> Result<(), AcpClientError> {
        terminate_child(&mut self.child, self.config.cancel_grace).await
    }
}

fn initialize_params() -> Value {
    json!({
        "protocolVersion": ACP_PROTOCOL_VERSION,
        "clientCapabilities": {},
        "clientInfo": {
            "name": "taugentic",
            "title": "Taugentic",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

#[tracing::instrument(
    skip(lines, writer, rpc, on_event, on_permission),
    fields(
        flavor_id = %rpc.trace.flavor_id,
        run_id = %rpc.trace.run_id,
        session_id = %rpc.trace.session_id,
        rpc.expected_id = expected_id
    )
)]
async fn read_response<W>(
    lines: &mut Lines<BufReader<ChildStdout>>,
    writer: &mut W,
    expected_id: u64,
    rpc: &mut RpcState,
    on_event: &mut impl FnMut(AcpClientEvent) -> Result<(), AcpClientError>,
    on_permission: &mut impl FnMut(AcpPermissionRequest) -> AcpPermissionDecisionFuture,
) -> Result<Value, AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    while let Some(line) = lines.next_line().await.map_err(|error| {
        AcpClientError::ProcessFailed(format!("failed reading ACP stdout: {error}"))
    })? {
        if line.trim().is_empty() {
            continue;
        }
        let message: JsonRpcMessage = serde_json::from_str(&line).map_err(|error| {
            AcpClientError::ProcessFailed(format!("failed to decode ACP JSON-RPC line: {error}"))
        })?;
        if message.method.as_deref() == Some("session/update") {
            for event in session_update_events(message.params) {
                on_event(event)?;
            }
        } else if message.method.as_deref() == Some("session/request_permission") {
            let request = parse_permission_request(message.params)?;
            let decision = on_permission(request).await?;
            send_success_response(
                writer,
                message.id,
                permission_decision_result(decision),
                &rpc.trace,
            )
            .await?;
        } else if message.id.is_some() && message.method.is_some() {
            send_error_response(
                writer,
                message.id,
                -32601,
                "client method not implemented",
                &rpc.trace,
            )
            .await?;
        } else if message.id == Some(json!(expected_id)) {
            if let Some(error) = message.error {
                return Err(AcpClientError::JsonRpcRequestFailed {
                    request_id: expected_id,
                    code: error.code,
                    detail: format_json_rpc_error(&error),
                });
            }
            return Ok(message.result.unwrap_or(Value::Null));
        }
    }
    Err(AcpClientError::ProcessFailed(format!(
        "ACP process exited before response id {expected_id}"
    )))
}

#[tracing::instrument(
    skip(writer, params, trace),
    fields(
        flavor_id = %trace.flavor_id,
        run_id = %trace.run_id,
        session_id = %trace.session_id,
        rpc.id = id,
        rpc.method = method
    )
)]
async fn send_request<W>(
    writer: &mut W,
    id: u64,
    method: &str,
    params: Value,
    trace: &TraceContext,
) -> Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    write_json_rpc_frame(
        writer,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }),
    )
    .await
}

#[tracing::instrument(
    skip(writer, params, trace),
    fields(
        flavor_id = %trace.flavor_id,
        run_id = %trace.run_id,
        session_id = %trace.session_id,
        rpc.method = method
    )
)]
async fn send_notification<W>(
    writer: &mut W,
    method: &str,
    params: Value,
    trace: &TraceContext,
) -> Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    write_json_rpc_frame(
        writer,
        json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }),
    )
    .await
}

#[tracing::instrument(
    skip(writer, result, trace),
    fields(
        flavor_id = %trace.flavor_id,
        run_id = %trace.run_id,
        session_id = %trace.session_id
    )
)]
async fn send_success_response<W>(
    writer: &mut W,
    id: Option<Value>,
    result: Value,
    trace: &TraceContext,
) -> Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    write_json_rpc_frame(
        writer,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }),
    )
    .await
}

#[tracing::instrument(
    skip(writer, trace),
    fields(
        flavor_id = %trace.flavor_id,
        run_id = %trace.run_id,
        session_id = %trace.session_id,
        rpc.error_code = code
    )
)]
async fn send_error_response<W>(
    writer: &mut W,
    id: Option<Value>,
    code: i64,
    message: &str,
    trace: &TraceContext,
) -> Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    write_json_rpc_frame(
        writer,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        }),
    )
    .await
}

async fn write_json_rpc_frame<W>(writer: &mut W, frame: Value) -> Result<(), AcpClientError>
where
    W: AsyncWrite + Unpin,
{
    let line = serde_json::to_string(&frame).map_err(|error| {
        AcpClientError::ProcessFailed(format!("failed to encode ACP JSON-RPC frame: {error}"))
    })?;
    writer
        .write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|error| {
            AcpClientError::ProcessFailed(format!("failed writing ACP stdin: {error}"))
        })?;
    writer.flush().await.map_err(|error| {
        AcpClientError::ProcessFailed(format!("failed flushing ACP stdin: {error}"))
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcMessage {
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}
