mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal;
use nix::unistd::Pid;
use ta_protocol::wire::{
    RuntimeExtensionAvailability, RuntimeExtensionDescriptor, RuntimeExtensionEnvVar,
    RuntimeExtensionId, RuntimeExtensionMcpServer, RuntimeExtensionMcpStdioServer,
    RuntimeExtensionState,
};
use taugentic_agent::mcp::McpToolRegistry;
use taugentic_agent::tools::Registry;

const MCP_UNMOUNT_TEST_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_UNMOUNT_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[tokio::test]
async fn dropping_mcp_mount_disconnects_stdio_process() {
    let dir = tempfile::tempdir_in(std::env::current_dir().expect("current dir")).expect("dir");
    let pid_file = dir.path().join("pid");
    let script = mock_mcp_script(&pid_file);

    let mut registry = Registry::new();
    let mut request = support::request();
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    request.runtime_extensions = vec![extension("srv1", script)];
    let mount = McpToolRegistry::mount_from_request(&mut registry, &request)
        .await
        .expect("mount");
    let pid = wait_for_pid(&pid_file).await;

    drop_mount_with_timeout(mount).await;

    wait_for("MCP stdio process to exit", || !process_exists(pid)).await;
}

#[tokio::test]
async fn mcp_stdio_spec_env_is_secret_bridge_without_parent_secret_inheritance() {
    const CHILD_MARKER: &str = "TAUGENTIC_AGENT_MCP_SPEC_ENV_SECRET_BRIDGE_CHILD";

    if std::env::var_os(CHILD_MARKER).is_none() {
        let test_name = "mcp_stdio_spec_env_is_secret_bridge_without_parent_secret_inheritance";
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env("OPENAI_API_KEY", "parent-secret")
            .output()
            .expect("run isolated MCP env bridge test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "isolated MCP env bridge test failed\nstdout:\n{}\nstderr:\n{}",
            stdout,
            stderr
        );
        return;
    }

    let dir = tempfile::tempdir_in(std::env::current_dir().expect("current dir")).expect("dir");
    let pid_file = dir.path().join("pid");
    let env_file = dir.path().join("env");
    let script = mock_mcp_env_script(&pid_file, &env_file);

    let mut registry = Registry::new();
    let mut request = support::request();
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    request.runtime_extensions = vec![extension_with_env(
        "srv1",
        script,
        vec![RuntimeExtensionEnvVar {
            name: "GITHUB_TOKEN".to_string(),
            value: "secret".to_string(),
        }],
    )];
    let mount = McpToolRegistry::mount_from_request(&mut registry, &request)
        .await
        .expect("mount");
    let pid = wait_for_pid(&pid_file).await;

    wait_for("MCP env snapshot", || env_file.exists()).await;
    let env_snapshot = fs::read_to_string(&env_file).expect("env snapshot");
    assert!(env_snapshot.contains("GITHUB_TOKEN=secret\n"));
    assert!(env_snapshot.contains("OPENAI_API_KEY=<unset>\n"));

    drop_mount_with_timeout(mount).await;

    wait_for("MCP env stdio process to exit", || !process_exists(pid)).await;
}

fn extension(id: &str, command: std::path::PathBuf) -> RuntimeExtensionState {
    extension_with_env(id, command, Vec::new())
}

fn extension_with_env(
    id: &str,
    command: std::path::PathBuf,
    env: Vec<RuntimeExtensionEnvVar>,
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
                command: command.to_string_lossy().to_string(),
                args: Vec::new(),
                env,
            },
        )),
    }
}

fn mock_mcp_script(pid_file: &std::path::Path) -> std::path::PathBuf {
    let dir = pid_file.parent().expect("pid parent");
    let script = dir.join("mock-mcp.py");
    fs::write(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, os, sys
open({:?}, "w").write(str(os.getpid()))
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"protocolVersion":"2025-03-26","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"mock","version":"1.0.0"}}}}}}), flush=True)
    elif method == "tools/list":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"tools":[]}}}}), flush=True)
"#,
            pid_file
        ),
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");
    script
}

fn mock_mcp_env_script(
    pid_file: &std::path::Path,
    env_file: &std::path::Path,
) -> std::path::PathBuf {
    let dir = pid_file.parent().expect("pid parent");
    let script = dir.join("mock-mcp-env.py");
    fs::write(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, os, sys
open({:?}, "w").write("GITHUB_TOKEN=" + os.environ.get("GITHUB_TOKEN", "<unset>") + "\n" + "OPENAI_API_KEY=" + os.environ.get("OPENAI_API_KEY", "<unset>") + "\n")
open({:?}, "w").write(str(os.getpid()))
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"protocolVersion":"2025-03-26","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"mock","version":"1.0.0"}}}}}}), flush=True)
    elif method == "tools/list":
        print(json.dumps({{"jsonrpc":"2.0","id":msg["id"],"result":{{"tools":[]}}}}), flush=True)
"#,
            env_file, pid_file
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
    tokio::time::timeout(MCP_UNMOUNT_TEST_TIMEOUT, drop_task)
        .await
        .expect("dropping MCP mount timed out")
        .expect("MCP mount drop task panicked");
}

async fn wait_for_pid(pid_file: &std::path::Path) -> Pid {
    let mut pid = None;
    wait_for("MCP stdio pid file", || {
        pid = fs::read_to_string(pid_file)
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .map(Pid::from_raw);
        pid.is_some()
    })
    .await;
    pid.expect("pid")
}

fn process_exists(pid: Pid) -> bool {
    match signal::kill(pid, None) {
        Ok(()) => true,
        Err(Errno::ESRCH) => false,
        Err(_) => true,
    }
}

async fn wait_for(label: &str, mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(MCP_UNMOUNT_TEST_TIMEOUT, async {
        loop {
            if condition() {
                return;
            }
            tokio::time::sleep(MCP_UNMOUNT_POLL_INTERVAL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {label}"));
}
