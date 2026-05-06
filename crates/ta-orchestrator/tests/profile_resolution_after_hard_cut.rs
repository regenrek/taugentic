use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ta_daemon_client::PersistentDaemonClient;
use ta_jsonrpc::{ClientConfig, JsonRpcClient, JsonRpcClientError, SocketAddress};
use ta_observability::{LOG_DIR_ENV_VAR, LOG_STDERR_ENV_VAR};
use ta_protocol::wire::{
    AgentRuntimeSnapshot, AgentRuntimeStrategyHealthStatus, AuthProfileConnectionState,
    AuthProfileId, DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSelectProfileParams,
    DaemonEvent, DaemonRuntimeMode, DaemonStatusParams, DaemonStatusResult, METHOD_DAEMON_STATUS,
    RunEvent, RunId, RunSource, RunStatus, RuntimeProfileAuthProfilePatch, RuntimeProfileId,
    RuntimeProfilePatch, SessionId, SessionStatus,
};
use ta_store::{
    CommitRepository, CommitRunTransition, CommitSessionOpen, ProjectionRepository, RunProjection,
    SessionProjection, SqliteStore,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const DAEMON_RUNTIME_MODE_ENV_VAR: &str = "TAUGENTIC_DAEMON_RUNTIME_MODE";
const DAEMON_SOCKET_NAME_ENV_VAR: &str = "TAUGENTIC_DAEMON_SOCKET_NAME";
static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[test]
#[cfg(unix)]
fn openai_runtime_profile_and_auth_profile_resolve_after_hard_cut() {
    let socket_name = unique_name("ta-pr");
    let mut daemon = ManagedDaemon::spawn_with_seeded_store(
        &socket_name,
        vec![("OPENAI_API_KEY".to_string(), "test-openai-key".to_string())],
    );

    let status = daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before profile resolution assertions");
    assert_eq!(status.runtime_mode, DaemonRuntimeMode::Local);

    let mut client = initialized_client(daemon.client_config(), "ta-profile-resolution-hard-cut");
    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should load");

    assert_openai_profile_resolution(&snapshot);
    assert_seeded_runtime_profile_survived_boot(&daemon.store_path());

    let selected = client
        .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
        })
        .expect("openai runtime profile should select through StrategyRegistry");
    assert_eq!(
        selected.selection.runtime_profile_id.as_str(),
        "runtime-openai-safe"
    );

    let patched = client
        .patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            patch: RuntimeProfilePatch {
                auth_profile: Some(RuntimeProfileAuthProfilePatch::Set {
                    value: AuthProfileId::new("auth-openai-api-key").expect("auth profile id"),
                }),
                ..Default::default()
            },
        })
        .expect("runtime profile auth profile should resolve through StrategyRegistry");
    assert_openai_profile_resolution(&patched);
}

fn assert_openai_profile_resolution(snapshot: &AgentRuntimeSnapshot) {
    let provider = snapshot
        .providers
        .iter()
        .find(|provider| provider.id.as_str() == "openai")
        .expect("openai strategy should exist");
    assert_eq!(
        provider.health.status,
        AgentRuntimeStrategyHealthStatus::Ready
    );

    let auth_profile = snapshot
        .auth_profiles
        .iter()
        .find(|profile| profile.profile.id.as_str() == "auth-openai-api-key")
        .expect("openai api-key auth profile should exist");
    assert_eq!(auth_profile.profile.provider_id.as_str(), "openai");
    assert_eq!(
        auth_profile.connection_state,
        AuthProfileConnectionState::Connected
    );

    let runtime_profile = snapshot
        .runtime_profiles
        .iter()
        .find(|profile| profile.id.as_str() == "runtime-openai-safe")
        .expect("openai safe runtime profile should exist");
    assert_eq!(runtime_profile.provider_id.as_str(), "openai");
    assert_eq!(
        runtime_profile
            .auth_profile_id
            .as_ref()
            .expect("runtime profile should reference auth profile")
            .as_str(),
        "auth-openai-api-key"
    );
}

fn initialized_client(config: ClientConfig, client_name: &str) -> PersistentDaemonClient {
    let mut client =
        PersistentDaemonClient::connect(config, client_name.to_string()).expect("connect");
    client
        .initialize(client_name, "0.0.1", None)
        .expect("initialize");
    client
}

struct ManagedDaemon {
    child: Option<Child>,
    socket_name: String,
    runtime_dir: PathBuf,
    store_path: PathBuf,
}

impl ManagedDaemon {
    fn spawn_with_seeded_store(socket_name: &str, extra_env: Vec<(String, String)>) -> Self {
        let root_dir = test_temp_dir(socket_name);
        let home_dir = root_dir.join("home");
        let runtime_dir = PathBuf::from("/tmp/tg-runtime");
        let log_dir = root_dir.join("logs");
        fs::create_dir_all(&home_dir).expect("home dir");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::create_dir_all(&log_dir).expect("log dir");
        let store_path = daemon_store_path_for_home(&home_dir, socket_name);
        seed_openai_runtime_profile_store(&store_path);

        let mut command = Command::new(env!("CARGO_BIN_EXE_ta-daemon"));
        command
            .env(DAEMON_SOCKET_NAME_ENV_VAR, socket_name)
            .env(DAEMON_RUNTIME_MODE_ENV_VAR, "local")
            .env("HOME", &home_dir)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env(LOG_DIR_ENV_VAR, &log_dir)
            .env(LOG_STDERR_ENV_VAR, "0")
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (key, value) in extra_env {
            command.env(key, value);
        }

        let child = command.spawn().expect("ta-daemon should spawn");
        Self {
            child: Some(child),
            socket_name: socket_name.to_string(),
            runtime_dir,
            store_path,
        }
    }

    fn client_config(&self) -> ClientConfig {
        ClientConfig {
            service_name: "ta-profile-resolution-hard-cut".to_string(),
            socket_address: daemon_socket_address(&self.runtime_dir, &self.socket_name),
            io_timeout: Duration::from_secs(30),
        }
    }

    fn store_path(&self) -> PathBuf {
        self.store_path.clone()
    }

    fn wait_for_status(&mut self) -> Result<DaemonStatusResult, String> {
        let client = JsonRpcClient::new(self.client_config());
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            self.fail_if_exited()?;
            match client.call::<_, DaemonStatusResult>(METHOD_DAEMON_STATUS, &DaemonStatusParams {})
            {
                Ok(status) => return Ok(status),
                Err(JsonRpcClientError::Socket(_)) | Err(JsonRpcClientError::Read(_)) => {
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) => {
                    return Err(format!("daemon returned unexpected startup error: {error}"));
                }
            }
        }
        Err("timed out waiting for daemon.status".to_string())
    }

    fn fail_if_exited(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Err("daemon process was already consumed".to_string());
        };
        let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to poll daemon process: {error}"))?
        else {
            return Ok(());
        };

        let output = self.wait_with_output().map_err(|error| error.to_string())?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "daemon exited early with status {status}: {stderr}"
        ))
    }

    fn wait_with_output(&mut self) -> io::Result<Output> {
        self.child
            .take()
            .expect("daemon child should exist")
            .wait_with_output()
    }

    fn shutdown(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for ManagedDaemon {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn daemon_store_path_for_home(home_dir: &Path, socket_name: &str) -> PathBuf {
    home_dir
        .join("Library")
        .join("Application Support")
        .join("taugentic")
        .join("daemon")
        .join("store")
        .join(format!("{socket_name}.sqlite3"))
}

fn seed_openai_runtime_profile_store(store_path: &Path) {
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent).expect("store parent");
    }
    let mut store = SqliteStore::open(store_path).expect("seed store should open");
    let session_id = SessionId::new("session-seeded-openai").expect("session id");
    let run_id = RunId::new("run-seeded-openai").expect("run id");
    store
        .commit_session_open(CommitSessionOpen {
            session: SessionProjection {
                id: session_id.clone(),
                owner_client_name: "ta-profile-resolution-hard-cut".to_string(),
                owner_principal_id: "principal-seeded".to_string(),
                current_session_authority_hash: "authority-hash".to_string(),
                current_session_authority_generation: 1,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "seeded profile compatibility".to_string(),
                status: SessionStatus::Idle,
            },
            occurred_at_ms: 1,
        })
        .expect("seed session");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id,
                session_id,
                runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                    .expect("runtime profile id"),
                objective: "seeded persisted runtime profile".to_string(),
                status: RunStatus::Queued,
                harness: ta_protocol::wire::RunHarnessKind::Native,
                source: RunSource::default(),
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            },
            events: vec![DaemonEvent::Run(RunEvent {
                run_id: RunId::new("run-seeded-openai").expect("run id"),
                status: RunStatus::Queued,
                detail: "seeded persisted runtime profile".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 2,
        })
        .expect("seed run");
}

fn assert_seeded_runtime_profile_survived_boot(store_path: &Path) {
    let store = SqliteStore::open(store_path).expect("seeded store should reopen");
    let seeded_run = store
        .run(&RunId::new("run-seeded-openai").expect("run id"))
        .expect("seeded run lookup")
        .expect("seeded run should survive daemon boot");
    assert_eq!(
        seeded_run.runtime_profile_id.as_str(),
        "runtime-openai-safe"
    );
}

#[cfg(unix)]
fn daemon_socket_address(runtime_dir: &Path, socket_name: &str) -> SocketAddress {
    SocketAddress::Unix(runtime_dir.join(format!("{socket_name}.sock")))
}

#[cfg(windows)]
fn daemon_socket_address(_runtime_dir: &Path, socket_name: &str) -> SocketAddress {
    SocketAddress::NamedPipe(socket_name.to_string())
}

fn test_temp_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/profile-resolution-after-hard-cut")
        .join(format!("{name}-{}", unique_suffix()))
}

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", unique_suffix())
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos}-{counter}")
}
