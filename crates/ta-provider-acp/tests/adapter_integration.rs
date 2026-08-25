#![cfg(target_os = "macos")]

mod support;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use ta_protocol::wire::{
    AgentStreamFrame, AgentToolCallOutcome, RuntimeExtensionAvailability,
    RuntimeExtensionDescriptor, RuntimeExtensionEnvVar, RuntimeExtensionHttpHeader,
    RuntimeExtensionId, RuntimeExtensionMcpHttpServer, RuntimeExtensionMcpServer,
    RuntimeExtensionMcpStdioServer, RuntimeExtensionState,
};
use ta_provider_acp::{
    adapter::{
        AcpClientEvent, AcpClientTrace, AcpProcessAdapter, AcpProcessConfig, AcpSessionModelUpdate,
    },
    descriptor::{AcpLaunchKind, AcpProviderSpec},
    error::AcpClientError,
    launch::build_perimeter_profile,
    mcp::{
        AcpMcpServerSpec, AcpMcpStdioServer, extension_to_mcp_server, extensions_to_mcp_servers,
    },
};

#[tokio::test]
async fn adapter_spawns_stub_translates_updates_and_records_json_rpc_frames() {
    let dir = unique_dir("adapter-stub");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", capability_stub_script("{}"));

    let mut client = spawn_client(config(&dir, stub, vec![trace_env(&trace_file)]));
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let mut events = Vec::new();
    client
        .prompt(&session, "say hello", &mut |event| {
            events.push(event);
            Ok(())
        })
        .await
        .expect("prompt");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(methods, ["initialize", "session/new", "session/prompt"]);
    assert_eq!(
        sent[1].pointer("/params/mcpServers"),
        Some(&Value::Array(vec![]))
    );
    assert!(events.iter().any(
        |event| matches!(event, AcpClientEvent::AssistantTextDelta(delta) if delta == "hello")
    ));
    assert!(events.iter().any(
        |event| matches!(event, AcpClientEvent::ToolCallStarted { tool_name, .. } if tool_name == "Read")
    ));
    assert!(events.iter().any(|event| matches!(
        event,
        AcpClientEvent::ToolCallCompleted {
            outcome: AgentToolCallOutcome::Completed,
            ..
        }
    )));
}

#[tokio::test]
async fn prompt_stream_emits_single_adapter_owned_turn_lifecycle() {
    let dir = unique_dir("adapter-stream-turn");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", capability_stub_script("{}"));

    let mut client = spawn_client(config(&dir, stub, vec![trace_env(&trace_file)]));
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let mut emissions = Vec::new();
    client
        .prompt_stream(&session, "say hello", &mut |emission| {
            emissions.push(emission);
            Ok(())
        })
        .await
        .expect("prompt stream");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let frames = emissions
        .iter()
        .map(|emission| emission.frame.clone())
        .collect::<Vec<_>>();
    let turn_ids = emissions
        .iter()
        .map(|emission| {
            emission
                .turn_id
                .as_ref()
                .expect("adapter stream emissions carry a turn id")
                .as_str()
        })
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(methods, ["initialize", "session/new", "session/prompt"]);
    assert_eq!(
        frames,
        vec![
            AgentStreamFrame::AssistantTurnStarted,
            AgentStreamFrame::AssistantMessageDelta {
                delta: "hello".to_string()
            },
            AgentStreamFrame::ToolCallStarted {
                tool_name: "Read".to_string(),
                input: "null".to_string()
            },
            AgentStreamFrame::ToolCallProgressed {
                delta: "done".to_string()
            },
            AgentStreamFrame::ToolCallCompleted {
                outcome: AgentToolCallOutcome::Completed
            },
            AgentStreamFrame::AssistantTurnCompleted
        ]
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| matches!(frame, AgentStreamFrame::AssistantTurnStarted))
            .count(),
        1
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| matches!(frame, AgentStreamFrame::AssistantTurnCompleted))
            .count(),
        1
    );
    assert!(turn_ids.iter().all(|turn_id| *turn_id == "acp-prompt-3"));
}

#[tokio::test]
async fn http_mcp_servers_require_agent_capability_and_serialize_type() {
    let unsupported = run_http_mcp_trace("adapter-http-unsupported", "{}").await;
    assert_eq!(
        unsupported[1].pointer("/params/mcpServers"),
        Some(&Value::Array(vec![]))
    );

    let supported = run_http_mcp_trace(
        "adapter-http-supported",
        r#"{"mcpCapabilities":{"http":true}}"#,
    )
    .await;
    assert_eq!(
        supported[1].pointer("/params/mcpServers/0/type"),
        Some(&Value::String("http".to_string()))
    );
    assert_eq!(
        supported[1].pointer("/params/mcpServers/0/url"),
        Some(&Value::String("https://example.invalid/mcp".to_string()))
    );
}

#[tokio::test]
async fn session_mode_must_be_advertised_before_set_mode() {
    let dir = unique_dir("adapter-mode-invalid");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", mode_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_mode_id = Some("code".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let result = client.set_session_mode_if_needed(&session).await;
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert!(matches!(result, Err(AcpClientError::InvalidConfig(_))));
    assert_eq!(methods, ["initialize", "session/new"]);
}

#[tokio::test]
async fn session_mode_is_not_set_when_already_current() {
    let dir = unique_dir("adapter-mode-current");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", current_mode_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_mode_id = Some("ask".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    client
        .set_session_mode_if_needed(&session)
        .await
        .expect("mode");
    client
        .prompt(&session, "say hello", &mut |_| Ok(()))
        .await
        .expect("prompt");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(methods, ["initialize", "session/new", "session/prompt"]);
}

#[tokio::test]
async fn session_model_must_be_advertised_before_set_model() {
    let dir = unique_dir("adapter-model-invalid");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", model_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_model_id = Some("gpt-missing".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let result = client.set_session_model_if_needed(&session).await;
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert!(matches!(result, Err(AcpClientError::InvalidConfig(_))));
    assert_eq!(methods, ["initialize", "session/new"]);
}

#[tokio::test]
async fn session_model_is_not_set_when_already_current() {
    let dir = unique_dir("adapter-model-current");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", current_model_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_model_id = Some("gpt-5.6-sol".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    client
        .set_session_model_if_needed(&session)
        .await
        .expect("model");
    client
        .prompt(&session, "say hello", &mut |_| Ok(()))
        .await
        .expect("prompt");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(methods, ["initialize", "session/new", "session/prompt"]);
}

#[tokio::test]
async fn session_model_set_uses_advertised_model() {
    let dir = unique_dir("adapter-model-set");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", model_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_model_id = Some("gpt-5.6-sol".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    client
        .set_session_model_if_needed(&session)
        .await
        .expect("model");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let model_id = sent
        .iter()
        .find(|frame| frame.get("method").and_then(Value::as_str) == Some("session/set_model"))
        .and_then(|frame| frame.pointer("/params/modelId"))
        .and_then(Value::as_str);
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(methods, ["initialize", "session/new", "session/set_model"]);
    assert_eq!(model_id, Some("gpt-5.6-sol"));
}

#[tokio::test]
async fn session_model_is_not_set_without_requested_model() {
    let dir = unique_dir("adapter-model-none");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", model_stub_script());

    let mut client = spawn_client(config(&dir, stub, vec![trace_env(&trace_file)]));
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let update = client
        .set_session_model_if_needed(&session)
        .await
        .expect("model");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(update, AcpSessionModelUpdate::NotNeeded);
    assert_eq!(methods, ["initialize", "session/new"]);
}

#[tokio::test]
async fn session_model_method_not_found_is_soft_unsupported() {
    let dir = unique_dir("adapter-model-unsupported");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", unsupported_model_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_model_id = Some("gpt-5.6-sol".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let update = client
        .set_session_model_if_needed(&session)
        .await
        .expect("model");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(update, AcpSessionModelUpdate::Unsupported);
    assert_eq!(methods, ["initialize", "session/new", "session/set_model"]);
}

#[tokio::test]
async fn session_model_set_error_is_invalid_config_and_stops_before_prompt() {
    let dir = unique_dir("adapter-model-rejected");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", rejected_model_stub_script());
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.session_model_id = Some("gpt-5.6-sol".to_string());

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let mut on_event = |_| Ok(());
    let result = match client.set_session_model_if_needed(&session).await {
        Ok(_) => client
            .prompt(&session, "say hello", &mut on_event)
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let methods = sent
        .iter()
        .filter_map(|frame| frame.get("method").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let _ = fs::remove_dir_all(&dir);

    assert!(matches!(result, Err(AcpClientError::InvalidConfig(_))));
    assert_eq!(methods, ["initialize", "session/new", "session/set_model"]);
}

#[tokio::test]
async fn prompt_error_includes_json_rpc_code_and_data() {
    let dir = unique_dir("adapter-prompt-error-data");
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", prompt_error_stub_script());

    let mut client = spawn_client(config(&dir, stub, vec![trace_env(&trace_file)]));
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    let result = client.prompt(&session, "say hello", &mut |_| Ok(())).await;
    client.shutdown().await.expect("shutdown");
    let _ = fs::remove_dir_all(&dir);

    let detail = match result {
        Err(AcpClientError::JsonRpcRequestFailed { detail, .. }) => detail,
        other => panic!("expected process failure, got {other:?}"),
    };
    assert!(detail.contains("Internal error (code -32603)"));
    assert!(detail.contains("The 'gpt-5.5' model requires a newer version of Codex"));
}

#[test]
fn extension_to_mcp_server_filters_and_maps_enabled_stdio_extensions() {
    let extension = ta_protocol::wire::RuntimeExtensionState {
        descriptor: RuntimeExtensionDescriptor {
            id: RuntimeExtensionId::new("fs").expect("extension id"),
            display_name: "Filesystem".to_string(),
            description: "Filesystem MCP".to_string(),
        },
        availability: RuntimeExtensionAvailability::Available,
        enabled: true,
        mcp_server: Some(RuntimeExtensionMcpServer::Stdio(
            RuntimeExtensionMcpStdioServer {
                name: "filesystem".to_string(),
                command: "/bin/fs-mcp".to_string(),
                args: vec!["--stdio".to_string()],
                env: vec![RuntimeExtensionEnvVar {
                    name: "ROOT".to_string(),
                    value: "/tmp".to_string(),
                }],
            },
        )),
    };

    let mapped = extension_to_mcp_server(&extension).expect("mcp server");

    assert!(matches!(
        mapped,
        AcpMcpServerSpec::Stdio(AcpMcpStdioServer { name, command, .. })
            if name == "filesystem" && command == "/bin/fs-mcp"
    ));
}

async fn run_http_mcp_trace(prefix: &str, capabilities: &str) -> Vec<Value> {
    let dir = unique_dir(prefix);
    fs::create_dir_all(&dir).expect("temp dir");
    let trace_file = dir.join("frames.ndjson");
    let stub = write_stub(&dir, "stub-acp", capability_stub_script(capabilities));
    let mut config = config(&dir, stub, vec![trace_env(&trace_file)]);
    config.mcp_servers = extensions_to_mcp_servers(&[http_extension()]);

    let mut client = spawn_client(config);
    let capabilities = client.initialize().await.expect("initialize");
    let session = client.create_session(&capabilities).await.expect("session");
    client
        .prompt(&session, "say hello", &mut |_| Ok(()))
        .await
        .expect("prompt");
    client.shutdown().await.expect("shutdown");

    let sent = read_trace(&trace_file);
    let _ = fs::remove_dir_all(&dir);
    sent
}

fn spawn_client(config: AcpProcessConfig) -> ta_provider_acp::adapter::AcpClient {
    AcpProcessAdapter::new(config)
        .spawn(AcpClientTrace {
            run_id: "run".to_string(),
            session_id: "session".to_string(),
        })
        .expect("spawn")
}

fn config(work_dir: &Path, command: PathBuf, env: Vec<(String, String)>) -> AcpProcessConfig {
    let sandbox_profile = test_perimeter_profile(work_dir, &command);
    AcpProcessConfig {
        flavor_id: "test-acp".to_string(),
        command,
        sandbox_profile,
        args: Vec::new(),
        env,
        env_remove: Vec::new(),
        work_dir: work_dir.to_path_buf(),
        mcp_servers: Vec::new(),
        session_mode_id: None,
        session_model_id: None,
        cancel_grace: Duration::from_millis(100),
    }
}

fn test_perimeter_profile(work_dir: &Path, command: &Path) -> ta_exec::SandboxProfile {
    let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
    build_perimeter_profile(&provider, &support::execution_context(work_dir), command)
        .expect("test ACP perimeter profile")
}

fn http_extension() -> RuntimeExtensionState {
    RuntimeExtensionState {
        descriptor: RuntimeExtensionDescriptor {
            id: RuntimeExtensionId::new("remote").expect("extension id"),
            display_name: "Remote".to_string(),
            description: "Remote HTTP MCP".to_string(),
        },
        availability: RuntimeExtensionAvailability::Available,
        enabled: true,
        mcp_server: Some(RuntimeExtensionMcpServer::Http(
            RuntimeExtensionMcpHttpServer {
                name: "remote".to_string(),
                url: "https://example.invalid/mcp".to_string(),
                headers: vec![RuntimeExtensionHttpHeader {
                    name: "Authorization".to_string(),
                    value: "Bearer test".to_string(),
                }],
            },
        )),
    }
}

fn write_stub(dir: &Path, name: &str, source: String) -> PathBuf {
    let stub = dir.join(name);
    fs::write(&stub, source).expect("stub script");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod");
    stub
}

fn trace_env(trace_file: &Path) -> (String, String) {
    ("TRACE_FILE".to_string(), trace_file.display().to_string())
}

fn read_trace(trace_file: &Path) -> Vec<Value> {
    fs::read_to_string(trace_file)
        .expect("trace file")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json-rpc frame"))
        .collect()
}

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/ta-provider-acp")
        .join(format!("{prefix}-{nanos}"))
}

fn capability_stub_script(capabilities: &str) -> String {
    format!(
        r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"protocolVersion":1,"agentCapabilities":{capabilities}}}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"sessionId":"s1"}}}}'
      ;;
    *'"id":3'*)
      printf '%s\n' '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"s1","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"hello"}}}}}}}}'
      printf '%s\n' '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"s1","update":{{"sessionUpdate":"tool_call","toolCallId":"t1","title":"Read"}}}}}}'
      printf '%s\n' '{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"s1","update":{{"sessionUpdate":"tool_call_update","toolCallId":"t1","fields":{{"status":"completed","rawOutput":"done"}}}}}}}}'
      printf '%s\n' '{{"jsonrpc":"2.0","id":3,"result":{{"stopReason":"end_turn"}}}}'
      ;;
  esac
done
"#
    )
}

fn mode_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","modes":{"currentModeId":"ask","availableModes":[{"id":"ask","name":"Ask"}]}}}'
      ;;
    *'"method":"session/set_mode"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":null}'
      ;;
  esac
done
"#
    .to_string()
}

fn current_mode_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","modes":{"currentModeId":"ask","availableModes":[{"id":"ask","name":"Ask"}]}}}'
      ;;
    *'"id":3'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
      ;;
    *'"method":"session/set_mode"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"session/set_mode should not be called"}}'
      ;;
  esac
done
"#
    .to_string()
}

fn model_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","models":{"currentModelId":"gpt-4.1","availableModels":[{"modelId":"gpt-4.1","name":"GPT-4.1"},{"modelId":"gpt-5.6-sol","name":"GPT-5.6 Sol"}]}}}'
      ;;
    *'"method":"session/set_model"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":null}'
      ;;
  esac
done
"#
    .to_string()
}

fn current_model_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","models":{"currentModelId":"gpt-5.6-sol","availableModels":[{"modelId":"gpt-5.6-sol","name":"GPT-5.6 Sol"}]}}}'
      ;;
    *'"id":3'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
      ;;
    *'"method":"session/set_model"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"session/set_model should not be called"}}'
      ;;
  esac
done
"#
    .to_string()
}

fn unsupported_model_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","models":{"currentModelId":"gpt-4.1","availableModels":[{"modelId":"gpt-4.1","name":"GPT-4.1"},{"modelId":"gpt-5.6-sol","name":"GPT-5.6 Sol"}]}}}'
      ;;
    *'"method":"session/set_model"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"method not found"}}'
      ;;
  esac
done
"#
    .to_string()
}

fn rejected_model_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","models":{"currentModelId":"gpt-4.1","availableModels":[{"modelId":"gpt-4.1","name":"GPT-4.1"},{"modelId":"gpt-5.6-sol","name":"GPT-5.6 Sol"}]}}}'
      ;;
    *'"method":"session/set_model"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"model rejected"}}'
      ;;
    *'"method":"session/prompt"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}'
      ;;
  esac
done
"#
    .to_string()
}

fn prompt_error_stub_script() -> String {
    r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$TRACE_FILE"
  case "$line" in
    *'"id":1'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}'
      ;;
    *'"id":2'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1"}}'
      ;;
    *'"id":3'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":3,"error":{"code":-32603,"message":"Internal error","data":"The '\''gpt-5.5'\'' model requires a newer version of Codex. Please upgrade to the latest app or CLI and try again."}}'
      ;;
  esac
done
"#
    .to_string()
}
