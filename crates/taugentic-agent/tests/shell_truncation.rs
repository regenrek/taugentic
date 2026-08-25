#![cfg(target_os = "macos")]

mod tool_support;

use std::error::Error;
use std::time::Duration;

use serde_json::json;
use taugentic_agent::tools::{ShellTool, Tool, ToolContext};
use tempfile::tempdir;

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn shell_truncates_stdout_at_line_limit() -> TestResult {
    let dir = tempdir()?;

    let output = ShellTool
        .run(
            json!({
                "command": "i=1; while [ $i -le 3000 ]; do echo \"line-$i\"; i=$((i + 1)); done",
                "timeout_ms": 5000
            }),
            context(dir.path()),
        )
        .await?;

    let stdout = output.content["stdout"].as_str().ok_or("missing stdout")?;
    assert_eq!(stdout.lines().count(), 2000);
    assert_eq!(output.content["truncated"], json!(true));
    assert_eq!(output.content["truncated_by"], json!("Lines"));
    Ok(())
}

#[tokio::test]
async fn shell_truncates_stdout_at_byte_limit() -> TestResult {
    let dir = tempdir()?;

    let output = ShellTool
        .run(
            json!({
                "command": "i=0; while [ $i -lt 40000 ]; do printf x; i=$((i + 1)); done",
                "timeout_ms": 5000
            }),
            context(dir.path()),
        )
        .await?;

    let stdout = output.content["stdout"].as_str().ok_or("missing stdout")?;
    assert_eq!(stdout.len(), 30 * 1024);
    assert!(stdout.lines().count() < 2000);
    assert_eq!(output.content["truncated"], json!(true));
    assert_eq!(output.content["truncated_by"], json!("Bytes"));
    Ok(())
}

fn context(path: &std::path::Path) -> ToolContext {
    tool_support::context(path, Duration::from_secs(30))
}
