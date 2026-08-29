use std::{env, ffi::OsString, sync::mpsc, thread, time::Duration};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child as TokioChild, ChildStdout};
use tokio::runtime::{Handle, Runtime};

use super::CodexLlmClientError;
use super::framing::parse_jsonl_frame;

#[cfg(target_os = "macos")]
const MACOS_CA_BUNDLE: &str = "/private/etc/ssl/cert.pem";
#[cfg(target_os = "macos")]
const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";
const CODEX_HOME_ENV: &str = "CODEX_HOME";

pub(crate) fn run_on_control_thread<T, F>(task: F) -> Result<T, CodexLlmClientError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CodexLlmClientError> + Send + 'static,
{
    thread::Builder::new()
        .name("taugentic-codex-control".to_string())
        .spawn(task)
        .map_err(|error| {
            CodexLlmClientError::CommandFailed(format!(
                "failed to spawn Codex control worker: {error}"
            ))
        })?
        .join()
        .map_err(|_| {
            CodexLlmClientError::CommandFailed("Codex control worker panicked".to_string())
        })?
}

pub(crate) fn app_server_env(
    auth_profile_id: &str,
) -> Result<Vec<(&'static str, OsString)>, CodexLlmClientError> {
    let profile_id = ta_protocol::wire::AuthProfileId::new(auth_profile_id)
        .map_err(|error| CodexLlmClientError::InvalidConfig(error.to_string()))?;
    let managed_home = managed_codex_home(&profile_id)?;
    std::fs::create_dir_all(&managed_home).map_err(|error| {
        CodexLlmClientError::InvalidConfig(format!(
            "failed to prepare managed Codex profile home: {error}"
        ))
    })?;
    let mut environment = vec![(CODEX_HOME_ENV, managed_home.into_os_string())];
    #[cfg(target_os = "macos")]
    {
        let ca_bundle = std::path::Path::new(MACOS_CA_BUNDLE);
        if !ca_bundle.is_file() {
            return Err(CodexLlmClientError::InvalidConfig(format!(
                "Codex app-server requires the macOS CA bundle at {MACOS_CA_BUNDLE}"
            )));
        }
        environment.push((SSL_CERT_FILE_ENV, ca_bundle.as_os_str().to_os_string()));
    }
    Ok(environment)
}

pub(crate) fn managed_codex_home(
    auth_profile_id: &ta_protocol::wire::AuthProfileId,
) -> Result<std::path::PathBuf, CodexLlmClientError> {
    let home = env::var_os("HOME").ok_or_else(|| {
        CodexLlmClientError::InvalidConfig(
            "managed Codex profile storage requires a user home".to_string(),
        )
    })?;
    Ok(std::path::PathBuf::from(home)
        .join(".taugentic")
        .join("codex-profiles")
        .join(auth_profile_id.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn app_server_uses_explicit_macos_ca_bundle() {
        let environment = app_server_env("profile-test").expect("app-server environment");

        assert!(environment.iter().any(|(name, value)| {
            *name == SSL_CERT_FILE_ENV && value == std::ffi::OsStr::new(MACOS_CA_BUNDLE)
        }));
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
        None => runtime
            .block_on(ta_exec::terminate_child_tree(child, Duration::from_secs(2)))
            .map(|_| ())
            .map_err(|error| {
                CodexLlmClientError::CommandFailed(format!(
                    "failed to terminate codex app-server process tree: {error}"
                ))
            }),
    }
}
