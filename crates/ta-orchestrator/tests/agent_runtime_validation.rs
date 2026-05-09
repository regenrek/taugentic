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
    AuthProfileId, DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimePatchProfileParams, DaemonAgentRuntimeSelectProfileParams,
    DaemonAgentRuntimeSetExtensionEnabledParams, DaemonApprovalDecideParams, DaemonRuntimeMode,
    DaemonStatusParams, DaemonStatusResult, ListApprovalsQuery, ListArtifactsQuery,
    METHOD_DAEMON_STATUS, RunStatus, RuntimePolicyMode, RuntimeProfileId, RuntimeProfilePatch,
    SessionId, SessionOverviewQuery,
};
use ta_provider_llm::families::codex_app_server::TAUGENTIC_CODEX_APP_SERVER_BIN_ENV;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const DAEMON_RUNTIME_MODE_ENV_VAR: &str = "TAUGENTIC_DAEMON_RUNTIME_MODE";
const DAEMON_SOCKET_NAME_ENV_VAR: &str = "TAUGENTIC_DAEMON_SOCKET_NAME";
static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[test]
#[cfg(unix)]
fn real_daemon_agent_runtime_fake_codex_login_updates_snapshot_and_provider_health() {
    let socket_name = unique_name("ta-agent-runtime-login");
    let fake_codex = FakeCodex::new("logged-out");
    let mut daemon = ManagedDaemon::spawn_with_env_owned(&socket_name, fake_codex_env(&fake_codex));

    let status = daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before runtime assertions");
    assert_eq!(status.runtime_mode, DaemonRuntimeMode::Local);

    let mut client = initialized_client(daemon.client_config(), "ta-agent-runtime-login");

    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should load before codex login");
    assert_eq!(
        codex_provider(&snapshot).health.status,
        AgentRuntimeStrategyHealthStatus::Degraded
    );
    assert_eq!(
        auth_profile(&snapshot, "auth-codex-chatgpt").connection_state,
        AuthProfileConnectionState::LoggedOut
    );

    let login = client
        .login_agent_runtime_auth_profile(DaemonAgentRuntimeAuthLoginParams {
            auth_profile_id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
        })
        .expect("daemon.agent.runtime.auth.login should succeed through fake codex");
    assert_eq!(
        login.auth_profile.connection_state,
        AuthProfileConnectionState::Connected
    );

    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should refresh after codex login");
    assert_eq!(
        codex_provider(&snapshot).health.status,
        AgentRuntimeStrategyHealthStatus::Ready
    );
    assert_eq!(
        auth_profile(&snapshot, "auth-codex-chatgpt").connection_state,
        AuthProfileConnectionState::Connected
    );

    let logout = client
        .logout_agent_runtime_auth_profile(DaemonAgentRuntimeAuthLogoutParams {
            auth_profile_id: AuthProfileId::new("auth-codex-chatgpt").expect("auth profile id"),
        })
        .expect("daemon.agent.runtime.auth.logout should succeed through fake codex");
    assert!(logout.disconnected);

    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should refresh after codex logout");
    assert_eq!(
        codex_provider(&snapshot).health.status,
        AgentRuntimeStrategyHealthStatus::Degraded
    );
    assert_eq!(
        auth_profile(&snapshot, "auth-codex-chatgpt").connection_state,
        AuthProfileConnectionState::LoggedOut
    );
}

#[test]
#[cfg(unix)]
fn real_daemon_agent_runtime_run_flow_tracks_runtime_profiles_for_mission_control() {
    let socket_name = unique_name("ta-agent-runtime-runs");
    let fake_codex = FakeCodex::new("chatgpt");
    let mut daemon = ManagedDaemon::spawn_with_env_owned(&socket_name, fake_codex_env(&fake_codex));

    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before run validation");
    let mut client = initialized_client(daemon.client_config(), "ta-agent-runtime-runs");

    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should load");
    assert_eq!(
        codex_provider(&snapshot).health.status,
        AgentRuntimeStrategyHealthStatus::Ready
    );

    let selected = client
        .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
        })
        .expect("runtime profile selection should succeed");
    assert_eq!(
        selected.selection.runtime_profile_id.as_str(),
        "runtime-codex-safe"
    );

    let extension_snapshot = client
        .set_agent_runtime_extension_enabled(DaemonAgentRuntimeSetExtensionEnabledParams {
            extension_id: ta_protocol::wire::RuntimeExtensionId::new("local-shell-tools")
                .expect("extension id"),
            enabled: false,
        })
        .expect("runtime extension toggle should succeed");
    assert!(
        !extension_snapshot
            .runtime_extensions
            .iter()
            .find(|extension| extension.descriptor.id.as_str() == "local-shell-tools")
            .expect("runtime extension should exist")
            .enabled
    );

    let session_a = open_and_attach_session(&mut client, "Codex Safe");
    let safe_run = client
        .start_run(ta_protocol::wire::StartRunCommand {
            objective: "Validate safe policy".to_string(),
            ..ta_protocol::wire::StartRunCommand::default()
        })
        .expect("safe run should start");
    assert_eq!(safe_run.status, RunStatus::WaitingForApproval);
    assert_eq!(safe_run.runtime_profile_id.as_str(), "runtime-codex-safe");

    let approvals = client
        .list_approvals(ListApprovalsQuery {
            run_id: None,
            approval_id: None,
        })
        .expect("approval list should load");
    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].run_id, safe_run.id);

    let artifacts = client
        .list_artifacts(ListArtifactsQuery {
            run_id: None,
            artifact_id: None,
        })
        .expect("artifact list should load");
    assert!(artifacts.items.is_empty());

    let approved = client
        .decide_approval(DaemonApprovalDecideParams {
            approval_id: approvals.items[0].id.clone(),
            decision: ta_protocol::wire::ApprovalDecision::Approved,
            commentary: None,
        })
        .expect("approval should resolve");
    assert_eq!(approved.run.status, RunStatus::Running);
    assert_eq!(
        approved.run.runtime_profile_id.as_str(),
        "runtime-codex-safe"
    );

    let allow_snapshot = client
        .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-allow")
                .expect("runtime profile id"),
        })
        .expect("allow runtime profile selection should succeed");
    assert_eq!(
        allow_snapshot.selection.runtime_profile_id.as_str(),
        "runtime-codex-allow"
    );

    let session_b = open_and_attach_session(&mut client, "Codex Allow");
    let allow_run = client
        .start_run(ta_protocol::wire::StartRunCommand {
            objective: "Validate allow policy".to_string(),
            ..ta_protocol::wire::StartRunCommand::default()
        })
        .expect("allow run should start");
    assert_eq!(allow_run.status, RunStatus::Running);
    assert_eq!(allow_run.runtime_profile_id.as_str(), "runtime-codex-allow");

    let patched = client
        .patch_agent_runtime_profile(DaemonAgentRuntimePatchProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-allow")
                .expect("runtime profile id"),
            patch: RuntimeProfilePatch {
                policy_mode: Some(RuntimePolicyMode::RequireApproval),
                ..Default::default()
            },
        })
        .expect("patching selected runtime profile should succeed");
    assert_eq!(
        patched.selection.runtime_profile_id.as_str(),
        "runtime-codex-allow"
    );
    assert_eq!(
        patched
            .runtime_profiles
            .iter()
            .find(|profile| profile.id.as_str() == "runtime-codex-allow")
            .expect("patched runtime profile should exist")
            .policy_mode,
        RuntimePolicyMode::RequireApproval
    );

    let session_c = open_and_attach_session(&mut client, "Codex Patched");
    let patched_run = client
        .start_run(ta_protocol::wire::StartRunCommand {
            objective: "Validate patched policy".to_string(),
            ..ta_protocol::wire::StartRunCommand::default()
        })
        .expect("patched run should start");
    assert_eq!(patched_run.status, RunStatus::WaitingForApproval);
    assert_eq!(
        patched_run.runtime_profile_id.as_str(),
        "runtime-codex-allow"
    );

    let overview = client
        .session_overview(SessionOverviewQuery::default())
        .expect("session overview should load");
    assert_session_latest_run_profile(&overview, &session_a, "runtime-codex-safe");
    assert_session_latest_run_profile(&overview, &session_b, "runtime-codex-allow");
    assert_session_latest_run_profile(&overview, &session_c, "runtime-codex-allow");
    assert!(
        overview
            .sessions
            .iter()
            .any(|session| !session.recent_activity.is_empty()),
        "session overview should surface daemon-owned recent activity for Mission Control"
    );
}

#[test]
#[cfg(unix)]
#[ignore = "requires local codex auth on the developer machine"]
fn manual_real_codex_runtime_snapshot_and_run_surface() {
    let socket_name = unique_name("ta-agent-runtime-real-codex");
    let mut daemon = ManagedDaemon::spawn_with_real_home(&socket_name, Vec::new());

    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before real codex validation");
    let mut client = initialized_client(daemon.client_config(), "ta-agent-runtime-real-codex");

    let snapshot = client
        .get_agent_runtime()
        .expect("runtime snapshot should load with real codex");
    let provider = codex_provider(&snapshot);
    assert!(
        provider.health.status == AgentRuntimeStrategyHealthStatus::Ready,
        "expected real codex provider to be ready, got {:?} {:?}",
        provider.health.status,
        provider.health.message
    );
    assert_eq!(
        auth_profile(&snapshot, "auth-codex-chatgpt").connection_state,
        AuthProfileConnectionState::Connected
    );

    client
        .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
        })
        .expect("runtime-codex-safe selection should succeed");
    let session_a = open_and_attach_session(&mut client, "Real Codex Safe");
    let safe_run = client
        .start_run(ta_protocol::wire::StartRunCommand {
            objective: "Validate real codex safe policy".to_string(),
            ..ta_protocol::wire::StartRunCommand::default()
        })
        .expect("safe run should start");
    assert_eq!(safe_run.status, RunStatus::WaitingForApproval);
    assert_eq!(safe_run.runtime_profile_id.as_str(), "runtime-codex-safe");

    let approvals = client
        .list_approvals(ListApprovalsQuery {
            run_id: None,
            approval_id: None,
        })
        .expect("approval list should load");
    assert_eq!(approvals.items.len(), 1);
    client
        .decide_approval(DaemonApprovalDecideParams {
            approval_id: approvals.items[0].id.clone(),
            decision: ta_protocol::wire::ApprovalDecision::Approved,
            commentary: None,
        })
        .expect("approval should resolve");

    client
        .select_agent_runtime_profile(DaemonAgentRuntimeSelectProfileParams {
            runtime_profile_id: RuntimeProfileId::new("runtime-codex-allow")
                .expect("runtime profile id"),
        })
        .expect("runtime-codex-allow selection should succeed");
    let session_b = open_and_attach_session(&mut client, "Real Codex Allow");
    let allow_run = client
        .start_run(ta_protocol::wire::StartRunCommand {
            objective: "Validate real codex allow policy".to_string(),
            ..ta_protocol::wire::StartRunCommand::default()
        })
        .expect("allow run should start");
    assert_eq!(allow_run.status, RunStatus::Failed);
    assert_eq!(allow_run.runtime_profile_id.as_str(), "runtime-codex-allow");

    let overview = client
        .session_overview(SessionOverviewQuery::default())
        .expect("session overview should load");
    assert_session_latest_run_profile(&overview, &session_a, "runtime-codex-safe");
    assert_session_latest_run_profile(&overview, &session_b, "runtime-codex-allow");
}

fn initialized_client(config: ClientConfig, client_name: &str) -> PersistentDaemonClient {
    let mut client =
        PersistentDaemonClient::connect(config, client_name.to_string()).expect("connect");
    client
        .initialize(client_name, "0.0.1", None)
        .expect("initialize");
    client
}

fn open_and_attach_session(client: &mut PersistentDaemonClient, title: &str) -> SessionId {
    let opened = client
        .open_session(title, ta_store::default_test_workspace_id())
        .expect("open session");
    client
        .attach_session(opened.session.id.clone())
        .expect("attach session");
    opened.session.id
}

fn codex_provider(snapshot: &AgentRuntimeSnapshot) -> &ta_protocol::wire::AgentRuntimeStrategyInfo {
    snapshot
        .providers
        .iter()
        .find(|provider| provider.id.as_str() == "codex")
        .expect("codex provider should exist")
}

fn auth_profile<'a>(
    snapshot: &'a AgentRuntimeSnapshot,
    auth_profile_id: &str,
) -> &'a ta_protocol::wire::AuthProfileState {
    snapshot
        .auth_profiles
        .iter()
        .find(|profile| profile.profile.id.as_str() == auth_profile_id)
        .expect("auth profile should exist")
}

fn assert_session_latest_run_profile(
    overview: &ta_protocol::wire::SessionOverviewResult,
    session_id: &SessionId,
    runtime_profile_id: &str,
) {
    let session = overview
        .sessions
        .iter()
        .find(|session| session.session.id == *session_id)
        .expect("session overview should contain session");
    assert_eq!(
        session
            .latest_run
            .as_ref()
            .expect("session should have latest run")
            .runtime_profile_id
            .as_str(),
        runtime_profile_id
    );
}

#[derive(Debug)]
struct ManagedDaemon {
    child: Option<Child>,
    socket_name: String,
    runtime_dir: PathBuf,
}

impl ManagedDaemon {
    fn spawn_with_env_owned(socket_name: &str, extra_env: Vec<(String, String)>) -> Self {
        Self::spawn_with_env_and_home(socket_name, extra_env, None)
    }

    fn spawn_with_real_home(socket_name: &str, extra_env: Vec<(String, String)>) -> Self {
        Self::spawn_with_env_and_home(
            socket_name,
            extra_env,
            std::env::var("HOME").ok().map(PathBuf::from),
        )
    }

    fn spawn_with_env_and_home(
        socket_name: &str,
        extra_env: Vec<(String, String)>,
        home_override: Option<PathBuf>,
    ) -> Self {
        let root_dir = test_temp_dir(socket_name);
        let config_home = home_override.unwrap_or_else(|| root_dir.join("home"));
        let runtime_dir = PathBuf::from("/tmp/tg-runtime");
        let log_dir = root_dir.join("logs");
        fs::create_dir_all(&config_home).expect("config home");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::create_dir_all(&log_dir).expect("log dir");
        // Slice 2 hard-cuts session creation without a workspace. Until the
        // slice 3 `daemon.workspace.open` RPC ships, side-load the canonical
        // default test workspace into the daemon's store before the daemon
        // process opens it.
        seed_default_test_workspace_under_home(&config_home, socket_name);

        let mut command = Command::new(env!("CARGO_BIN_EXE_ta-daemon"));
        command
            .env(DAEMON_SOCKET_NAME_ENV_VAR, socket_name)
            .env(DAEMON_RUNTIME_MODE_ENV_VAR, "local")
            .env("HOME", &config_home)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env(LOG_DIR_ENV_VAR, &log_dir)
            .env(LOG_STDERR_ENV_VAR, "0")
            .env_remove("OPENAI_API_KEY")
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
        }
    }

    fn client_config(&self) -> ClientConfig {
        ClientConfig {
            service_name: "ta-agent-runtime-validation".to_string(),
            socket_address: daemon_socket_address(&self.runtime_dir, &self.socket_name),
            io_timeout: Duration::from_secs(30),
        }
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

#[cfg(unix)]
fn daemon_socket_address(runtime_dir: &Path, socket_name: &str) -> SocketAddress {
    SocketAddress::Unix(runtime_dir.join(format!("{socket_name}.sock")))
}

#[cfg(windows)]
fn daemon_socket_address(_runtime_dir: &Path, socket_name: &str) -> SocketAddress {
    SocketAddress::NamedPipe(socket_name.to_string())
}

#[derive(Debug)]
struct FakeCodex {
    dir: PathBuf,
}

impl FakeCodex {
    #[cfg(unix)]
    fn new(initial_state: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let dir = test_temp_dir("fake-codex");
        let state_path = dir.join("state");
        fs::create_dir_all(&dir).expect("fake codex dir");
        fs::write(&state_path, initial_state).expect("fake codex state");
        let script_path = dir.join("codex");
        let script = format!(
            "#!/bin/sh\nSTATE=\"{}\"\nread_state() {{\n  if [ -f \"$STATE\" ]; then\n    cat \"$STATE\"\n  else\n    printf 'logged-out'\n  fi\n}}\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then\n  state=$(read_state)\n  if [ \"$state\" = \"chatgpt\" ]; then\n    printf 'Logged in using ChatGPT\\n'\n    exit 0\n  fi\n  if [ \"$state\" = \"api-key\" ]; then\n    printf 'Logged in using API key\\n'\n    exit 0\n  fi\n  printf 'Not logged in\\n'\n  exit 0\nfi\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"--with-api-key\" ]; then\n  read key\n  if [ -z \"$key\" ]; then\n    printf 'missing api key\\n' 1>&2\n    exit 1\n  fi\n  printf 'api-key' > \"$STATE\"\n  exit 0\nfi\nif [ \"$1\" = \"login\" ]; then\n  printf 'chatgpt' > \"$STATE\"\n  exit 0\nfi\nif [ \"$1\" = \"logout\" ]; then\n  printf 'logged-out' > \"$STATE\"\n  exit 0\nfi\nif [ \"$1\" = \"exec\" ]; then\n  cat >/dev/null\n  printf '%s\\n' '{{\"type\":\"turn.started\"}}'\n  printf '%s\\n' '{{\"type\":\"item.completed\",\"item\":{{\"id\":\"m1\",\"type\":\"agent_message\",\"text\":\"Fake codex completed run\"}}}}'\n  printf '%s\\n' '{{\"type\":\"item.completed\",\"item\":{{\"id\":\"c1\",\"type\":\"command_execution\",\"command\":\"printf hi\",\"aggregated_output\":\"hi\",\"exit_code\":0,\"status\":\"completed\"}}}}'\n  exit 0\nfi\nprintf 'unexpected codex invocation: %s %s %s\\n' \"$1\" \"$2\" \"$3\" 1>&2\nexit 1\n",
            state_path.display()
        );
        fs::write(&script_path, script).expect("fake codex script");
        let mut permissions = fs::metadata(&script_path)
            .expect("fake codex metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("fake codex permissions");
        Self { dir }
    }

    fn bin_dir(&self) -> &Path {
        &self.dir
    }

    fn binary_path(&self) -> PathBuf {
        self.dir.join("codex")
    }
}

fn fake_codex_env(fake_codex: &FakeCodex) -> Vec<(String, String)> {
    vec![
        ("PATH".to_string(), prepend_path(fake_codex.bin_dir())),
        (
            TAUGENTIC_CODEX_APP_SERVER_BIN_ENV.to_string(),
            fake_codex.binary_path().display().to_string(),
        ),
    ]
}

fn prepend_path(dir: &Path) -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    if current.is_empty() {
        dir.display().to_string()
    } else {
        format!("{}:{current}", dir.display())
    }
}

fn seed_default_test_workspace_under_home(config_home: &Path, socket_name: &str) {
    let store_path = if cfg!(target_os = "macos") {
        config_home
            .join("Library")
            .join("Application Support")
            .join("taugentic")
            .join("daemon")
            .join("store")
            .join(format!("{socket_name}.sqlite3"))
    } else if cfg!(windows) {
        config_home
            .join("AppData")
            .join("Roaming")
            .join("taugentic")
            .join("daemon")
            .join("store")
            .join(format!("{socket_name}.sqlite3"))
    } else {
        config_home
            .join(".local")
            .join("share")
            .join("taugentic")
            .join("daemon")
            .join("store")
            .join(format!("{socket_name}.sqlite3"))
    };
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent).expect("daemon store parent");
    }
    let mut store = ta_store::SqliteStore::open(&store_path).expect("open store");
    ta_store::WorkspaceRepository::upsert_workspace(&mut store, ta_store::default_test_workspace())
        .expect("seed default workspace");
}

fn test_temp_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/agent-runtime-validation")
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
