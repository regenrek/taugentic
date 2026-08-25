use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ta_exec::{ExecEngine, LocalExecEngine, ProcessGroupPolicy, SpawnRequest, StdioPolicy};
use ta_protocol::wire::ExecutionContext;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child as TokioChild, ChildStdin as TokioChildStdin};
use tokio::runtime::{Builder, Runtime};
use tokio_util::sync::CancellationToken;

use super::events::{CodexAppServerEvent, event_from_notification, required_string};
use super::launch::build_codex_perimeter_profile_for_context;
use super::policy::CodexTurnPolicy;
use super::process::{app_server_env, spawn_jsonl_reader, terminate_app_server};
use super::search_path::{default_binary, resolve_codex_binary};
use super::{
    CODEX_API_KEY_AUTH_PROFILE_ID, CODEX_CHATGPT_AUTH_PROFILE_ID, CodexLlmClientError,
    OPENAI_API_KEY_ENV_VAR,
};

const APP_SERVER_RECV_TICK: Duration = Duration::from_millis(50);
const APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const APP_SERVER_TURN_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

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

#[derive(Debug, Clone)]
pub struct CodexAppServerClient {
    cli: CodexCli,
    turn_idle_timeout: Duration,
}

impl Default for CodexAppServerClient {
    fn default() -> Self {
        Self {
            cli: CodexCli::default(),
            turn_idle_timeout: APP_SERVER_TURN_IDLE_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexAppServerInput {
    pub execution_context: Arc<ExecutionContext>,
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
    turn_policy: Option<CodexTurnPolicy>,
    turn_idle_timeout: Duration,
    runtime: Runtime,
}

impl CodexAppServerClient {
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            cli: CodexCli {
                binary: binary.into(),
            },
            turn_idle_timeout: APP_SERVER_TURN_IDLE_TIMEOUT,
        }
    }

    #[doc(hidden)]
    pub fn with_binary_and_turn_idle_timeout(
        binary: impl Into<PathBuf>,
        turn_idle_timeout: Duration,
    ) -> Self {
        Self {
            cli: CodexCli {
                binary: binary.into(),
            },
            turn_idle_timeout,
        }
    }

    #[tracing::instrument(skip(self, input), fields(cwd = %input.execution_context.effective_cwd.as_str()))]
    pub fn start_session(
        &self,
        input: CodexAppServerInput,
    ) -> Result<CodexAppServerSession, CodexLlmClientError> {
        validate_auth_profile(input.auth_profile_id.as_deref())?;
        let binary = resolve_codex_binary(&self.cli.binary)?;
        let turn_policy = CodexTurnPolicy::from_execution_context(&input.execution_context)?;
        let sandbox_profile =
            build_codex_perimeter_profile_for_context(&input.execution_context, &binary)?;
        let request = SpawnRequest::new(binary.clone().into_os_string())
            .args(["app-server", "--listen", "stdio://"])
            .cwd(
                input
                    .execution_context
                    .effective_cwd
                    .as_path()
                    .to_path_buf(),
            )
            .stdin(StdioPolicy::Piped)
            .stdout(StdioPolicy::Piped)
            .stderr(StdioPolicy::Inherit)
            .process_group(ProcessGroupPolicy::New)
            .sandbox_profile(sandbox_profile);
        let mut session = self.spawn_app_server(request, input.auth_profile_id.as_deref())?;
        session.turn_policy = Some(turn_policy);
        session.turn_idle_timeout = self.turn_idle_timeout;
        session.start_thread(input)?;
        Ok(session)
    }

    pub(super) fn start_control_session(
        &self,
    ) -> Result<CodexAppServerSession, CodexLlmClientError> {
        let binary = resolve_codex_binary(&self.cli.binary)?;
        let request = SpawnRequest::new(binary.into_os_string())
            .args(["app-server", "--listen", "stdio://"])
            .stdin(StdioPolicy::Piped)
            .stdout(StdioPolicy::Piped)
            .stderr(StdioPolicy::Inherit)
            .process_group(ProcessGroupPolicy::New);
        self.spawn_app_server(request, None)
    }

    fn spawn_app_server(
        &self,
        mut request: SpawnRequest,
        auth_profile_id: Option<&str>,
    ) -> Result<CodexAppServerSession, CodexLlmClientError> {
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
        for (name, value) in app_server_env(auth_profile_id)? {
            request = request.env(name, value);
        }
        let mut child = {
            let _runtime_guard = runtime.enter();
            LocalExecEngine.spawn(request).map_err(|error| {
                CodexLlmClientError::CliUnavailable(format!(
                    "failed to spawn codex app-server: {error}"
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
            turn_policy: None,
            turn_idle_timeout: APP_SERVER_TURN_IDLE_TIMEOUT,
            runtime,
        };
        session.initialize()?;
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
        let turn_policy = self.turn_policy.clone().ok_or_else(|| {
            CodexLlmClientError::InvalidConfig(
                "Codex execution session is missing its compiled turn policy".to_string(),
            )
        })?;
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
                }],
                "approvalPolicy": turn_policy.approval_policy,
                "sandboxPolicy": turn_policy.sandbox_policy
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
        on_event: impl FnMut(CodexAppServerEvent) -> Result<(), CodexLlmClientError>,
    ) -> Result<(), CodexLlmClientError> {
        self.stream_events_with_idle_timeout(cancellation, self.turn_idle_timeout, on_event)
    }

    fn stream_events_with_idle_timeout(
        &mut self,
        cancellation: &CancellationToken,
        idle_timeout: Duration,
        mut on_event: impl FnMut(CodexAppServerEvent) -> Result<(), CodexLlmClientError>,
    ) -> Result<(), CodexLlmClientError> {
        let mut last_progress = Instant::now();
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
                            last_progress = Instant::now();
                            if done {
                                return Ok(());
                            }
                        }
                    } else {
                        self.respond_to_server_request(&message)?;
                    }
                }
                None => {
                    self.ensure_child_running()?;
                    if last_progress.elapsed() >= idle_timeout {
                        return Err(CodexLlmClientError::CommandTimedOut(format!(
                            "codex app-server turn produced no progress for {}ms",
                            idle_timeout.as_millis()
                        )));
                    }
                }
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
                "cwd": input.execution_context.effective_cwd.as_str(),
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

    pub(super) fn request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, CodexLlmClientError> {
        let id = self.next_request_id();
        self.send(json!({"id": id, "method": method, "params": params}))?;
        self.read_until_response(id)
    }

    fn read_until_response(&mut self, expected_id: i64) -> Result<Value, CodexLlmClientError> {
        let deadline = Instant::now() + APP_SERVER_REQUEST_TIMEOUT;
        loop {
            if Instant::now() >= deadline {
                return Err(CodexLlmClientError::CommandTimedOut(format!(
                    "codex app-server request {expected_id} exceeded {}ms",
                    APP_SERVER_REQUEST_TIMEOUT.as_millis()
                )));
            }
            let Some(message) = self.recv_message_tick()? else {
                self.ensure_child_running()?;
                continue;
            };
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
#[path = "client_tests.rs"]
mod tests;
