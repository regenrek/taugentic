use super::*;

pub struct ManagedDaemon {
    child: Option<Child>,
    pub socket_name: String,
    config_home: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_dir: PathBuf,
    cleanup_root_on_drop: bool,
}

impl ManagedDaemon {
    pub fn spawn(socket_name: &str) -> Self {
        Self::spawn_with_root(socket_name, test_temp_dir(socket_name), &[], true)
    }

    pub fn spawn_with_env(socket_name: &str, extra_env: &[(&str, &str)]) -> Self {
        Self::spawn_with_root(socket_name, test_temp_dir(socket_name), extra_env, true)
    }

    pub fn spawn_in_existing_root(
        socket_name: &str,
        root_dir: PathBuf,
        extra_env: &[(&str, &str)],
    ) -> Self {
        Self::spawn_with_root(socket_name, root_dir, extra_env, false)
    }

    pub fn spawn_with_root(
        socket_name: &str,
        root_dir: PathBuf,
        extra_env: &[(&str, &str)],
        cleanup_root_on_drop: bool,
    ) -> Self {
        let config_home = config_home_for_root(&root_dir);
        let runtime_dir = PathBuf::from("/tmp/tg-runtime");
        let log_dir = root_dir.join("logs");
        fs::create_dir_all(&config_home).expect("test config home should exist");
        fs::create_dir_all(config_base_dir_for_root(&root_dir))
            .expect("test config base dir should exist");
        fs::create_dir_all(&runtime_dir).expect("test runtime dir should exist");
        fs::create_dir_all(&log_dir).expect("test log dir should exist");
        // Slice 2 hard-cuts session creation without a workspace. Until slice 3
        // ships `daemon.workspace.open`, integration tests pre-seed the
        // canonical default test workspace by side-loading the daemon's store
        // before the daemon process opens it.
        seed_default_test_workspace_for_daemon(&root_dir, socket_name);

        let mut command = Command::new(env!("CARGO_BIN_EXE_ta-daemon"));
        command
            .env(DAEMON_SOCKET_NAME_ENV_VAR, socket_name)
            .env(DAEMON_RUNTIME_MODE_ENV_VAR, "local")
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("RUST_LOG", "info")
            .env(LOG_DIR_ENV_VAR, &log_dir)
            .env(LOG_STDERR_ENV_VAR, "0")
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        apply_isolated_config_env(&mut command, &root_dir);
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let child = command.spawn().expect("ta-daemon binary should spawn");

        Self {
            child: Some(child),
            socket_name: socket_name.to_string(),
            config_home,
            runtime_dir,
            log_dir,
            cleanup_root_on_drop,
        }
    }

    pub fn client(&self) -> JsonRpcClient {
        JsonRpcClient::new(ClientConfig {
            service_name: "ta-orchestrator-tests".to_string(),
            socket_address: daemon_socket_address(&self.runtime_dir, &self.socket_name),
            io_timeout: Duration::from_secs(30),
        })
    }

    pub fn wait_for_status(&mut self) -> Result<DaemonStatusResult, String> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            self.fail_if_exited()?;

            match self
                .client()
                .call::<_, DaemonStatusResult>(METHOD_DAEMON_STATUS, &DaemonStatusParams {})
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

    pub fn fail_if_exited(&mut self) -> Result<(), String> {
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

    pub fn wait_with_output(&mut self) -> io::Result<Output> {
        self.child
            .take()
            .expect("daemon child should exist")
            .wait_with_output()
    }

    pub fn shutdown(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(unix)]
pub fn daemon_socket_address(runtime_dir: &Path, socket_name: &str) -> ta_jsonrpc::SocketAddress {
    ta_jsonrpc::SocketAddress::Unix(runtime_dir.join(format!("{socket_name}.sock")))
}

#[cfg(windows)]
pub fn daemon_socket_address(_: &Path, socket_name: &str) -> ta_jsonrpc::SocketAddress {
    ta_jsonrpc::SocketAddress::NamedPipe(socket_name.to_string())
}

impl Drop for ManagedDaemon {
    fn drop(&mut self) {
        self.shutdown();
        if self.cleanup_root_on_drop
            && let Some(root_dir) = self.config_home.parent()
        {
            let _ = fs::remove_dir_all(root_dir);
        }
    }
}

pub fn spawn_conflicting_daemon(socket_name: &str) -> Result<Output, String> {
    spawn_daemon_with_env(socket_name, &[])
}

pub fn spawn_daemon_with_env(
    socket_name: &str,
    extra_env: &[(&str, &str)],
) -> Result<Output, String> {
    let root_dir = test_temp_dir("ta-daemon-conflict");
    let log_dir = root_dir.join("logs");
    let runtime_dir = PathBuf::from("/tmp/tg-runtime");
    fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&runtime_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(config_base_dir_for_root(&root_dir)).map_err(|error| error.to_string())?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_ta-daemon"));
    command
        .env(DAEMON_SOCKET_NAME_ENV_VAR, socket_name)
        .env(DAEMON_RUNTIME_MODE_ENV_VAR, "local")
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .env(LOG_DIR_ENV_VAR, &log_dir)
        .env(LOG_STDERR_ENV_VAR, "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    apply_isolated_config_env(&mut command, &root_dir);
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn conflicting daemon: {error}"))?;

    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if child
            .try_wait()
            .map_err(|error| format!("failed to poll conflicting daemon: {error}"))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("failed to collect conflicting daemon output: {error}"))?;
            let _ = fs::remove_dir_all(&root_dir);
            return Ok(output);
        }
        thread::sleep(POLL_INTERVAL);
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&root_dir);
    Err("timed out waiting for conflicting daemon to exit".to_string())
}

pub fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    if cfg!(windows) {
        let compact = prefix.strip_prefix("ta-daemon-it-").unwrap_or(prefix);
        let compact: String = compact.chars().take(24).collect();
        return format!("ta-it-{compact}-{nanos}");
    }
    format!("{prefix}-{nanos}")
}

pub fn reserve_tcp_address() -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("should reserve local tcp address");
    let address = listener
        .local_addr()
        .expect("reserved tcp listener should have local addr");
    address.to_string()
}

pub fn test_temp_dir(prefix: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-artifacts/daemon-integration")
        .join(unique_name(prefix))
}

pub fn apply_isolated_config_env(command: &mut Command, root_dir: &Path) {
    let config_home = config_home_for_root(root_dir);
    command
        .env("HOME", &config_home)
        .env("USERPROFILE", &config_home)
        .env("XDG_CONFIG_HOME", config_home.join(".config"))
        .env("APPDATA", config_home.join("AppData").join("Roaming"));
}

pub fn config_home_for_root(root_dir: &Path) -> PathBuf {
    root_dir.join("home")
}

pub fn config_base_dir_for_root(root_dir: &Path) -> PathBuf {
    let config_home = config_home_for_root(root_dir);
    match std::env::consts::OS {
        "macos" | "darwin" => config_home.join("Library").join("Application Support"),
        "windows" => config_home.join("AppData").join("Roaming"),
        _ => config_home.join(".config"),
    }
}

pub fn runtime_control_state_path_for_root(root_dir: &Path) -> PathBuf {
    config_base_dir_for_root(root_dir).join("taugentic/daemon/runtime-control.json")
}

pub fn store_path_for_root(root_dir: &Path, socket_name: &str) -> PathBuf {
    config_base_dir_for_root(root_dir)
        .join("taugentic")
        .join("daemon")
        .join("store")
        .join(format!("{socket_name}.sqlite3"))
}

/// Pre-seed the canonical default test workspace into the daemon's store so
/// integration tests can call `daemon.session.open` with the default workspace
/// id before slice 3 introduces `daemon.workspace.open`.
pub fn seed_default_test_workspace_for_daemon(root_dir: &Path, socket_name: &str) {
    let store_path = store_path_for_root(root_dir, socket_name);
    if let Some(parent) = store_path.parent() {
        fs::create_dir_all(parent).expect("daemon store parent should exist");
    }
    let mut store = SqliteStore::open(&store_path).expect("daemon store should open");
    ta_store::WorkspaceRepository::upsert_workspace(&mut store, ta_store::default_test_workspace())
        .expect("seed default test workspace");
}

pub fn wait_for_daily_log_file(log_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        match fs::read_dir(log_dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|error| {
                        format!(
                            "failed to read daemon log directory {}: {error}",
                            log_dir.display()
                        )
                    })?;
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    if name == file_name || name.starts_with(&format!("{file_name}.")) {
                        return Ok(path);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read daemon log directory {}: {error}",
                    log_dir.display()
                ));
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for daemon log file in {}",
        log_dir.display()
    ))
}

pub fn wait_for_log_entries(log_path: &Path, messages: &[&str]) -> Result<Vec<Value>, String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut last_contents = String::new();
    while Instant::now() < deadline {
        match fs::read_to_string(log_path) {
            Ok(contents) => {
                last_contents = contents.clone();
                let parsed = parse_jsonl(&contents)?;
                if messages
                    .iter()
                    .all(|message| find_log_entry(&parsed, message).is_some())
                {
                    return Ok(parsed);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to read daemon log file {}: {error}",
                    log_path.display()
                ));
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for startup log records in {}; last contents: {}",
        log_path.display(),
        last_contents
    ))
}

pub fn parse_jsonl(contents: &str) -> Result<Vec<Value>, String> {
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<Value>(line).map_err(|error| {
                format!("failed to parse daemon log line as json: {error}; line={line}")
            })
        })
        .collect()
}

pub fn find_log_entry<'a>(entries: &'a [Value], message: &str) -> Option<&'a Value> {
    entries.iter().find(|entry| {
        entry
            .get("fields")
            .and_then(|fields| fields.get("message"))
            .and_then(Value::as_str)
            == Some(message)
    })
}
