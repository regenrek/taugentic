use std::{env, ffi::OsString, sync::mpsc, thread};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child as TokioChild, ChildStdout};
use tokio::runtime::{Handle, Runtime};

use super::framing::parse_jsonl_frame;
use super::{CODEX_API_KEY_AUTH_PROFILE_ID, CodexLlmClientError, OPENAI_API_KEY_ENV_VAR};

pub(crate) fn app_server_env(
    auth_profile_id: Option<&str>,
) -> Result<Vec<(&'static str, OsString)>, CodexLlmClientError> {
    match auth_profile_id {
        Some(CODEX_API_KEY_AUTH_PROFILE_ID) => Ok(vec![(
            OPENAI_API_KEY_ENV_VAR,
            env::var_os(OPENAI_API_KEY_ENV_VAR).ok_or(CodexLlmClientError::MissingApiKeyEnv)?,
        )]),
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn spawn_jsonl_reader(
    stdout: ChildStdout,
    runtime: Handle,
    messages: mpsc::Sender<Result<Value, CodexLlmClientError>>,
) -> Result<thread::JoinHandle<()>, CodexLlmClientError> {
    thread::Builder::new()
        .name("taugentic-codex-app-server-reader".to_string())
        .spawn(move || {
            runtime.block_on(async move {
                let mut reader = BufReader::new(stdout);
                loop {
                    let mut frame = Vec::new();
                    match reader.read_until(b'\n', &mut frame).await {
                        Ok(0) => break,
                        Ok(_) if frame.iter().all(u8::is_ascii_whitespace) => continue,
                        Ok(_) => {
                            if messages.send(parse_jsonl_frame(&frame)).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = messages.send(Err(CodexLlmClientError::CommandFailed(
                                format!("failed to read codex app-server stdout: {error}"),
                            )));
                            break;
                        }
                    }
                }
            })
        })
        .map_err(|error| {
            CodexLlmClientError::CommandFailed(format!(
                "failed to spawn codex app-server reader: {error}"
            ))
        })
}

pub(crate) fn terminate_app_server(
    child: &mut TokioChild,
    runtime: &Runtime,
) -> Result<(), CodexLlmClientError> {
    match child.try_wait().map_err(|error| {
        CodexLlmClientError::CommandFailed(format!("failed to poll codex app-server: {error}"))
    })? {
        Some(_) => Ok(()),
        None => {
            child.start_kill().map_err(|error| {
                CodexLlmClientError::CommandFailed(format!(
                    "failed to kill codex app-server: {error}"
                ))
            })?;
            runtime.block_on(child.wait()).map_err(|error| {
                CodexLlmClientError::CommandFailed(format!(
                    "failed to wait for codex app-server exit: {error}"
                ))
            })?;
            Ok(())
        }
    }
}
