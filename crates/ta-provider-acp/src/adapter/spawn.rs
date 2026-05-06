use std::time::Duration;

#[cfg(unix)]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use ta_exec::{
    ExecEngine, LocalExecEngine, NetworkPolicy, SandboxProfile, SpawnRequest, StdioPolicy,
};
use tokio::process::Child;

use super::{AcpProcessConfig, TraceContext};
use crate::error::AcpClientError;

#[tracing::instrument(
    skip(config, trace),
    fields(
        flavor_id = %trace.flavor_id,
        run_id = %trace.run_id,
        session_id = %trace.session_id,
        command = %config.command.display()
    )
)]
pub(super) fn spawn_acp_process(
    config: &AcpProcessConfig,
    trace: &TraceContext,
) -> Result<Child, AcpClientError> {
    ensure_acp_perimeter_supported(&config.sandbox_profile)?;
    let mut request = SpawnRequest::new(config.command.as_os_str())
        .args(config.args.clone())
        .cwd(config.work_dir.clone())
        .stdin(StdioPolicy::Piped)
        .stdout(StdioPolicy::Piped)
        .stderr(StdioPolicy::Inherit)
        .env_remove(config.env_remove.iter().map(String::as_str))
        .sandbox_profile(config.sandbox_profile.clone());
    for (name, value) in &config.env {
        request = request.env(name.as_str(), value.as_str());
    }

    LocalExecEngine.spawn(request).map_err(|error| {
        AcpClientError::ProcessFailed(format!(
            "failed to spawn ACP process {}: {error}",
            config.command.display()
        ))
    })
}

fn ensure_acp_perimeter_supported(profile: &SandboxProfile) -> Result<(), AcpClientError> {
    match profile.network_policy() {
        NetworkPolicy::Off | NetworkPolicy::Open => Ok(()),
        NetworkPolicy::Loopback | NetworkPolicy::Allowlist(_) => {
            Err(AcpClientError::InvalidConfig(
                "ACP perimeter sandbox supports only closed or open network policy in this slice"
                    .to_string(),
            ))
        }
    }
}

pub(super) async fn terminate_child(
    child: &mut Child,
    grace: Duration,
) -> Result<(), AcpClientError> {
    let _ = request_child_shutdown(child);
    match tokio::time::timeout(grace, child.wait()).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(AcpClientError::ProcessFailed(format!(
            "failed waiting for ACP process: {error}"
        ))),
        Err(_) => {
            let _ = child.start_kill();
            child.wait().await.map(|_| ()).map_err(|error| {
                AcpClientError::ProcessFailed(format!("failed killing ACP process: {error}"))
            })
        }
    }
}

#[cfg(unix)]
fn request_child_shutdown(child: &mut Child) -> Result<(), AcpClientError> {
    if let Some(pid) = child.id() {
        send_signal(pid, Signal::SIGTERM)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn request_child_shutdown(child: &mut Child) -> Result<(), AcpClientError> {
    child.start_kill().map_err(|error| {
        AcpClientError::ProcessFailed(format!("failed requesting ACP process shutdown: {error}"))
    })
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: Signal) -> Result<(), AcpClientError> {
    kill(Pid::from_raw(pid as i32), signal).map_err(|error| {
        AcpClientError::ProcessFailed(format!("failed to signal ACP process {pid}: {error}"))
    })
}
