#![cfg(target_os = "macos")]

mod tool_support;

use std::error::Error;
use std::time::{Duration, Instant};

use serde_json::json;
use taugentic_agent::tools::{ShellTool, Tool};
use tempfile::tempdir;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn shell_cancel_kills_process_group() -> TestResult {
    let dir = tempdir()?;
    let cancellation = CancellationToken::new();
    let ctx = tool_support::context_with_cancellation(
        dir.path(),
        Duration::from_secs(30),
        cancellation.clone(),
    );

    let handle = tokio::spawn(async move {
        ShellTool
            .run(
                json!({
                    "command": "sleep 30 & echo $! > grandchild.pid; wait",
                    "timeout_ms": 30000
                }),
                ctx,
            )
            .await
    });

    let pid_file = dir.path().join("grandchild.pid");
    let grandchild_pid = wait_for_pid_file(&pid_file).await?;
    let pgid = process_group_id(grandchild_pid).await?;
    cancellation.cancel();
    let output = handle.await??;

    assert_ne!(output.content["exit_code"], json!(0));
    assert_process_group_exits(pgid).await?;
    Ok(())
}

async fn wait_for_pid_file(path: &std::path::Path) -> Result<i32, Box<dyn Error + Send + Sync>> {
    let started = Instant::now();
    loop {
        if let Ok(content) = std::fs::read_to_string(path) {
            return Ok(content.trim().parse()?);
        }
        if started.elapsed() > Duration::from_secs(2) {
            return Err("grandchild pid file was not written".into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn process_group_id(pid: i32) -> Result<i32, Box<dyn Error + Send + Sync>> {
    let output = Command::new("ps")
        .arg("-o")
        .arg("pgid=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .await?;
    if !output.status.success() {
        return Err(format!("failed to inspect pgid for pid {pid}").into());
    }
    let pgid = String::from_utf8(output.stdout)?;
    Ok(pgid.trim().parse()?)
}

async fn assert_process_group_exits(pgid: i32) -> Result<(), Box<dyn Error + Send + Sync>> {
    let started = Instant::now();
    loop {
        let output = Command::new("ps")
            .arg("-o")
            .arg("pgid=")
            .arg("-o")
            .arg("pid=")
            .arg("-o")
            .arg("stat=")
            .arg("-g")
            .arg(pgid.to_string())
            .output()
            .await?;
        if output.stdout.is_empty() {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(3) {
            return Err(format!(
                "process group {pgid} survived cancellation:\n{}",
                String::from_utf8_lossy(&output.stdout)
            )
            .into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
