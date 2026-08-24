mod support;

use std::time::Duration;

use serde_json::json;
use ta_protocol::wire::{
    RuntimeExtensionAvailability, RuntimeExtensionDescriptor, RuntimeExtensionEnvVar,
    RuntimeExtensionId, RuntimeExtensionMcpServer, RuntimeExtensionMcpStdioServer,
    RuntimeExtensionState,
};
use taugentic_agent::ExecutionError;
use taugentic_agent::mcp::McpToolRegistry;
use taugentic_agent::tools::{Registry, ToolContext};
use tempfile::TempDir;

static MCP_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const MCP_TEST_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn mcp_mount_registers_remote_tools() {
    let _guard = MCP_TEST_LOCK.lock().await;
    let mut registry = Registry::new();
    let mut request = support::request();
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    let mock_dir = mock_mcp_dir();
    request.runtime_extensions = vec![extension("srv1", "some_tool", mock_dir.path())];
    let mount = McpToolRegistry::mount_from_request(&mut registry, &request)
        .await
        .expect("mount");

    let tool = registry.get("some_tool").expect("tool");
    let output = tool
        .run(json!({"value": 1}), ToolContext::new("."))
        .await
        .expect("tool output");

    assert_eq!(output.content["result"], "some_tool");
    drop_mount_with_timeout(mount).await;
}

#[tokio::test]
async fn mcp_tool_name_collision_is_prefixed_and_builtin_wins() {
    let _guard = MCP_TEST_LOCK.lock().await;
    let mut registry = Registry::with_read_only_builtins();
    let mut request = support::request();
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    let mock_dir = mock_mcp_dir();
    request.runtime_extensions = vec![extension("srv1", "read_file", mock_dir.path())];
    let mount = McpToolRegistry::mount_from_request(&mut registry, &request)
        .await
        .expect("mount");

    assert!(registry.get("read_file").is_some());
    assert!(registry.get("mcp/srv1/read_file").is_some());
    drop_mount_with_timeout(mount).await;
}

#[tokio::test]
async fn mcp_tool_call_timeout_disconnects_unresponsive_server() {
    let _guard = MCP_TEST_LOCK.lock().await;
    let mut registry = Registry::new();
    let mut request = support::request();
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    let mock_dir = mock_mcp_dir();
    request.runtime_extensions = vec![extension_with_call_behavior(
        "srv1",
        "slow_tool",
        mock_dir.path(),
        CallBehavior::Ignore,
    )];
    let mount = McpToolRegistry::mount_from_request(&mut registry, &request)
        .await
        .expect("mount");

    let tool = registry.get("slow_tool").expect("tool");
    let mut ctx = ToolContext::new(".");
    ctx.timeout = Duration::from_millis(100);
    let error = tool
        .run(json!({"value": 1}), ctx)
        .await
        .expect_err("unresponsive MCP tool should time out");

    assert!(matches!(
        error,
        ExecutionError::ProcessTimeout { detail, .. }
            if detail.contains("timed out calling MCP tool slow_tool")
    ));
    drop_mount_with_timeout(mount).await;
}

fn extension(id: &str, tool_name: &str, mock_dir: &std::path::Path) -> RuntimeExtensionState {
    extension_with_call_behavior(id, tool_name, mock_dir, CallBehavior::Respond)
}

fn extension_with_call_behavior(
    id: &str,
    tool_name: &str,
    mock_dir: &std::path::Path,
    call_behavior: CallBehavior,
) -> RuntimeExtensionState {
    RuntimeExtensionState {
        descriptor: RuntimeExtensionDescriptor {
            id: RuntimeExtensionId::new(id).expect("id"),
            display_name: id.to_string(),
            description: id.to_string(),
        },
        availability: RuntimeExtensionAvailability::Available,
        enabled: true,
        mcp_server: Some(RuntimeExtensionMcpServer::Stdio(
            RuntimeExtensionMcpStdioServer {
                name: id.to_string(),
                command: mock_mcp_script(mock_dir, tool_name, call_behavior)
                    .to_string_lossy()
                    .to_string(),
                args: Vec::new(),
                env: python_mock_env(),
            },
        )),
    }
}

fn python_mock_env() -> Vec<RuntimeExtensionEnvVar> {
    vec![
        RuntimeExtensionEnvVar {
            name: "PYTHONDONTWRITEBYTECODE".to_string(),
            value: "1".to_string(),
        },
        RuntimeExtensionEnvVar {
            name: "PYTHONNOUSERSITE".to_string(),
            value: "1".to_string(),
        },
    ]
}

#[derive(Clone, Copy)]
enum CallBehavior {
    Respond,
    Ignore,
}

fn mock_mcp_dir() -> TempDir {
    tempfile::tempdir_in(std::env::current_dir().expect("current dir")).expect("mcp temp dir")
}

fn mock_mcp_script(
    dir: &std::path::Path,
    tool_name: &str,
    call_behavior: CallBehavior,
) -> std::path::PathBuf {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let script = dir.join("mock-mcp.py");
    let respond_to_calls = if matches!(call_behavior, CallBehavior::Respond) {
        "True"
    } else {
        "False"
    };
    fs::write(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, sys
tool = {tool_name:?}
respond_to_calls = {respond_to_calls}
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "notifications/initialized":
        continue
    if method == "initialize":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"protocolVersion":"2025-03-26","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"mock","version":"1.0.0"}}}}}}), flush=True)
    elif method == "tools/list":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"tools":[{{"name":tool,"description":"mock","inputSchema":{{"type":"object"}}}}]}}}}), flush=True)
    elif method == "tools/call":
        if respond_to_calls:
            print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"structuredContent":{{"result":msg["params"]["name"],"arguments":msg["params"].get("arguments", {{}})}},"isError":False}}}}), flush=True)
"#
        ),
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");
    script
}

async fn drop_mount_with_timeout(mount: McpToolRegistry) {
    let drop_task = tokio::task::spawn_blocking(move || drop(mount));
    tokio::time::timeout(MCP_TEST_CLEANUP_TIMEOUT, drop_task)
        .await
        .expect("dropping MCP mount timed out")
        .expect("MCP mount drop task panicked");
}
