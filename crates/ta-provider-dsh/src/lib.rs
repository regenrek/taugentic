//! Direct NDJSON bridge contract for the sealed DeepSeek Harness runtime.
//!
//! This crate deliberately has no DSH SDK dependency and no credential, tool,
//! policy, transcript, or persistence ownership. Packaging supplies the sealed
//! executable in a later slice; tests inject a deterministic child process.
use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

/// Every interaction which needs the bridge to make progress uses this one
/// deadline.  It is deliberately a provider concern: callers neither own a
/// child handle nor get a second lifecycle/reap path.
const LIFECYCLE_DEADLINE: std::time::Duration = std::time::Duration::from_millis(500);

pub const DSH_RUNTIME_VERSION: &str = "dsh-v0.1.1-rc.2";
pub const DSH_BRIDGE_PROTOCOL: &str = "taugentic-dsh-bridge/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekModel {
    V4Flash,
    V4Pro,
}

impl DeepSeekModel {
    pub fn parse(value: &str) -> Result<Self, DshError> {
        match value {
            "deepseek-v4-flash" => Ok(Self::V4Flash),
            "deepseek-v4-pro" => Ok(Self::V4Pro),
            _ => Err(DshError::UnsupportedModel(value.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::V4Flash => "deepseek-v4-flash",
            Self::V4Pro => "deepseek-v4-pro",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SealedRuntime {
    executable: PathBuf,
}

impl SealedRuntime {
    /// Packaging may construct this only from its verified embedded asset.
    /// A bare program name is rejected so PATH/system-Node lookup is impossible.
    pub fn from_sealed_executable(executable: PathBuf) -> Result<Self, DshError> {
        if !executable.is_absolute() || executable.file_name().is_none() {
            return Err(DshError::InvalidRuntime(
                "DSH runtime must be an absolute sealed asset path".to_string(),
            ));
        }
        Ok(Self { executable })
    }
}

#[derive(Debug, Error)]
pub enum DshError {
    #[error("unsupported direct DSH model: {0}")]
    UnsupportedModel(String),
    #[error("invalid sealed DSH runtime: {0}")]
    InvalidRuntime(String),
    #[error("DSH bridge process failed: {0}")]
    Process(String),
    #[error("DSH bridge execution cancelled")]
    Cancelled,
    #[error("DSH bridge protocol rejected: {0}")]
    Protocol(String),
}

#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
enum Request<'a> {
    Initialize {
        protocol: &'a str,
        runtime: &'a str,
    },
    Prompt {
        model: &'a str,
        objective: &'a str,
        seed: Option<&'a str>,
    },
    Cancel {
        run_id: &'a str,
    },
    Approval {
        approval_id: &'a str,
        approved: bool,
    },
    Shutdown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BridgeEvent {
    Initialized {
        protocol: String,
        runtime: String,
    },
    Stream {
        turn_id: String,
        item_id: String,
        delta: String,
    },
    Approval {
        approval_id: String,
        call_id: String,
        tool_name: String,
    },
    Snapshot {
        continuation: String,
    },
    Completed,
    Cancelled,
    Error {
        message: String,
    },
    Shutdown,
}

/// Live controls are intentionally transport-only. The caller owns approval
/// policy and uses the bridge id only to correlate its resulting decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeControl {
    Cancel { run_id: String },
    Approval { approval_id: String, approved: bool },
}

struct DshClient {
    child: Child,
    stdin: ChildStdin,
    stdout: tokio::io::Lines<BufReader<ChildStdout>>,
    pending_approvals: BTreeSet<String>,
}

impl DshClient {
    pub async fn start(runtime: SealedRuntime) -> Result<Self, DshError> {
        let mut child = Command::new(runtime.executable)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| DshError::Process(error.to_string()))?;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                Self::force_reap_child(&mut child).await;
                return Err(DshError::Process("bridge stdin unavailable".to_string()));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                Self::force_reap_child(&mut child).await;
                return Err(DshError::Process("bridge stdout unavailable".to_string()));
            }
        };
        let mut client = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            pending_approvals: BTreeSet::new(),
        };
        if let Err(error) = client
            .send(Request::Initialize {
                protocol: DSH_BRIDGE_PROTOCOL,
                runtime: DSH_RUNTIME_VERSION,
            })
            .await
        {
            // `send` entered the provider's bounded reap path before this
            // initialization failure can leave `start`.
            return Err(error);
        }
        let initialize_result = tokio::time::timeout(LIFECYCLE_DEADLINE, client.next()).await;
        let initialized = client
            .enforce_deadline("initialize acknowledgement", initialize_result)
            .await;
        match initialized {
            Err(error) => {
                client.force_reap().await;
                Err(error)
            }
            Ok(BridgeEvent::Initialized { protocol, runtime })
                if protocol == DSH_BRIDGE_PROTOCOL && runtime == DSH_RUNTIME_VERSION =>
            {
                Ok(client)
            }
            Ok(BridgeEvent::Initialized { protocol, runtime }) => {
                client.force_reap().await;
                Err(DshError::Protocol(format!(
                    "version mismatch: protocol={protocol}, runtime={runtime}"
                )))
            }
            Ok(event) => {
                client.force_reap().await;
                Err(DshError::Protocol(format!(
                    "expected initialize response, got {event:?}"
                )))
            }
        }
    }

    pub async fn prompt(
        &mut self,
        model: DeepSeekModel,
        objective: &str,
        seed: Option<&str>,
    ) -> Result<(), DshError> {
        self.send(Request::Prompt {
            model: model.as_str(),
            objective,
            seed,
        })
        .await
    }
    pub async fn cancel(&mut self, run_id: &str) -> Result<(), DshError> {
        self.send(Request::Cancel { run_id }).await
    }
    pub async fn resolve_approval(
        &mut self,
        approval_id: &str,
        approved: bool,
    ) -> Result<(), DshError> {
        self.send(Request::Approval {
            approval_id,
            approved,
        })
        .await
    }
    pub async fn next(&mut self) -> Result<BridgeEvent, DshError> {
        let line = self
            .stdout
            .next_line()
            .await
            .map_err(|error| DshError::Process(error.to_string()))?
            .ok_or_else(|| DshError::Process("bridge process exited".to_string()))?;
        serde_json::from_str(&line).map_err(|error| DshError::Protocol(error.to_string()))
    }
    pub async fn shutdown(mut self) -> Result<(), DshError> {
        let result = self.send(Request::Shutdown).await;
        let result = match result {
            Ok(()) => {
                let acknowledgement_result =
                    tokio::time::timeout(LIFECYCLE_DEADLINE, self.next()).await;
                match self
                    .enforce_deadline("shutdown acknowledgement", acknowledgement_result)
                    .await
                {
                    Ok(BridgeEvent::Shutdown) => {
                        let exit_result = tokio::time::timeout(LIFECYCLE_DEADLINE, async {
                            self.child
                                .wait()
                                .await
                                .map_err(|error| DshError::Process(error.to_string()))
                        })
                        .await;
                        self.enforce_deadline("shutdown process exit", exit_result)
                            .await
                            .map(|_| ())
                    }
                    Ok(_) => Err(DshError::Protocol(
                        "bridge did not acknowledge shutdown".to_string(),
                    )),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        if result.is_err() {
            self.force_reap().await;
        }
        result
    }

    async fn run_turn<F>(
        &mut self,
        model: DeepSeekModel,
        objective: &str,
        seed: Option<&str>,
        controls: &mut mpsc::UnboundedReceiver<BridgeControl>,
        mut on_event: F,
    ) -> Result<(), DshError>
    where
        F: FnMut(BridgeEvent) -> Pin<Box<dyn Future<Output = Result<(), DshError>> + Send>>,
    {
        self.prompt(model, objective, seed).await?;
        loop {
            tokio::select! {
                control = controls.recv() => match control {
                    Some(BridgeControl::Cancel { run_id }) => {
                        self.cancel(&run_id).await?;
                        self.force_reap().await;
                        return Err(DshError::Cancelled);
                    }
                    Some(BridgeControl::Approval { approval_id, approved }) => {
                        if !self.pending_approvals.remove(&approval_id) {
                            return Err(DshError::Protocol(format!(
                                "approval control is unknown, duplicate, or late: {approval_id}"
                            )));
                        }
                        self.resolve_approval(&approval_id, approved).await?;
                    }
                    None => return Err(DshError::Process("bridge controls closed".to_string())),
                },
                event = self.next() => {
                    let event = event?;
                    if let BridgeEvent::Approval { approval_id, call_id, tool_name } = &event {
                        if approval_id.is_empty() || call_id.is_empty() || tool_name.is_empty() {
                            return Err(DshError::Protocol("DSH approval event has an empty correlation field".to_string()));
                        }
                        if !self.pending_approvals.insert(approval_id.clone()) {
                            return Err(DshError::Protocol(format!(
                                "duplicate DSH approval event: {approval_id}"
                            )));
                        }
                    }
                    let terminal = matches!(event, BridgeEvent::Completed | BridgeEvent::Cancelled | BridgeEvent::Error { .. });
                    if terminal && !self.pending_approvals.is_empty() {
                        return Err(DshError::Protocol(
                            "DSH bridge terminated with unresolved approval controls".to_string(),
                        ));
                    }
                    on_event(event).await?;
                    if terminal { return Ok(()); }
                }
            }
        }
    }
    async fn send(&mut self, request: Request<'_>) -> Result<(), DshError> {
        let payload = serde_json::to_string(&request)
            .map_err(|error| DshError::Protocol(error.to_string()))?;
        let write_result = tokio::time::timeout(LIFECYCLE_DEADLINE, async {
            self.stdin
                .write_all(payload.as_bytes())
                .await
                .map_err(|error| DshError::Process(error.to_string()))?;
            self.stdin
                .write_all(b"\n")
                .await
                .map_err(|error| DshError::Process(error.to_string()))?;
            self.stdin
                .flush()
                .await
                .map_err(|error| DshError::Process(error.to_string()))
        })
        .await;
        self.enforce_deadline("stdin write", write_result).await
    }

    /// Convert every elapsed bridge deadline into the same fail-closed reap.
    /// The operation itself is evaluated before this method is called, so no
    /// mutable borrow of the child survives when reaping begins.
    async fn enforce_deadline<T>(
        &mut self,
        phase: &str,
        result: Result<Result<T, DshError>, tokio::time::error::Elapsed>,
    ) -> Result<T, DshError> {
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => {
                self.force_reap().await;
                Err(error)
            }
            Err(_) => {
                self.force_reap().await;
                Err(DshError::Process(format!(
                    "DSH bridge {phase} exceeded lifecycle deadline"
                )))
            }
        }
    }

    /// The external bridge is never allowed to outlive a failed protocol
    /// interaction. This is intentionally a single fail-closed teardown path
    /// for handshake, malformed input, child death, cancellation and failed
    /// shutdown handling in callers.
    pub async fn force_reap(&mut self) {
        Self::force_reap_child(&mut self.child).await;
    }

    async fn force_reap_child(child: &mut Child) {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(LIFECYCLE_DEADLINE, child.wait()).await;
    }
}

impl Drop for DshClient {
    fn drop(&mut self) {
        // This is the last-resort ownership backstop when a supervisor is
        // dropped before it can enter an async terminal path. All normal
        // terminal paths go through `force_reap`, which waits boundedly.
        let _ = self.child.start_kill();
    }
}

/// One initialized, reusable bridge process. This is the only owner of the
/// external DSH protocol loop: callers provide one control stream and one
/// event mapper for each turn, while this supervisor preserves the warm child
/// across successful turns and tears it down on every failed interaction.
pub struct DshSupervisor {
    client: Option<DshClient>,
}

impl DshSupervisor {
    pub async fn start(runtime: SealedRuntime) -> Result<Self, DshError> {
        Ok(Self {
            client: Some(DshClient::start(runtime).await?),
        })
    }

    pub async fn run_turn<F>(
        &mut self,
        model: DeepSeekModel,
        objective: &str,
        seed: Option<&str>,
        controls: &mut mpsc::UnboundedReceiver<BridgeControl>,
        on_event: F,
    ) -> Result<(), DshError>
    where
        F: FnMut(BridgeEvent) -> Pin<Box<dyn Future<Output = Result<(), DshError>> + Send>>,
    {
        let Some(client) = self.client.as_mut() else {
            return Err(DshError::Process(
                "DSH supervisor is no longer available".to_string(),
            ));
        };
        let result = client
            .run_turn(model, objective, seed, controls, on_event)
            .await;
        if result.is_err() {
            client.force_reap().await;
            self.client.take();
        }
        result
    }

    pub async fn shutdown(&mut self) -> Result<(), DshError> {
        let Some(client) = self.client.take() else {
            return Ok(());
        };
        client.shutdown().await
    }

    pub async fn force_reap(&mut self) {
        if let Some(client) = self.client.as_mut() {
            client.force_reap().await;
        }
        self.client.take();
    }
}

impl Drop for DshSupervisor {
    fn drop(&mut self) {
        // `DshClient::drop` issues the same terminal kill when an owning
        // caller disappears outside an async context. Normal Drop through
        // the harness first delivers cancellation and reaches `force_reap`.
        self.client.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> SealedRuntime {
        let path = std::env::temp_dir().join(format!("dsh-fixture-{}", std::process::id()));
        std::fs::write(&path, r#"#!/bin/sh
while IFS= read -r l; do case "$l" in
*initialize*) echo '{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}' ;;
*prompt*) echo '{"event":"stream","turnId":"turn-1","itemId":"item-1","delta":"hello"}'; echo '{"event":"approval","approvalId":"approval-1","callId":"tool-1","toolName":"shell"}'; echo '{"event":"snapshot","continuation":"v1:opaque"}' ;;
*cancel*) echo '{"event":"cancelled"}' ;;
*shutdown*) echo '{"event":"shutdown"}'; exit 0 ;;
esac; done
"#).expect("fixture write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("fixture chmod");
        SealedRuntime::from_sealed_executable(path).expect("sealed fixture")
    }
    #[tokio::test]
    async fn direct_fixture_streams_approves_cancels_snapshots_and_shuts_down() {
        let mut client = DshClient::start(fixture()).await.expect("initialize");
        client
            .prompt(DeepSeekModel::V4Pro, "test", Some("v1:seed"))
            .await
            .expect("prompt");
        assert!(matches!(
            client.next().await.expect("stream"),
            BridgeEvent::Stream { .. }
        ));
        let approval = client.next().await.expect("approval");
        assert!(matches!(approval, BridgeEvent::Approval { .. }));
        assert!(matches!(
            client.next().await.expect("snapshot"),
            BridgeEvent::Snapshot { .. }
        ));
        client
            .resolve_approval("approval-1", true)
            .await
            .expect("resolve");
        client.cancel("run-1").await.expect("cancel");
        assert_eq!(
            client.next().await.expect("cancelled"),
            BridgeEvent::Cancelled
        );
        client.shutdown().await.expect("shutdown");
    }
    #[test]
    fn rejects_unsupported_models_and_relative_runtime() {
        assert!(DeepSeekModel::parse("deepseek-chat").is_err());
        assert!(SealedRuntime::from_sealed_executable(PathBuf::from("node")).is_err());
    }

    #[tokio::test]
    async fn initialization_version_mismatch_reaps_the_child() {
        let runtime = fixture_script(
            r#"#!/bin/sh
while IFS= read -r l; do case "$l" in
*initialize*) echo '{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"wrong"}' ;;
esac; done
"#,
        );
        let error = match DshSupervisor::start(runtime).await {
            Ok(_) => panic!("mismatched runtime must fail closed"),
            Err(error) => error,
        };
        assert!(matches!(error, DshError::Protocol(_)));
    }

    #[tokio::test]
    async fn silent_initialize_hits_the_bounded_reap_path() {
        let marker = marker_path("silent-initialize");
        let runtime = fixture_script(&format!(
            r#"#!/bin/sh
echo $$ > "{}"
while :; do :; done
"#,
            marker.display()
        ));
        let error = match DshSupervisor::start(runtime).await {
            Ok(_) => panic!("silent initialize must hit the lifecycle deadline"),
            Err(error) => error,
        };
        assert!(matches!(error, DshError::Process(_)));
        assert_process_reaped(&marker);
    }

    #[tokio::test]
    async fn non_reading_initialize_stdin_fails_closed_and_reaps() {
        let marker = marker_path("closed-initialize-stdin");
        let runtime = fixture_script(&format!(
            r#"#!/bin/sh
exec 0<&-
echo $$ > "{}"
while :; do :; done
"#,
            marker.display()
        ));
        let error = match DshSupervisor::start(runtime).await {
            Ok(_) => panic!("closed initialize stdin must fail"),
            Err(error) => error,
        };
        assert!(matches!(error, DshError::Process(_)));
        assert_process_reaped(&marker);
    }

    #[tokio::test]
    async fn non_reading_prompt_stdin_hits_the_bounded_reap_path() {
        let marker = marker_path("blocked-prompt-stdin");
        let runtime = fixture_script(&format!(
            r#"#!/bin/sh
echo $$ > "{}"
while IFS= read -r l; do case "$l" in
*initialize*) echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}'; while :; do :; done ;;
esac; done
"#,
            marker.display()
        ));
        let mut client = DshClient::start(runtime).await.expect("initialize");
        let error = client
            .prompt(DeepSeekModel::V4Flash, &large_text(), None)
            .await
            .expect_err("non-reading child must block prompt stdin");
        assert!(matches!(error, DshError::Process(_)));
        assert_process_reaped(&marker);
    }

    #[tokio::test]
    async fn non_reading_approval_stdin_hits_the_bounded_reap_path() {
        let marker = marker_path("blocked-approval-stdin");
        let runtime = fixture_script(&format!(
            r#"#!/bin/sh
echo $$ > "{}"
while IFS= read -r l; do case "$l" in
*initialize*) echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}' ;;
*prompt*) echo '{{"event":"approval","approvalId":"bridge-approval","callId":"tool-1","toolName":"shell"}}'; while :; do :; done ;;
esac; done
"#,
            marker.display()
        ));
        let mut client = DshClient::start(runtime).await.expect("initialize");
        client
            .prompt(DeepSeekModel::V4Flash, "approval", None)
            .await
            .expect("prompt");
        assert!(matches!(
            client.next().await.expect("approval"),
            BridgeEvent::Approval { .. }
        ));
        let error = client
            .resolve_approval(&large_text(), true)
            .await
            .expect_err("non-reading child must block approval stdin");
        assert!(matches!(error, DshError::Process(_)));
        assert_process_reaped(&marker);
    }

    #[tokio::test]
    async fn silent_shutdown_hits_the_bounded_reap_path() {
        let marker = marker_path("silent-shutdown");
        let runtime = fixture_script(&format!(
            r#"#!/bin/sh
echo $$ > "{}"
while IFS= read -r l; do case "$l" in
*initialize*) echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}' ;;
*shutdown*) while :; do :; done ;;
esac; done
"#,
            marker.display()
        ));
        let mut supervisor = DshSupervisor::start(runtime).await.expect("initialize");
        let error = supervisor
            .shutdown()
            .await
            .expect_err("silent shutdown must hit the lifecycle deadline");
        assert!(matches!(error, DshError::Process(_)));
        assert_process_reaped(&marker);
    }

    #[tokio::test]
    async fn malformed_event_reaps_supervisor_and_prevents_reuse() {
        let runtime = fixture_script(
            r#"#!/bin/sh
while IFS= read -r l; do case "$l" in
*initialize*) echo '{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}' ;;
*prompt*) echo '{not-json' ;;
esac; done
"#,
        );
        let mut supervisor = DshSupervisor::start(runtime).await.expect("initialize");
        let (_sender, mut controls) = mpsc::unbounded_channel();
        let error = supervisor
            .run_turn(
                DeepSeekModel::V4Flash,
                "test",
                None,
                &mut controls,
                |_event| Box::pin(async { Ok(()) }),
            )
            .await
            .expect_err("malformed stream must fail");
        assert!(matches!(error, DshError::Protocol(_)));
        assert!(supervisor.client.is_none(), "failed child must be reaped");
    }

    #[tokio::test]
    async fn cancellation_reaps_the_child_and_emits_no_terminal_callback() {
        let mut supervisor = DshSupervisor::start(fixture()).await.expect("initialize");
        let (sender, mut controls) = mpsc::unbounded_channel();
        sender
            .send(BridgeControl::Cancel {
                run_id: "run-cancel".to_string(),
            })
            .expect("cancel control");
        let error = supervisor
            .run_turn(
                DeepSeekModel::V4Flash,
                "test",
                None,
                &mut controls,
                |_event| Box::pin(async { Ok(()) }),
            )
            .await
            .expect_err("cancellation must terminate turn");
        assert!(matches!(error, DshError::Cancelled));
        assert!(
            supervisor.client.is_none(),
            "cancelled child must be reaped"
        );
    }

    fn fixture_script(source: &str) -> SealedRuntime {
        let path = std::env::temp_dir().join(format!(
            "dsh-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(&path, source).expect("fixture write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("fixture chmod");
        SealedRuntime::from_sealed_executable(path).expect("sealed fixture")
    }

    fn marker_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dsh-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn assert_process_reaped(marker: &std::path::Path) {
        let pid = std::fs::read_to_string(marker)
            .expect("fixture pid marker")
            .trim()
            .to_string();
        let status = std::process::Command::new("kill")
            .args(["-0", pid.as_str()])
            .status()
            .expect("kill probe");
        assert!(!status.success(), "fixture child {pid} must be reaped");
    }

    fn large_text() -> String {
        "x".repeat(256 * 1024)
    }

    #[tokio::test]
    async fn supervisor_reuses_one_initialized_child_for_two_completed_turns() {
        let marker = std::env::temp_dir().join(format!("dsh-init-marker-{}", std::process::id()));
        let path = std::env::temp_dir().join(format!("dsh-reuse-fixture-{}", std::process::id()));
        std::fs::write(&path, format!(r#"#!/bin/sh
while IFS= read -r l; do case "$l" in
*initialize*) echo init >> "{}"; echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}' ;;
*prompt*) echo '{{"event":"stream","turnId":"turn-1","itemId":"item-1","delta":"hello"}}'; echo '{{"event":"completed"}}' ;;
*shutdown*) echo '{{"event":"shutdown"}}'; exit 0 ;;
esac; done
"#, marker.display())).expect("fixture write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("fixture chmod");
        let runtime = SealedRuntime::from_sealed_executable(path).expect("sealed runtime");
        let mut supervisor = DshSupervisor::start(runtime).await.expect("initialize");
        let (sender, mut controls) = mpsc::unbounded_channel();
        for objective in ["first", "second"] {
            supervisor
                .run_turn(
                    DeepSeekModel::V4Pro,
                    objective,
                    None,
                    &mut controls,
                    |event| {
                        Box::pin(async move {
                            assert!(matches!(
                                event,
                                BridgeEvent::Stream { .. } | BridgeEvent::Completed
                            ));
                            Ok(())
                        })
                    },
                )
                .await
                .expect("turn");
        }
        drop(sender);
        supervisor.shutdown().await.expect("shutdown");
        assert_eq!(
            std::fs::read_to_string(marker)
                .expect("marker")
                .lines()
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn supervisor_rejects_unknown_approval_control_and_reaps_child() {
        let mut supervisor = DshSupervisor::start(fixture()).await.expect("initialize");
        let (sender, mut controls) = mpsc::unbounded_channel();
        sender
            .send(BridgeControl::Approval {
                approval_id: "unknown".to_string(),
                approved: true,
            })
            .expect("control");
        let error = supervisor
            .run_turn(
                DeepSeekModel::V4Flash,
                "test",
                None,
                &mut controls,
                |_event| Box::pin(async move { Ok(()) }),
            )
            .await
            .expect_err("unknown approval must fail closed");
        assert!(matches!(error, DshError::Protocol(_)));
        assert!(supervisor.client.is_none(), "failed child must be reaped");
    }
}
