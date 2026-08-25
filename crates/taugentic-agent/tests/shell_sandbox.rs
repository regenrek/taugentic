use std::error::Error;
use std::path::Path;
use std::time::Duration;

use serde_json::json;
#[cfg(target_os = "linux")]
use taugentic_agent::ExecutionError;
use taugentic_agent::tools::{ShellTool, Tool, ToolContext};
use tempfile::tempdir;

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn shell_sandbox_unsupported_backend_fails_closed() -> TestResult {
    const CHILD_MARKER: &str = "TA_TEST_SHELL_SANDBOX_UNSUPPORTED_CHILD";

    if std::env::var_os(CHILD_MARKER).is_none() {
        let test_name = "shell_sandbox_unsupported_backend_fails_closed";
        let output = std::process::Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env("TA_LINUX_SANDBOX_HELPER", "/nonexistent/ta-linux-sandbox")
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "isolated unsupported sandbox test failed\nstdout:\n{}\nstderr:\n{}",
            stdout,
            stderr
        );
        return Ok(());
    }

    let dir = tempdir()?;

    let error = ShellTool
        .run(
            json!({ "command": "echo should-not-run" }),
            context(dir.path()),
        )
        .await
        .expect_err("unsupported sandbox backend should fail before spawn");

    assert!(matches!(
        error,
        ExecutionError::Unsupported(message)
            if message.contains("shell sandbox backend is unsupported")
    ));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn shell_sandbox_denies_home_ssh_read() -> TestResult {
    let home = tempdir()?;
    let workdir = tempdir()?;
    let secret_path = home.path().join(".ssh/taugentic-shell-sandbox-secret");
    std::fs::create_dir_all(secret_path.parent().ok_or("missing secret parent")?)?;
    std::fs::write(&secret_path, "super-secret")?;

    let output = std::process::Command::new(std::env::current_exe()?)
        .arg("--ignored")
        .arg("--exact")
        .arg("shell_sandbox_denies_home_ssh_read_helper")
        .env("HOME", home.path())
        .env("TA_TEST_WORKDIR", workdir.path())
        .output()?;

    assert!(
        output.status.success(),
        "sandbox denial helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "spawned with controlled HOME by shell_sandbox_denies_home_ssh_read"]
async fn shell_sandbox_denies_home_ssh_read_helper() -> TestResult {
    let workdir = std::env::var_os("TA_TEST_WORKDIR").ok_or("TA_TEST_WORKDIR must be set")?;
    let output = ShellTool
        .run(
            json!({
                "command": "cat \"$HOME/.ssh/taugentic-shell-sandbox-secret\"",
                "timeout_ms": 5000
            }),
            context(Path::new(&workdir)),
        )
        .await?;

    assert_ne!(output.content["exit_code"], json!(0));
    let stdout = output.content["stdout"].as_str().ok_or("missing stdout")?;
    assert!(!stdout.contains("super-secret"));
    Ok(())
}

fn context(path: &Path) -> ToolContext {
    tool_support::context(path, Duration::from_secs(30))
}
mod tool_support;
