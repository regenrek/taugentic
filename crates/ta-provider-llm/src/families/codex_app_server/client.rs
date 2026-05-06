use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ta_exec::{ExecEngine, LocalExecEngine, SpawnRequest, StdioPolicy};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child as TokioChild, ChildStdin as TokioChildStdin};
use tokio::runtime::{Builder, Runtime};
use tokio_util::sync::CancellationToken;

use super::events::{CodexAppServerEvent, event_from_notification, required_string};
use super::launch::build_codex_perimeter_profile;
use super::process::{app_server_env, spawn_jsonl_reader, terminate_app_server};
use super::search_path::{default_binary, resolve_codex_binary};
use super::{
    CODEX_API_KEY_AUTH_PROFILE_ID, CODEX_CHATGPT_AUTH_PROFILE_ID, CodexLlmClientError,
    OPENAI_API_KEY_ENV_VAR,
};

const APP_SERVER_RECV_TICK: Duration = Duration::from_millis(50);

#[derive(Debug, Clone)]
pub struct CodexCli {
    binary: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCommandOutput {
    pub stdout: String,
    pub stderr: String,
}

impl Default for CodexCli {
    fn default() -> Self {
        Self {
            binary: default_binary(),
        }
    }
}

impl CodexCli {
    #[cfg(test)]
    pub(crate) fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub(crate) fn run(
        &self,
        args: &[&str],
        stdin: Option<&str>,
    ) -> Result<CodexCommandOutput, CodexLlmClientError> {
        self.run_with_timeout(args, stdin, None)
    }

    pub(crate) fn run_with_timeout(
        &self,
        args: &[&str],
        stdin: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<CodexCommandOutput, CodexLlmClientError> {
        let binary = resolve_codex_binary(&self.binary)?;
        let mut command = Command::new(&binary);
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stdin.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| {
            CodexLlmClientError::CliUnavailable(format!(
                "failed to spawn {}: {error}",
                binary.display()
            ))
        })?;
        if let Some(stdin_input) = stdin {
            let mut child_stdin = child.stdin.take().ok_or_else(|| {
                CodexLlmClientError::CommandFailed("codex command stdin was not piped".to_string())
            })?;
            child_stdin
                .write_all(stdin_input.as_bytes())
                .map_err(|error| {
                    CodexLlmClientError::CommandFailed(format!(
                        "failed to write codex command stdin: {error}"
                    ))
                })?;
        }

        let stdout_reader =
            spawn_output_reader(child.stdout.take(), "stdout", binary.display().to_string())?;
        let stderr_reader =
            spawn_output_reader(child.stderr.take(), "stderr", binary.display().to_string())?;

        let status = match timeout {
            Some(timeout) => match wait_for_exit(&mut child, timeout)? {
                Some(status) => status,
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(CodexLlmClientError::CommandTimedOut(format!(
                        "{} {} exceeded {}ms",
                        binary.display(),
                        args.join(" "),
                        timeout.as_millis(),
                    )));
                }
            },
            None => child.wait().map_err(|error| {
                CodexLlmClientError::CommandFailed(format!(
                    "failed to wait for codex command exit: {error}"
                ))
            })?,
        };

        let stdout = join_output_reader(stdout_reader, "stdout")?;
        let stderr = join_output_reader(stderr_reader, "stderr")?;
        let stdout = String::from_utf8_lossy(&stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
        if !status.success() {
            let detail = if stderr.is_empty() {
                stdout.clone()
            } else {
                stderr.clone()
            };
            return Err(CodexLlmClientError::CommandFailed(detail));
        }
        Ok(CodexCommandOutput { stdout, stderr })
    }
}

#[derive(Debug, Clone, Default)]
pub struct CodexAppServerClient {
    cli: CodexCli,
}

#[derive(Debug, Clone)]
pub struct CodexAppServerInput {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub auth_profile_id: Option<String>,
}

pub struct CodexAppServerSession {
    child: TokioChild,
    stdin: TokioChildStdin,
    messages: Receiver<Result<Value, CodexLlmClientError>>,
    reader_thread: Option<thread::JoinHandle<()>>,
    next_id: i64,
    thread_id: String,
    active_turn_id: Option<String>,
    runtime: Runtime,
}

impl CodexAppServerClient {
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            cli: CodexCli {
                binary: binary.into(),
            },
        }
    }

    #[tracing::instrument(skip(self, input), fields(cwd = %input.cwd.display()))]
    pub fn start_session(
        &self,
        input: CodexAppServerInput,
    ) -> Result<CodexAppServerSession, CodexLlmClientError> {
        validate_auth_profile(input.auth_profile_id.as_deref())?;
        let binary = resolve_codex_binary(&self.cli.binary)?;
        let sandbox_profile = build_codex_perimeter_profile(&input.cwd, &binary)?;
        let runtime = Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .thread_name("taugentic-codex-app-server-io")
            .build()
            .map_err(|error| {
                CodexLlmClientError::CommandFailed(format!(
                    "failed to create Codex app-server runtime: {error}"
                ))
            })?;
        let mut request = SpawnRequest::new(binary.clone().into_os_string())
            .args(["app-server", "--listen", "stdio://"])
            .cwd(input.cwd.clone())
            .stdin(StdioPolicy::Piped)
            .stdout(StdioPolicy::Piped)
            .stderr(StdioPolicy::Inherit)
            .sandbox_profile(sandbox_profile);
        for (name, value) in app_server_env(input.auth_profile_id.as_deref())? {
            request = request.env(name, value);
        }
        let mut child = {
            let _runtime_guard = runtime.enter();
            LocalExecEngine.spawn(request).map_err(|error| {
                CodexLlmClientError::CliUnavailable(format!(
                    "failed to spawn {} app-server: {error}",
                    binary.display()
                ))
            })?
        };
        let stdin = child.stdin.take().ok_or_else(|| {
            CodexLlmClientError::CommandFailed("codex app-server stdin was not piped".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CodexLlmClientError::CommandFailed("codex app-server stdout was not piped".to_string())
        })?;
        let (messages_tx, messages) = mpsc::channel();
        let reader_thread = spawn_jsonl_reader(stdout, runtime.handle().clone(), messages_tx)?;
        let mut session = CodexAppServerSession {
            child,
            stdin,
            messages,
            reader_thread: Some(reader_thread),
            next_id: 1,
            thread_id: String::new(),
            active_turn_id: None,
            runtime,
        };
        session.initialize()?;
        session.start_thread(input)?;
        Ok(session)
    }
}

impl CodexAppServerSession {
    #[tracing::instrument(skip(self, objective), fields(thread_id = %self.thread_id))]
    pub fn send_user_turn(&mut self, objective: &str) -> Result<(), CodexLlmClientError> {
        if objective.trim().is_empty() {
            return Err(CodexLlmClientError::InvalidConfig(
                "codex app-server objective must not be empty".to_string(),
            ));
        }
        let id = self.next_request_id();
        self.send(json!({
            "id": id,
            "method": "turn/start",
            "params": {
                "threadId": self.thread_id,
                "input": [{
                    "type": "text",
                    "text": objective,
                    "textElements": []
                }]
            }
        }))?;
        let result = self.read_until_response(id)?;
        let turn = result.get("turn").cloned().unwrap_or(Value::Null);
        self.active_turn_id = Some(required_string(&turn, "id")?);
        Ok(())
    }

    #[tracing::instrument(skip(self, on_event, cancellation), fields(thread_id = %self.thread_id))]
    pub fn stream_events(
        &mut self,
        cancellation: &CancellationToken,
        mut on_event: impl FnMut(CodexAppServerEvent) -> Result<(), CodexLlmClientError>,
    ) -> Result<(), CodexLlmClientError> {
        loop {
            if cancellation.is_cancelled() {
                if let Ok(id) = self.request_turn_interrupt() {
                    self.drain_interrupt_ack(id);
                }
                return Err(CodexLlmClientError::Cancelled(
                    "Codex app-server execution cancelled".to_string(),
                ));
            }
            match self.recv_message_tick()? {
                Some(message) => {
                    if let Some(error) = message.get("error") {
                        return Err(parse_json_rpc_error(error));
                    }
                    if let Some(method) = message.get("method").and_then(Value::as_str) {
                        if let Some(event) = event_from_notification(method, &message)? {
                            let done = matches!(event, CodexAppServerEvent::TurnCompleted { .. });
                            on_event(event)?;
                            if done {
                                return Ok(());
                            }
                        }
                    } else {
                        self.respond_to_server_request(&message)?;
                    }
                }
                None => self.ensure_child_running()?,
            }
        }
    }

    #[tracing::instrument(skip(self), fields(thread_id = %self.thread_id, turn_id = ?self.active_turn_id))]
    pub fn cancel_turn(&mut self) -> Result<(), CodexLlmClientError> {
        let id = self.request_turn_interrupt()?;
        if id == 0 {
            return Ok(());
        }
        let _ = self.read_until_response(id)?;
        Ok(())
    }

    fn request_turn_interrupt(&mut self) -> Result<i64, CodexLlmClientError> {
        let Some(turn_id) = self.active_turn_id.clone() else {
            return Ok(0);
        };
        let id = self.next_request_id();
        self.send(json!({
            "id": id,
            "method": "turn/interrupt",
            "params": {
                "threadId": self.thread_id,
                "turnId": turn_id
            }
        }))?;
        Ok(id)
    }

    fn drain_interrupt_ack(&mut self, expected_id: i64) {
        if expected_id == 0 {
            return;
        }
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            match self.recv_message_tick() {
                Ok(Some(message))
                    if message.get("id").and_then(Value::as_i64) == Some(expected_id) =>
                {
                    return;
                }
                Ok(Some(_)) | Ok(None) => {}
                Err(_) => return,
            }
        }
    }

    pub fn shutdown(&mut self) -> Result<(), CodexLlmClientError> {
        terminate_app_server(&mut self.child, &self.runtime)
    }

    fn initialize(&mut self) -> Result<(), CodexLlmClientError> {
        let id = self.next_request_id();
        self.send(json!({
            "id": id,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "taugentic",
                    "title": "Taugentic",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        }))?;
        let _ = self.read_until_response(id)?;
        self.send(json!({"method": "initialized"}))
    }

    fn start_thread(&mut self, input: CodexAppServerInput) -> Result<(), CodexLlmClientError> {
        let id = self.next_request_id();
        self.send(json!({
            "id": id,
            "method": "thread/start",
            "params": {
                "model": input.model,
                "cwd": input.cwd.to_string_lossy(),
                "ephemeral": true
            }
        }))?;
        let result = self.read_until_response(id)?;
        let thread = result.get("thread").ok_or_else(|| {
            CodexLlmClientError::Protocol("thread/start missing thread".to_string())
        })?;
        self.thread_id = required_string(thread, "id")?;
        Ok(())
    }

    fn next_request_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send(&mut self, message: Value) -> Result<(), CodexLlmClientError> {
        let mut line = serde_json::to_string(&message)
            .map_err(|error| CodexLlmClientError::Protocol(error.to_string()))?;
        line.push('\n');
        self.runtime
            .block_on(async {
                self.stdin.write_all(line.as_bytes()).await?;
                self.stdin.flush().await
            })
            .map_err(|error| CodexLlmClientError::CommandFailed(error.to_string()))
    }

    fn read_until_response(&mut self, expected_id: i64) -> Result<Value, CodexLlmClientError> {
        loop {
            let message = self.recv_message_blocking()?;
            if message.get("id").and_then(Value::as_i64) == Some(expected_id) {
                if let Some(error) = message.get("error") {
                    return Err(parse_json_rpc_error(error));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            if message.get("id").is_some() && message.get("method").is_none() {
                return Err(CodexLlmClientError::Protocol(format!(
                    "received response for unexpected codex app-server request id: {}",
                    message.get("id").cloned().unwrap_or(Value::Null)
                )));
            }
            self.respond_to_server_request(&message)?;
        }
    }

    fn recv_message_blocking(&mut self) -> Result<Value, CodexLlmClientError> {
        loop {
            match self.messages.recv_timeout(APP_SERVER_RECV_TICK) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Timeout) => self.ensure_child_running()?,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(CodexLlmClientError::CommandFailed(
                        "codex app-server closed stdout".to_string(),
                    ));
                }
            }
        }
    }

    fn recv_message_tick(&mut self) -> Result<Option<Value>, CodexLlmClientError> {
        match self.messages.recv_timeout(APP_SERVER_RECV_TICK) {
            Ok(result) => result.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(CodexLlmClientError::CommandFailed(
                "codex app-server closed stdout".to_string(),
            )),
        }
    }

    fn ensure_child_running(&mut self) -> Result<(), CodexLlmClientError> {
        match self.child.try_wait().map_err(|error| {
            CodexLlmClientError::CommandFailed(format!(
                "failed to poll codex app-server status: {error}"
            ))
        })? {
            Some(status) => Err(CodexLlmClientError::CommandFailed(format!(
                "codex app-server exited with {status}"
            ))),
            None => Ok(()),
        }
    }

    fn respond_to_server_request(&mut self, message: &Value) -> Result<(), CodexLlmClientError> {
        let Some(id) = message.get("id").cloned() else {
            return Ok(());
        };
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(());
        };
        self.send(json!({
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("taugentic delegates codex app-server request {method} to vendor protocol")
            }
        }))
    }
}

impl Drop for CodexAppServerSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
    }
}

fn parse_json_rpc_error(error: &Value) -> CodexLlmClientError {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("codex app-server JSON-RPC error")
        .to_string();
    let data = error.get("data").cloned();
    match code {
        -32001 => CodexLlmClientError::RateLimited {
            retry_after_ms: None,
            detail: message,
        },
        -32602 => CodexLlmClientError::InvalidConfig(message),
        _ if message.to_ascii_lowercase().contains("unauthorized") => {
            CodexLlmClientError::Auth(message)
        }
        _ if message.to_ascii_lowercase().contains("context") => {
            CodexLlmClientError::ContextLengthExceeded(message)
        }
        _ => CodexLlmClientError::JsonRpc {
            code,
            message,
            data,
        },
    }
}

fn validate_auth_profile(auth_profile_id: Option<&str>) -> Result<(), CodexLlmClientError> {
    match auth_profile_id {
        Some(CODEX_API_KEY_AUTH_PROFILE_ID) if env::var_os(OPENAI_API_KEY_ENV_VAR).is_none() => {
            Err(CodexLlmClientError::MissingApiKeyEnv)
        }
        Some(CODEX_API_KEY_AUTH_PROFILE_ID | CODEX_CHATGPT_AUTH_PROFILE_ID) | None => Ok(()),
        Some(other) => Err(CodexLlmClientError::UnknownAuthProfile(other.to_string())),
    }
}

fn wait_for_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<Option<ExitStatus>, CodexLlmClientError> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().map_err(|error| {
            CodexLlmClientError::CommandFailed(format!(
                "failed to poll codex command status: {error}"
            ))
        })? {
            Some(status) => return Ok(Some(status)),
            None if Instant::now() >= deadline => return Ok(None),
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn spawn_output_reader<R: Read + Send + 'static>(
    mut reader: Option<R>,
    stream_name: &'static str,
    binary_display: String,
) -> Result<thread::JoinHandle<Result<Vec<u8>, CodexLlmClientError>>, CodexLlmClientError> {
    let mut reader = reader.take().ok_or_else(|| {
        CodexLlmClientError::CommandFailed(format!(
            "{binary_display} command {stream_name} was not piped"
        ))
    })?;
    Ok(thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(|error| {
            CodexLlmClientError::CommandFailed(format!(
                "failed to read codex command {stream_name}: {error}"
            ))
        })?;
        Ok(bytes)
    }))
}

fn join_output_reader(
    handle: thread::JoinHandle<Result<Vec<u8>, CodexLlmClientError>>,
    stream_name: &'static str,
) -> Result<Vec<u8>, CodexLlmClientError> {
    handle.join().map_err(|_| {
        CodexLlmClientError::CommandFailed(format!(
            "codex command {stream_name} reader thread panicked"
        ))
    })?
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        PathBuf::from("/tmp")
            .join("ta-provider-llm-test-artifacts")
            .join(format!("{name}-{suffix}"))
    }

    #[cfg(unix)]
    fn write_script(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let dir = unique_dir(name);
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("codex");
        fs::write(&path, body).expect("script");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("permissions");
        path
    }

    #[test]
    #[cfg(unix)]
    fn run_passes_stdin_to_codex_command() {
        let binary = write_script(
            "stdin",
            "#!/bin/sh\nread value\nprintf 'stdin:%s\\n' \"$value\"\n",
        );
        let cli = CodexCli::with_binary(binary);

        let output = cli
            .run(&["login", "--with-api-key"], Some("sk-test"))
            .expect("output");

        assert_eq!(output.stdout, "stdin:sk-test");
    }

    #[test]
    #[cfg(unix)]
    fn run_surfaces_non_zero_exit_as_command_failure() {
        let binary = write_script("failure", "#!/bin/sh\necho 'boom' 1>&2\nexit 7\n");
        let cli = CodexCli::with_binary(binary);

        let error = cli
            .run(&["login", "status"], None)
            .expect_err("should fail");

        assert!(matches!(error, CodexLlmClientError::CommandFailed(message) if message == "boom"));
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_kills_stuck_status_probe() {
        let binary = write_script("timeout", "#!/bin/sh\nsleep 5\n");
        let cli = CodexCli::with_binary(binary);

        let error = cli
            .run_with_timeout(&["login", "status"], None, Some(Duration::from_millis(50)))
            .expect_err("should time out");

        assert!(
            matches!(error, CodexLlmClientError::CommandTimedOut(message) if message.contains("login status"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_returns_fast_successful_status_probe() {
        let binary = write_script(
            "fast-success",
            "#!/bin/sh\nprintf 'Logged in using ChatGPT\\n'\nexit 0\n",
        );
        let cli = CodexCli::with_binary(binary);

        let output = cli
            .run_with_timeout(&["login", "status"], None, Some(Duration::from_millis(200)))
            .expect("fast successful command should not be degraded");

        assert_eq!(output.stdout, "Logged in using ChatGPT");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn json_rpc_error_response_maps_to_typed_error() {
        let error = parse_json_rpc_error(&json!({
            "code": -32001,
            "message": "Server overloaded; retry later.",
            "data": {"kind": "overloaded"}
        }));
        assert!(matches!(error, CodexLlmClientError::RateLimited { .. }));
    }

    #[test]
    fn json_rpc_id_correlation_rejects_unexpected_response() {
        let binary = write_script(
            "bad-id",
            r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "initialize":
        print(json.dumps({"id": 999, "result": {}}), flush=True)
        sys.exit(0)
"#,
        );
        let client = CodexAppServerClient::with_binary(binary);
        let result = client.start_session(CodexAppServerInput {
            cwd: env::current_dir().expect("current dir"),
            model: None,
            auth_profile_id: None,
        });
        let Err(error) = result else {
            panic!("unexpected response id should fail");
        };
        assert!(
            matches!(error, CodexLlmClientError::Protocol(_)),
            "unexpected error: {error:?}"
        );
    }
}
