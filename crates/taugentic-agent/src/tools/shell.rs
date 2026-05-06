use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ta_exec::{
    ExecEngine, ExecError, LocalExecEngine, NetworkPolicy, SandboxProfile, SpawnRequest,
};
use ta_protocol::wire::ApprovalScope;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::ExecutionError;
use crate::tools::{Tool, ToolContext, ToolDescriptor, ToolOutput};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const CANCEL_GRACE_PERIOD: Duration = Duration::from_secs(2);
const MAX_OUTPUT_LINES: usize = 2_000;
const MAX_OUTPUT_BYTES: usize = 30 * 1024;
const DEFAULT_SANDBOX_ENV_ALLOWLIST: &[&str] = &["PATH", "HOME", "LANG"];

#[derive(Default)]
pub struct ShellTool;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellInput {
    command: String,
    timeout_ms: Option<u64>,
    cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub truncated: bool,
    pub truncated_by: Option<TruncatedBy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruncatedBy {
    Lines,
    Bytes,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &str {
        "Run a shell command in the current workdir."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": { "type": "string" },
                "timeout_ms": { "type": "integer", "minimum": 1 },
                "cwd": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
            approval_scope: Some(ApprovalScope::ProcessExec),
            read_only: false,
            parallel_safe: false,
        }
    }

    #[tracing::instrument(
        name = "tool.shell.run",
        skip_all,
        fields(tool = "shell", workdir = %ctx.workdir.display())
    )]
    async fn run(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput, ExecutionError> {
        let input: ShellInput = serde_json::from_value(input)
            .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
        if input.command.trim().is_empty() {
            return Err(ExecutionError::InvalidToolInput(
                "command must be non-empty".to_string(),
            ));
        }

        let timeout = input
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or_else(|| {
                if ctx.timeout.is_zero() {
                    DEFAULT_TIMEOUT
                } else {
                    ctx.timeout
                }
            });
        let cwd = resolve_cwd(&ctx.workdir, input.cwd.as_deref())?;
        let started = Instant::now();
        let mut child = spawn_shell(&input.command, &cwd)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ExecutionError::ToolFailed("stdout pipe missing".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ExecutionError::ToolFailed("stderr pipe missing".to_string()))?;
        let capture = Arc::new(Mutex::new(OutputCapture::default()));
        let stdout_task = tokio::spawn(capture_pipe(stdout, StreamKind::Stdout, capture.clone()));
        let stderr_task = tokio::spawn(capture_pipe(stderr, StreamKind::Stderr, capture.clone()));

        let status = tokio::select! {
            status = child.wait() => status.map_err(|error| ExecutionError::ToolFailed(error.to_string()))?,
            () = tokio::time::sleep(timeout) => terminate_child(&mut child).await?,
            () = ctx.cancellation_token.cancelled() => terminate_child(&mut child).await?,
        };

        stdout_task
            .await
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
        stderr_task
            .await
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?
            .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;

        let capture = capture.lock().await;
        let result = ShellResult {
            stdout: capture.stdout.clone(),
            stderr: capture.stderr.clone(),
            exit_code: status.code(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            truncated: capture.truncated,
            truncated_by: capture.truncated_by,
        };

        Ok(ToolOutput {
            content: serde_json::to_value(result)
                .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?,
        })
    }
}

fn resolve_cwd(workdir: &Path, cwd: Option<&Path>) -> Result<PathBuf, ExecutionError> {
    let workdir = workdir
        .canonicalize()
        .map_err(|error| ExecutionError::ToolFailed(error.to_string()))?;
    let requested = cwd
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                workdir.join(path)
            }
        })
        .unwrap_or_else(|| workdir.clone());
    let requested = requested
        .canonicalize()
        .map_err(|error| ExecutionError::InvalidToolInput(error.to_string()))?;
    if !requested.starts_with(&workdir) {
        return Err(ExecutionError::InvalidToolInput(
            "cwd must stay inside the workdir".to_string(),
        ));
    }
    Ok(requested)
}

fn spawn_shell(command: &str, cwd: &Path) -> Result<Child, ExecutionError> {
    let request =
        SpawnRequest::shell(command, cwd).sandbox_profile(native_shell_sandbox_profile(cwd));

    LocalExecEngine.spawn(request).map_err(map_exec_error)
}

fn native_shell_sandbox_profile(cwd: &Path) -> SandboxProfile {
    DEFAULT_SANDBOX_ENV_ALLOWLIST.iter().fold(
        SandboxProfile::new()
            .read_path(cwd)
            .write_path(cwd)
            .network(NetworkPolicy::Off)
            .child_inherits_tty(false),
        |profile, name| profile.env(*name),
    )
}

fn map_exec_error(error: ExecError) -> ExecutionError {
    match error {
        ExecError::Sandbox(ta_exec::SandboxError::Unsupported { kind, reason }) => {
            ExecutionError::Unsupported(format!(
                "shell sandbox backend is unsupported (kind: {kind}): {reason}"
            ))
        }
        other => ExecutionError::ToolFailed(other.to_string()),
    }
}

async fn terminate_child(child: &mut Child) -> Result<std::process::ExitStatus, ExecutionError> {
    ta_exec::terminate_child_tree(child, CANCEL_GRACE_PERIOD)
        .await
        .map_err(|error| ExecutionError::ToolFailed(error.to_string()))
}

#[derive(Debug, Default)]
struct OutputCapture {
    stdout: String,
    stderr: String,
    lines: usize,
    bytes: usize,
    truncated: bool,
    truncated_by: Option<TruncatedBy>,
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

async fn capture_pipe<R>(
    mut reader: R,
    stream: StreamKind,
    capture: Arc<Mutex<OutputCapture>>,
) -> Result<(), std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            return Ok(());
        }
        capture.lock().await.append(stream, &buffer[..read]);
    }
}

impl OutputCapture {
    fn append(&mut self, stream: StreamKind, bytes: &[u8]) {
        let accepted = self.accepted_prefix(bytes);
        if accepted.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(accepted);
        match stream {
            StreamKind::Stdout => self.stdout.push_str(&text),
            StreamKind::Stderr => self.stderr.push_str(&text),
        }
        self.bytes += accepted.len();
        self.lines += accepted.iter().filter(|byte| **byte == b'\n').count();
    }

    fn accepted_prefix<'a>(&mut self, bytes: &'a [u8]) -> &'a [u8] {
        if self.truncated {
            return &[];
        }
        if self.lines >= MAX_OUTPUT_LINES {
            self.mark_truncated(TruncatedBy::Lines);
            return &[];
        }
        if self.bytes >= MAX_OUTPUT_BYTES {
            self.mark_truncated(TruncatedBy::Bytes);
            return &[];
        }

        let byte_limit = MAX_OUTPUT_BYTES - self.bytes;
        let byte_cut = (bytes.len() > byte_limit).then_some(byte_limit);
        let line_cut = line_cut_position(bytes, MAX_OUTPUT_LINES - self.lines);
        let (end, truncated_by) = match (line_cut, byte_cut) {
            (Some(line), Some(byte)) if line <= byte => (line, TruncatedBy::Lines),
            (Some(_line), Some(byte)) => (byte, TruncatedBy::Bytes),
            (Some(line), None) => (line, TruncatedBy::Lines),
            (None, Some(byte)) => (byte, TruncatedBy::Bytes),
            (None, None) => (bytes.len(), TruncatedBy::Bytes),
        };
        if end < bytes.len() {
            self.mark_truncated(truncated_by);
        }
        &bytes[..end]
    }

    fn mark_truncated(&mut self, truncated_by: TruncatedBy) {
        if !self.truncated {
            self.truncated = true;
            self.truncated_by = Some(truncated_by);
        }
    }
}

fn line_cut_position(bytes: &[u8], remaining_lines: usize) -> Option<usize> {
    if remaining_lines == 0 {
        return Some(0);
    }
    let mut seen = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            seen += 1;
            if seen == remaining_lines && index + 1 < bytes.len() {
                return Some(index + 1);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_shell_sandbox_profile_is_fail_closed_by_default() {
        let cwd = Path::new("/repo");
        let profile = native_shell_sandbox_profile(cwd);

        assert_eq!(profile.network_policy(), &NetworkPolicy::Off);
        assert!(profile.reads_path(cwd));
        assert!(profile.writes_path(cwd));
        assert!(profile.allows_env("PATH"));
        assert!(profile.allows_env("HOME"));
        assert!(profile.allows_env("LANG"));
        assert!(!profile.allows_env("OPENAI_API_KEY"));
        assert!(!profile.child_inherits_tty_enabled());
    }

    #[test]
    fn sandbox_unsupported_maps_to_clear_tool_error() {
        let error = map_exec_error(ExecError::Sandbox(ta_exec::SandboxError::Unsupported {
            kind: ta_exec::SandboxKind::Unsupported,
            reason: "missing backend",
        }));

        assert!(matches!(
            error,
            ExecutionError::Unsupported(message)
                if message.contains("shell sandbox backend is unsupported")
                    && message.contains("missing backend")
        ));
    }
}
