#![cfg(target_os = "macos")]

use std::error::Error;
use std::time::{Duration, Instant};

use serde_json::json;
use taugentic_agent::tools::{ShellTool, Tool, ToolContext};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

type TestResult = Result<(), Box<dyn Error>>;

#[tokio::test]
async fn shell_timeout_kills_command_within_grace() -> TestResult {
    let dir = tempdir()?;
    let started = Instant::now();

    let output = ShellTool
        .run(
            json!({ "command": "sleep 5", "timeout_ms": 500 }),
            context(dir.path(), CancellationToken::new()),
        )
        .await?;

    assert!(started.elapsed() < Duration::from_secs(3));
    assert_ne!(output.content["exit_code"], json!(0));
    Ok(())
}

fn context(path: &std::path::Path, cancellation_token: CancellationToken) -> ToolContext {
    ToolContext {
        workdir: path.to_path_buf(),
        cancellation_token,
        timeout: Duration::from_secs(30),
        parent_turn_id: None,
    }
}
