use std::{
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use ta_observability::{
    FileLogOutput, LOG_DIR_ENV_VAR, LOG_STDERR_ENV_VAR, LogFormat, ObservabilityConfig,
    ObservabilityConfigError, ObservabilityEnvInputs, parse_bool_env, select_file_output,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    DAEMON_CONTROL_TOKEN_ENV_VAR, DAEMON_DEFAULT_SOCKET_NAME, DAEMON_RUNTIME_MODE_ENV_VAR,
    DAEMON_SOCKET_NAME_ENV_VAR, DaemonControlConfigError, DaemonRuntimeMode,
    HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR, HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR,
    HANDOFF_EXPECTED_OP_ID_ENV_VAR, ServerConfig, SocketAddress,
    daemon_log_path_for_socket_address, resolve_local_endpoint_name,
    runtime_control_state_file_path,
};

pub const DAEMON_REMOTE_WS_ENABLED_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_ENABLED";
pub const DAEMON_REMOTE_WS_BIND_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_BIND";
pub const DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR: &str = "TAUGENTIC_DAEMON_REMOTE_WS_AUTH_TOKEN";
pub const DAEMON_REMOTE_WS_DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:42321";
pub const DAEMON_REMOTE_WS_PATH: &str = "/rpc";

const MIN_REMOTE_AUTH_TOKEN_LEN: usize = 16;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlToken(String);

impl ControlToken {
    pub(crate) fn new(token: String) -> Self {
        Self(token)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for ControlToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ControlToken([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteAuthToken(String);

impl RemoteAuthToken {
    pub(crate) fn new(token: String) -> Self {
        Self(token)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for RemoteAuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RemoteAuthToken([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeControlHandoffExpectation {
    pub expected_transition_op_id: Option<u64>,
    pub expected_daemon_instance_id: Option<String>,
    pub expected_control_token: Option<ControlToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteWebsocketConfig {
    pub bind_address: SocketAddr,
    pub auth_token: RemoteAuthToken,
    pub path: String,
}

impl RemoteWebsocketConfig {
    #[cfg(test)]
    pub fn endpoint_url(&self) -> String {
        format!("ws://{}{}", self.bind_address, self.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketNameSource {
    Default,
    EnvOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModeSource {
    Default,
    Persisted,
    EnvOverride,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeMode {
    pub value: DaemonRuntimeMode,
    pub source: RuntimeModeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfigInputs {
    pub socket_name_override: Option<String>,
    pub runtime_mode_env: Option<std::ffi::OsString>,
    pub persisted_runtime_mode: Option<DaemonRuntimeMode>,
    pub control_token: Option<String>,
    pub remote_enabled: Option<std::ffi::OsString>,
    pub remote_bind_address: Option<String>,
    pub remote_auth_token: Option<String>,
    pub observability: ObservabilityEnvInputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DaemonEnvConfigInputs {
    pub socket_name_override: Option<String>,
    pub runtime_mode_env: Option<std::ffi::OsString>,
    pub control_token: Option<String>,
    pub remote_enabled: Option<std::ffi::OsString>,
    pub remote_bind_address: Option<String>,
    pub remote_auth_token: Option<String>,
    pub observability: ObservabilityEnvInputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDaemonConfig {
    pub socket_name: String,
    pub socket_name_source: SocketNameSource,
    pub server: ServerConfig,
    pub runtime_mode: ResolvedRuntimeMode,
    pub control_token: Option<ControlToken>,
    pub remote: Option<RemoteWebsocketConfig>,
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Error)]
pub enum DaemonConfigError {
    #[error(transparent)]
    Observability(#[from] ObservabilityConfigError),
    #[error(transparent)]
    ControlPlane(#[from] DaemonControlConfigError),
    #[error("invalid remote websocket enable flag in {env_var}; expected 0/1/true/false")]
    InvalidRemoteEnableFlag {
        env_var: &'static str,
        value: String,
    },
    #[error("invalid remote websocket bind address in {env_var}: {value:?}")]
    InvalidRemoteBindAddress {
        env_var: &'static str,
        value: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error(
        "remote websocket bind address in {env_var} must be loopback-only while plaintext ws transport is used: {value}"
    )]
    InsecureRemoteBindAddress {
        env_var: &'static str,
        value: String,
    },
    #[error(
        "remote websocket auth token env var {env_var} must be set when remote websocket transport is enabled"
    )]
    MissingRemoteAuthToken { env_var: &'static str },
    #[error(
        "remote websocket auth token from {env_var} must be at least {min_len} non-whitespace ASCII characters"
    )]
    InvalidRemoteAuthToken {
        env_var: &'static str,
        min_len: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonConfig {
    socket_name: String,
    store_path: PathBuf,
    pub server: ServerConfig,
    pub runtime_mode: DaemonRuntimeMode,
    pub control_token: Option<ControlToken>,
    pub remote: Option<RemoteWebsocketConfig>,
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonControlLaunchConfig {
    pub socket_name: String,
    pub log_path: PathBuf,
    pub control_token: Option<ControlToken>,
    pub remote: Option<RemoteWebsocketConfig>,
}

impl DaemonControlLaunchConfig {
    pub fn log_dir(&self) -> &Path {
        self.log_path
            .parent()
            .expect("daemon log path should have parent")
    }

    pub fn with_control_token(&self, control_token: Option<ControlToken>) -> Self {
        let mut next = self.clone();
        next.control_token = control_token;
        next
    }

    pub fn environment(&self) -> Vec<(String, String)> {
        let mut env = vec![
            (
                LOG_DIR_ENV_VAR.to_string(),
                self.log_dir().display().to_string(),
            ),
            (LOG_STDERR_ENV_VAR.to_string(), "0".to_string()),
            (
                DAEMON_SOCKET_NAME_ENV_VAR.to_string(),
                self.socket_name.to_string(),
            ),
        ];
        if let Some(control_token) = self.control_token.as_ref() {
            env.push((
                DAEMON_CONTROL_TOKEN_ENV_VAR.to_string(),
                control_token.as_str().to_string(),
            ));
        }
        if let Some(remote) = self.remote.as_ref() {
            env.push((
                DAEMON_REMOTE_WS_ENABLED_ENV_VAR.to_string(),
                "1".to_string(),
            ));
            env.push((
                DAEMON_REMOTE_WS_BIND_ENV_VAR.to_string(),
                remote.bind_address.to_string(),
            ));
            env.push((
                DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR.to_string(),
                remote.auth_token.as_str().to_string(),
            ));
        }
        env.sort_by(|left, right| left.0.cmp(&right.0));
        env
    }
}

impl DaemonConfig {
    pub fn load() -> Result<Self, DaemonConfigError> {
        Self::from_resolved(ResolvedDaemonConfig::load()?)
    }

    pub fn socket_address(&self) -> &SocketAddress {
        &self.server.socket_address
    }

    pub fn socket_name(&self) -> &str {
        &self.socket_name
    }

    pub fn store_path(&self) -> &Path {
        &self.store_path
    }

    pub fn artifact_root(&self) -> PathBuf {
        resolved_daemon_artifact_root(&self.socket_name)
    }

    pub fn log_path(&self) -> PathBuf {
        self.observability
            .file_output
            .as_ref()
            .map(FileLogOutput::path)
            .expect("daemon config should always have file logging enabled")
    }

    pub fn daemon_control_launch_config(&self) -> DaemonControlLaunchConfig {
        DaemonControlLaunchConfig {
            socket_name: self.socket_name().to_string(),
            log_path: self.log_path(),
            control_token: self.control_token.clone(),
            remote: self.remote.clone(),
        }
    }

    fn from_resolved(resolved: ResolvedDaemonConfig) -> Result<Self, DaemonConfigError> {
        Ok(Self {
            store_path: resolved_daemon_store_path(&resolved.socket_name),
            socket_name: resolved.socket_name,
            server: resolved.server,
            runtime_mode: resolved.runtime_mode.value,
            control_token: resolved.control_token,
            remote: resolved.remote,
            observability: resolved.observability,
        })
    }
}

impl RuntimeControlHandoffExpectation {
    pub fn from_env() -> Self {
        Self::from_values(
            env::var(HANDOFF_EXPECTED_OP_ID_ENV_VAR).ok(),
            env::var(HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR).ok(),
            env::var(HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR).ok(),
        )
    }

    pub fn from_values(
        expected_transition_op_id: Option<String>,
        expected_daemon_instance_id: Option<String>,
        expected_control_token: Option<String>,
    ) -> Self {
        Self {
            expected_transition_op_id: expected_transition_op_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse::<u64>().ok()),
            expected_daemon_instance_id: expected_daemon_instance_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            expected_control_token: normalize_optional_token(expected_control_token),
        }
    }

    pub fn environment(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if let Some(expected_transition_op_id) = self.expected_transition_op_id {
            env.push((
                HANDOFF_EXPECTED_OP_ID_ENV_VAR.to_string(),
                expected_transition_op_id.to_string(),
            ));
        }
        if let Some(expected_daemon_instance_id) = self.expected_daemon_instance_id.as_ref() {
            env.push((
                HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR.to_string(),
                expected_daemon_instance_id.clone(),
            ));
        }
        if let Some(expected_control_token) = self.expected_control_token.as_ref() {
            env.push((
                HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR.to_string(),
                expected_control_token.as_str().to_string(),
            ));
        }
        env.sort_by(|left, right| left.0.cmp(&right.0));
        env
    }
}

pub fn mint_control_token() -> ControlToken {
    ControlToken::new(Uuid::new_v4().simple().to_string())
}

impl ResolvedDaemonConfig {
    pub fn load() -> Result<Self, DaemonConfigError> {
        Self::load_from_env_inputs(DaemonEnvConfigInputs::from_env())
    }

    pub fn from_inputs(inputs: DaemonConfigInputs) -> Result<Self, DaemonConfigError> {
        let (socket_name, socket_name_source) =
            resolved_socket_name_from_override(inputs.socket_name_override.as_deref());
        let server = ServerConfig::local_default("ta-daemon", &socket_name);
        let default_log_path = resolved_daemon_log_path_for_socket_address(
            &server.socket_address,
            inputs.observability.log_dir.as_deref(),
        );
        let default_file_output = file_output_for_log_path(&default_log_path);

        Ok(Self {
            socket_name,
            socket_name_source,
            server,
            runtime_mode: resolved_runtime_mode(
                inputs.runtime_mode_env,
                inputs.persisted_runtime_mode,
            )?,
            control_token: normalize_optional_token(inputs.control_token),
            remote: remote_websocket_config_from_inputs(
                inputs.remote_enabled,
                inputs.remote_bind_address,
                inputs.remote_auth_token,
            )?,
            observability: resolved_daemon_observability_config(
                "ta-daemon",
                "info",
                default_file_output,
                inputs.observability,
            )?,
        })
    }

    pub fn load_from_env_inputs(inputs: DaemonEnvConfigInputs) -> Result<Self, DaemonConfigError> {
        Self::from_inputs(DaemonConfigInputs::from_env_inputs(inputs)?)
    }

    #[cfg(test)]
    pub fn log_path(&self) -> PathBuf {
        self.observability
            .file_output
            .as_ref()
            .map(FileLogOutput::path)
            .expect("daemon config should always have file logging enabled")
    }
}

impl DaemonConfigInputs {
    pub fn from_env_inputs(inputs: DaemonEnvConfigInputs) -> Result<Self, DaemonConfigError> {
        Ok(Self {
            socket_name_override: inputs.socket_name_override,
            persisted_runtime_mode: if inputs.runtime_mode_env.is_none() {
                read_persisted_control_plane_desired_mode()?
            } else {
                None
            },
            runtime_mode_env: inputs.runtime_mode_env,
            control_token: inputs.control_token,
            remote_enabled: inputs.remote_enabled,
            remote_bind_address: inputs.remote_bind_address,
            remote_auth_token: inputs.remote_auth_token,
            observability: inputs.observability,
        })
    }
}

impl DaemonEnvConfigInputs {
    pub fn from_env() -> Self {
        Self {
            socket_name_override: env::var(DAEMON_SOCKET_NAME_ENV_VAR)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            runtime_mode_env: env::var_os(DAEMON_RUNTIME_MODE_ENV_VAR),
            control_token: env::var(DAEMON_CONTROL_TOKEN_ENV_VAR).ok(),
            remote_enabled: env::var_os(DAEMON_REMOTE_WS_ENABLED_ENV_VAR),
            remote_bind_address: env::var(DAEMON_REMOTE_WS_BIND_ENV_VAR).ok(),
            remote_auth_token: env::var(DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR).ok(),
            observability: ObservabilityEnvInputs::from_env(),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_config() -> DaemonConfig {
    let mut config = DaemonConfig::from_resolved(
        ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: Some(format!("test-{}", Uuid::new_v4().simple())),
            runtime_mode_env: None,
            persisted_runtime_mode: None,
            control_token: None,
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs {
                stderr_format: Some("json".to_string()),
                stderr_enabled: Some("0".to_string()),
                log_dir: None,
            },
        })
        .expect("resolved config should load"),
    )
    .expect("daemon config should build");
    config.store_path = isolated_test_store_path(&config.socket_name);
    config
}

#[cfg(test)]
pub(crate) fn with_test_config_home<T>(label: &str, f: impl FnOnce() -> T) -> T {
    crate::with_test_config_home(label, f)
}

#[cfg(test)]
fn isolated_test_store_path(socket_name: &str) -> PathBuf {
    env::temp_dir()
        .join("taugentic-test-store")
        .join(format!("{socket_name}.sqlite3"))
}

pub fn resolved_daemon_log_path_for_socket_address(
    address: &SocketAddress,
    log_dir_override: Option<&str>,
) -> PathBuf {
    let default_log_path = daemon_log_path_for_socket_address(address);
    match log_dir_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(directory) => PathBuf::from(directory).join(
            default_log_path
                .file_name()
                .expect("daemon log path should have file name"),
        ),
        None => default_log_path,
    }
}

pub fn resolved_daemon_store_path(socket_name: &str) -> PathBuf {
    runtime_control_state_file_path()
        .parent()
        .expect("runtime control state path should have parent")
        .join("store")
        .join(format!("{socket_name}.sqlite3"))
}

pub fn resolved_daemon_artifact_root(socket_name: &str) -> PathBuf {
    runtime_control_state_file_path()
        .parent()
        .expect("runtime control state path should have parent")
        .join("artifacts")
        .join(socket_name)
}

pub fn daemon_log_path_for_current_env(address: &SocketAddress) -> PathBuf {
    let override_directory = env::var_os(LOG_DIR_ENV_VAR)
        .map(|directory| directory.to_string_lossy().trim().to_string())
        .filter(|directory| !directory.is_empty());
    resolved_daemon_log_path_for_socket_address(address, override_directory.as_deref())
}

fn resolved_daemon_observability_config(
    service_name: &str,
    default_level: &str,
    default_file_output: FileLogOutput,
    inputs: ObservabilityEnvInputs,
) -> Result<ObservabilityConfig, DaemonConfigError> {
    let stderr_format = inputs
        .stderr_format
        .as_deref()
        .map(LogFormat::parse)
        .transpose()?
        .unwrap_or(LogFormat::Pretty);
    let stderr_enabled = inputs
        .stderr_enabled
        .as_deref()
        .map(|value| parse_bool_env(LOG_STDERR_ENV_VAR, value))
        .transpose()?
        .unwrap_or(true);
    let file_output = select_file_output(Some(default_file_output), inputs.log_dir);

    Ok(ObservabilityConfig {
        service_name: service_name.to_string(),
        default_level: default_level.to_string(),
        stderr_enabled,
        stderr_format,
        file_output,
    })
}

fn file_output_for_log_path(path: &Path) -> FileLogOutput {
    FileLogOutput {
        directory: path
            .parent()
            .expect("daemon log path should have parent")
            .to_path_buf(),
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("daemon log path should have file name")
            .to_string(),
    }
}

fn resolved_socket_name_from_override(value: Option<&str>) -> (String, SocketNameSource) {
    match value {
        Some(value) if !value.trim().is_empty() => {
            (value.trim().to_string(), SocketNameSource::EnvOverride)
        }
        _ => (
            resolve_local_endpoint_name(DAEMON_DEFAULT_SOCKET_NAME, DAEMON_SOCKET_NAME_ENV_VAR),
            SocketNameSource::Default,
        ),
    }
}

fn resolved_runtime_mode(
    env_value: Option<std::ffi::OsString>,
    persisted_mode: Option<DaemonRuntimeMode>,
) -> Result<ResolvedRuntimeMode, DaemonConfigError> {
    if let Some(value) = env_value.clone() {
        return Ok(ResolvedRuntimeMode {
            value: daemon_runtime_mode_from_input(Some(value))?,
            source: RuntimeModeSource::EnvOverride,
        });
    }

    if let Some(value) = persisted_mode {
        return Ok(ResolvedRuntimeMode {
            value: configured_runtime_mode_from_inputs(None, Some(value))?,
            source: RuntimeModeSource::Persisted,
        });
    }

    Ok(ResolvedRuntimeMode {
        value: configured_runtime_mode_from_inputs(None, None)?,
        source: RuntimeModeSource::Default,
    })
}

fn normalize_optional_token(token: Option<String>) -> Option<ControlToken> {
    token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(ControlToken::new)
}

fn configured_runtime_mode_from_inputs(
    env_value: Option<std::ffi::OsString>,
    persisted_mode: Option<DaemonRuntimeMode>,
) -> Result<DaemonRuntimeMode, DaemonConfigError> {
    if let Some(value) = env_value {
        return daemon_runtime_mode_from_input(Some(value));
    }

    Ok(persisted_mode.unwrap_or(DaemonRuntimeMode::Local))
}

fn daemon_runtime_mode_from_input(
    value: Option<std::ffi::OsString>,
) -> Result<DaemonRuntimeMode, DaemonConfigError> {
    let Some(value) = value else {
        return Ok(DaemonRuntimeMode::Local);
    };

    let value = value.to_string_lossy().trim().to_ascii_lowercase();
    parse_daemon_runtime_mode_value(&value).ok_or(DaemonConfigError::ControlPlane(
        DaemonControlConfigError::InvalidRuntimeMode {
            env_var: DAEMON_RUNTIME_MODE_ENV_VAR,
            value,
        },
    ))
}

fn parse_daemon_runtime_mode_value(value: &str) -> Option<DaemonRuntimeMode> {
    match value {
        "" | "local" => Some(DaemonRuntimeMode::Local),
        "background" => Some(DaemonRuntimeMode::Background),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedRuntimeControlConfigRecord {
    desired_mode: Option<DaemonRuntimeMode>,
}

fn read_persisted_control_plane_desired_mode()
-> Result<Option<DaemonRuntimeMode>, DaemonConfigError> {
    let path = runtime_control_state_file_path();
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(DaemonConfigError::ControlPlane(
                DaemonControlConfigError::ReadRuntimeModeFile { path, source },
            ));
        }
    };
    let record: PersistedRuntimeControlConfigRecord =
        serde_json::from_str(&contents).map_err(|_| {
            DaemonConfigError::ControlPlane(DaemonControlConfigError::InvalidRuntimeModeFile {
                path: path.clone(),
                value: contents.trim().to_string(),
            })
        })?;
    Ok(record.desired_mode)
}

fn remote_websocket_config_from_inputs(
    enabled: Option<std::ffi::OsString>,
    bind_address: Option<String>,
    auth_token: Option<String>,
) -> Result<Option<RemoteWebsocketConfig>, DaemonConfigError> {
    if !remote_websocket_enabled(enabled)? {
        return Ok(None);
    }

    remote_websocket_config_from_values(bind_address, auth_token)
}

fn remote_websocket_enabled(value: Option<std::ffi::OsString>) -> Result<bool, DaemonConfigError> {
    let Some(value) = value else {
        return Ok(false);
    };

    let value = value.to_string_lossy().trim().to_ascii_lowercase();
    match value.as_str() {
        "" | "0" | "false" => Ok(false),
        "1" | "true" => Ok(true),
        _ => Err(DaemonConfigError::InvalidRemoteEnableFlag {
            env_var: DAEMON_REMOTE_WS_ENABLED_ENV_VAR,
            value,
        }),
    }
}

fn remote_websocket_config_from_values(
    bind_address: Option<String>,
    auth_token: Option<String>,
) -> Result<Option<RemoteWebsocketConfig>, DaemonConfigError> {
    let bind_address =
        bind_address.unwrap_or_else(|| DAEMON_REMOTE_WS_DEFAULT_BIND_ADDRESS.to_string());
    let bind_address = bind_address.parse::<SocketAddr>().map_err(|source| {
        DaemonConfigError::InvalidRemoteBindAddress {
            env_var: DAEMON_REMOTE_WS_BIND_ENV_VAR,
            value: bind_address,
            source,
        }
    })?;
    validate_remote_bind_address(bind_address)?;
    let auth_token = auth_token.ok_or(DaemonConfigError::MissingRemoteAuthToken {
        env_var: DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR,
    })?;
    let auth_token = normalize_remote_auth_token(&auth_token)?;

    Ok(Some(RemoteWebsocketConfig {
        bind_address,
        auth_token,
        path: DAEMON_REMOTE_WS_PATH.to_string(),
    }))
}

fn validate_remote_bind_address(bind_address: SocketAddr) -> Result<(), DaemonConfigError> {
    if bind_address.ip().is_loopback() {
        return Ok(());
    }

    Err(DaemonConfigError::InsecureRemoteBindAddress {
        env_var: DAEMON_REMOTE_WS_BIND_ENV_VAR,
        value: bind_address.to_string(),
    })
}

fn normalize_remote_auth_token(token: &str) -> Result<RemoteAuthToken, DaemonConfigError> {
    let token = token.trim();
    if token.len() < MIN_REMOTE_AUTH_TOKEN_LEN
        || !token.chars().all(|character| character.is_ascii_graphic())
    {
        return Err(DaemonConfigError::InvalidRemoteAuthToken {
            env_var: DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR,
            min_len: MIN_REMOTE_AUTH_TOKEN_LEN,
        });
    }

    Ok(RemoteAuthToken::new(token.to_string()))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, process::Command};

    use super::*;
    use ta_observability::LogFormat;

    #[test]
    fn loaded_daemon_config_exposes_effective_log_path() {
        with_test_config_home("loaded-daemon-config", || {
            let config = DaemonConfig::load().expect("config should load");

            assert_eq!(
                config.log_path(),
                config
                    .observability
                    .file_output
                    .as_ref()
                    .expect("daemon should have file logging")
                    .path()
            );
        });
    }

    #[test]
    fn loaded_daemon_config_projects_socket_scoped_store_path() {
        with_test_config_home("loaded-daemon-config-store-path", || {
            let resolved = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
                socket_name_override: Some("socket-alpha".to_string()),
                runtime_mode_env: None,
                persisted_runtime_mode: None,
                control_token: None,
                remote_enabled: None,
                remote_bind_address: None,
                remote_auth_token: None,
                observability: ObservabilityEnvInputs {
                    stderr_format: Some("json".to_string()),
                    stderr_enabled: Some("0".to_string()),
                    log_dir: None,
                },
            })
            .expect("resolved config should load");
            let config = DaemonConfig::from_resolved(resolved).expect("daemon config should build");

            assert_eq!(
                config.store_path(),
                runtime_control_state_file_path()
                    .parent()
                    .expect("runtime control path should have parent")
                    .join("store/socket-alpha.sqlite3")
            );
        });
    }

    #[test]
    fn test_config_uses_isolated_store_path_outside_config_home_override() {
        with_test_config_home("test-config-isolated-store-path", || {
            let config = test_config();
            let daemon_config_dir = runtime_control_state_file_path()
                .parent()
                .expect("runtime control path should have parent")
                .to_path_buf();

            assert!(
                !config.store_path().starts_with(&daemon_config_dir),
                "test config store path should stay isolated from config-home override"
            );
            assert_eq!(
                config.store_path(),
                isolated_test_store_path(config.socket_name())
            );
        });
    }

    #[test]
    fn remote_websocket_config_is_disabled_by_default() {
        assert!(!remote_websocket_enabled(None).expect("config"));
    }

    #[test]
    fn remote_websocket_config_rejects_invalid_enable_flag_values() {
        let error = remote_websocket_enabled(Some("maybe".into()))
            .expect_err("invalid enable flag should fail");
        assert!(matches!(
            error,
            DaemonConfigError::InvalidRemoteEnableFlag { .. }
        ));
    }

    #[test]
    fn invalid_remote_enable_flag_display_does_not_echo_raw_value() {
        let error =
            remote_websocket_enabled(Some("raw-secret-token".into())).expect_err("invalid flag");
        let rendered = error.to_string();

        assert!(rendered.contains("invalid remote websocket enable flag"));
        assert!(rendered.contains(DAEMON_REMOTE_WS_ENABLED_ENV_VAR));
        assert!(rendered.contains("expected 0/1/true/false"));
        assert!(!rendered.contains("raw-secret-token"));
    }

    #[test]
    fn remote_websocket_config_requires_auth_token_when_enabled() {
        let error = remote_websocket_config_from_inputs(
            Some("1".into()),
            Some("127.0.0.1:43123".to_string()),
            None,
        )
        .expect_err("missing token should fail");
        assert!(matches!(
            error,
            DaemonConfigError::MissingRemoteAuthToken { .. }
        ));
    }

    #[test]
    fn remote_websocket_config_loads_explicit_bind_and_token() {
        let remote = remote_websocket_config_from_inputs(
            Some("1".into()),
            Some("127.0.0.1:43123".to_string()),
            Some("0123456789abcdef0123456789abcdef".to_string()),
        )
        .expect("remote config")
        .expect("remote config should be enabled");
        assert_eq!(
            remote.bind_address,
            "127.0.0.1:43123"
                .parse::<SocketAddr>()
                .expect("socket address should parse")
        );
        assert_eq!(remote.endpoint_url(), "ws://127.0.0.1:43123/rpc");
    }

    #[test]
    fn remote_websocket_config_uses_default_bind_and_path_when_enabled() {
        let remote = remote_websocket_config_from_inputs(
            Some("1".into()),
            None,
            Some("0123456789abcdef0123456789abcdef".to_string()),
        )
        .expect("remote config")
        .expect("remote config should be enabled");

        assert_eq!(
            remote.bind_address,
            DAEMON_REMOTE_WS_DEFAULT_BIND_ADDRESS
                .parse::<SocketAddr>()
                .expect("default bind address should parse")
        );
        assert_eq!(remote.path, DAEMON_REMOTE_WS_PATH);
    }

    #[test]
    fn remote_websocket_config_trims_stored_auth_token() {
        let remote = remote_websocket_config_from_inputs(
            Some("true".into()),
            Some("127.0.0.1:43123".to_string()),
            Some("  0123456789abcdef0123456789abcdef  ".to_string()),
        )
        .expect("remote config")
        .expect("remote config should be enabled");

        assert_eq!(
            remote.auth_token.as_str(),
            "0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn remote_websocket_config_rejects_invalid_auth_tokens() {
        for invalid_token in ["   ", "short-token", "0123456789abcde\u{00f6}"] {
            let error = remote_websocket_config_from_inputs(
                Some("true".into()),
                Some("127.0.0.1:43123".to_string()),
                Some(invalid_token.to_string()),
            )
            .expect_err("invalid auth token should fail");

            assert!(matches!(
                error,
                DaemonConfigError::InvalidRemoteAuthToken { .. }
            ));
        }
    }

    #[test]
    fn remote_websocket_config_rejects_non_loopback_bind_address() {
        let error = remote_websocket_config_from_inputs(
            Some("1".into()),
            Some("0.0.0.0:43123".to_string()),
            Some("0123456789abcdef0123456789abcdef".to_string()),
        )
        .expect_err("non-loopback bind should fail");

        assert!(matches!(
            error,
            DaemonConfigError::InsecureRemoteBindAddress { .. }
        ));
    }

    #[test]
    fn resolved_config_prefers_runtime_mode_env_over_persisted() {
        let resolved = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: None,
            runtime_mode_env: Some("background".into()),
            persisted_runtime_mode: Some(DaemonRuntimeMode::Local),
            control_token: None,
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs::default(),
        })
        .expect("config should load");

        assert_eq!(resolved.runtime_mode.value, DaemonRuntimeMode::Background);
        assert_eq!(resolved.runtime_mode.source, RuntimeModeSource::EnvOverride);
    }

    #[test]
    fn resolved_config_uses_persisted_runtime_mode_when_env_missing() {
        let resolved = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: None,
            runtime_mode_env: None,
            persisted_runtime_mode: Some(DaemonRuntimeMode::Background),
            control_token: None,
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs::default(),
        })
        .expect("config should load");

        assert_eq!(resolved.runtime_mode.value, DaemonRuntimeMode::Background);
        assert_eq!(resolved.runtime_mode.source, RuntimeModeSource::Persisted);
    }

    #[test]
    fn resolved_config_trims_empty_control_token_to_none() {
        let resolved = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: None,
            runtime_mode_env: None,
            persisted_runtime_mode: None,
            control_token: Some("   ".to_string()),
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs::default(),
        })
        .expect("config should load");

        assert_eq!(resolved.control_token, None);
    }

    #[test]
    fn resolved_config_applies_observability_overrides() {
        let resolved = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: Some("ta-daemon".to_string()),
            runtime_mode_env: None,
            persisted_runtime_mode: None,
            control_token: None,
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs {
                stderr_format: Some("json".to_string()),
                stderr_enabled: Some("0".to_string()),
                log_dir: Some("/override/logs".to_string()),
            },
        })
        .expect("config should load");

        assert_eq!(resolved.observability.stderr_format, LogFormat::Json);
        assert!(!resolved.observability.stderr_enabled);
        assert_eq!(
            resolved.log_path(),
            PathBuf::from("/override/logs/ta-daemon.log.jsonl")
        );
    }

    #[test]
    fn daemon_control_launch_config_projects_canonical_values() {
        let env = DaemonControlLaunchConfig {
            socket_name: "ta-daemon.sock".to_string(),
            log_path: PathBuf::from("/tmp/taugentic/ta-daemon.log.jsonl"),
            control_token: Some(ControlToken::new("secret-token".to_string())),
            remote: Some(RemoteWebsocketConfig {
                bind_address: "127.0.0.1:43123"
                    .parse()
                    .expect("socket address should parse"),
                auth_token: RemoteAuthToken::new("0123456789abcdef0123456789abcdef".to_string()),
                path: DAEMON_REMOTE_WS_PATH.to_string(),
            }),
        }
        .environment();

        assert_eq!(
            env,
            vec![
                (
                    DAEMON_CONTROL_TOKEN_ENV_VAR.to_string(),
                    "secret-token".to_string()
                ),
                (
                    DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR.to_string(),
                    "0123456789abcdef0123456789abcdef".to_string()
                ),
                (
                    DAEMON_REMOTE_WS_BIND_ENV_VAR.to_string(),
                    "127.0.0.1:43123".to_string()
                ),
                (
                    DAEMON_REMOTE_WS_ENABLED_ENV_VAR.to_string(),
                    "1".to_string()
                ),
                (
                    DAEMON_SOCKET_NAME_ENV_VAR.to_string(),
                    "ta-daemon.sock".to_string()
                ),
                (LOG_DIR_ENV_VAR.to_string(), "/tmp/taugentic".to_string()),
                (LOG_STDERR_ENV_VAR.to_string(), "0".to_string()),
            ]
        );
    }

    #[test]
    fn daemon_control_launch_config_omits_absent_optional_values() {
        let env = DaemonControlLaunchConfig {
            socket_name: "ta-daemon.sock".to_string(),
            log_path: PathBuf::from("/tmp/taugentic/ta-daemon.log.jsonl"),
            control_token: None,
            remote: None,
        }
        .environment();

        assert_eq!(
            env,
            vec![
                (
                    DAEMON_SOCKET_NAME_ENV_VAR.to_string(),
                    "ta-daemon.sock".to_string()
                ),
                (LOG_DIR_ENV_VAR.to_string(), "/tmp/taugentic".to_string()),
                (LOG_STDERR_ENV_VAR.to_string(), "0".to_string()),
            ]
        );
    }

    #[test]
    fn daemon_config_load_reads_trimmed_launch_values_from_env() {
        with_test_config_home("config-load-env", || {
            let resolved = ResolvedDaemonConfig::load_from_env_inputs(DaemonEnvConfigInputs {
                socket_name_override: Some("  custom-daemon.sock  ".to_string()),
                runtime_mode_env: None,
                control_token: Some("  control-token-value  ".to_string()),
                remote_enabled: Some(OsString::from(" true ")),
                remote_bind_address: Some("127.0.0.1:43123".to_string()),
                remote_auth_token: Some("  0123456789abcdef0123456789abcdef  ".to_string()),
                observability: ObservabilityEnvInputs::default(),
            })
            .expect("config should load");
            let launch = DaemonConfig::from_resolved(resolved.clone())
                .expect("daemon config should build")
                .daemon_control_launch_config();

            assert_eq!(resolved.socket_name, "custom-daemon.sock");
            assert_eq!(resolved.socket_name_source, SocketNameSource::EnvOverride);
            assert_eq!(
                resolved.control_token.as_ref().map(ControlToken::as_str),
                Some("control-token-value")
            );
            let remote = resolved.remote.as_ref().expect("remote config should load");
            assert_eq!(remote.bind_address.to_string(), "127.0.0.1:43123");
            assert_eq!(
                remote.auth_token.as_str(),
                "0123456789abcdef0123456789abcdef"
            );
            assert_eq!(launch.socket_name, "custom-daemon.sock");
            assert_eq!(
                launch.control_token.as_ref().map(ControlToken::as_str),
                Some("control-token-value")
            );
            assert_eq!(
                launch
                    .remote
                    .as_ref()
                    .expect("launch config should retain remote")
                    .bind_address
                    .to_string(),
                "127.0.0.1:43123"
            );
        });
    }

    #[test]
    fn handoff_expectation_trims_and_types_values() {
        let expected = RuntimeControlHandoffExpectation::from_values(
            Some(" 42 ".to_string()),
            Some(" daemon-1 ".to_string()),
            Some("  control-token-value  ".to_string()),
        );

        assert_eq!(expected.expected_transition_op_id, Some(42));
        assert_eq!(
            expected.expected_daemon_instance_id.as_deref(),
            Some("daemon-1")
        );
        assert_eq!(
            expected
                .expected_control_token
                .as_ref()
                .map(ControlToken::as_str),
            Some("control-token-value")
        );
    }

    #[test]
    fn handoff_expectation_drops_blank_and_invalid_values() {
        let expected = RuntimeControlHandoffExpectation::from_values(
            Some(" definitely-not-a-number ".to_string()),
            Some("   ".to_string()),
            Some("   ".to_string()),
        );

        assert_eq!(expected.expected_transition_op_id, None);
        assert_eq!(expected.expected_daemon_instance_id, None);
        assert_eq!(expected.expected_control_token, None);
    }

    #[test]
    fn handoff_expectation_projects_canonical_environment() {
        let env = RuntimeControlHandoffExpectation::from_values(
            Some("42".to_string()),
            Some("daemon-1".to_string()),
            Some("control-token".to_string()),
        )
        .environment();

        assert_eq!(
            env,
            vec![
                (
                    HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR.to_string(),
                    "control-token".to_string()
                ),
                (
                    HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR.to_string(),
                    "daemon-1".to_string()
                ),
                (HANDOFF_EXPECTED_OP_ID_ENV_VAR.to_string(), "42".to_string()),
            ]
        );
    }

    #[test]
    fn handoff_expectation_loads_typed_values_from_env() {
        if env::var_os("TAUGENTIC_TEST_HANDOFF_EXPECTATION_LOAD").is_some() {
            let expected = RuntimeControlHandoffExpectation::from_env();

            assert_eq!(expected.expected_transition_op_id, Some(42));
            assert_eq!(
                expected.expected_daemon_instance_id.as_deref(),
                Some("daemon-1")
            );
            assert_eq!(
                expected
                    .expected_control_token
                    .as_ref()
                    .map(ControlToken::as_str),
                Some("control-token")
            );
            return;
        }

        run_handoff_env_subprocess(
            "host::config::tests::handoff_expectation_loads_typed_values_from_env",
            &[
                ("TAUGENTIC_TEST_HANDOFF_EXPECTATION_LOAD", "1"),
                (HANDOFF_EXPECTED_OP_ID_ENV_VAR, " 42 "),
                (HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR, " daemon-1 "),
                (HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR, "  control-token  "),
            ],
        );
    }

    #[test]
    fn handoff_expectation_drops_invalid_env_values() {
        if env::var_os("TAUGENTIC_TEST_HANDOFF_EXPECTATION_DROP").is_some() {
            let expected = RuntimeControlHandoffExpectation::from_env();

            assert_eq!(expected.expected_transition_op_id, None);
            assert_eq!(expected.expected_daemon_instance_id, None);
            assert_eq!(expected.expected_control_token, None);
            return;
        }

        run_handoff_env_subprocess(
            "host::config::tests::handoff_expectation_drops_invalid_env_values",
            &[
                ("TAUGENTIC_TEST_HANDOFF_EXPECTATION_DROP", "1"),
                (HANDOFF_EXPECTED_OP_ID_ENV_VAR, " definitely-not-a-number "),
                (HANDOFF_EXPECTED_DAEMON_INSTANCE_ID_ENV_VAR, "   "),
                (HANDOFF_EXPECTED_CONTROL_TOKEN_ENV_VAR, "   "),
            ],
        );
    }

    #[test]
    fn resolved_config_fails_for_invalid_runtime_mode_env() {
        let error = ResolvedDaemonConfig::from_inputs(DaemonConfigInputs {
            socket_name_override: None,
            runtime_mode_env: Some("definitely-wrong".into()),
            persisted_runtime_mode: Some(DaemonRuntimeMode::Local),
            control_token: None,
            remote_enabled: None,
            remote_bind_address: None,
            remote_auth_token: None,
            observability: ObservabilityEnvInputs::default(),
        })
        .expect_err("invalid runtime mode should fail");

        assert!(matches!(error, DaemonConfigError::ControlPlane(_)));
    }

    #[test]
    fn load_fails_when_persisted_runtime_mode_file_is_malformed() {
        with_test_config_home("malformed-runtime-mode", || {
            let path = crate::runtime_control_state_file_path();
            fs::create_dir_all(path.parent().expect("parent should exist"))
                .expect("config dir should exist");
            fs::write(&path, "{ definitely not json").expect("write should succeed");

            let error =
                ResolvedDaemonConfig::load().expect_err("malformed persisted config should fail");
            assert!(matches!(error, DaemonConfigError::ControlPlane(_)));
        });
    }

    #[test]
    fn load_from_env_inputs_ignores_malformed_persisted_runtime_mode_when_env_override_is_set() {
        with_test_config_home("env-override-malformed-runtime-mode", || {
            let path = crate::runtime_control_state_file_path();
            fs::create_dir_all(path.parent().expect("parent should exist"))
                .expect("config dir should exist");
            fs::write(&path, "{ definitely not json").expect("write should succeed");

            let resolved = ResolvedDaemonConfig::load_from_env_inputs(DaemonEnvConfigInputs {
                runtime_mode_env: Some("background".into()),
                ..DaemonEnvConfigInputs::default()
            })
            .expect("env override should bypass malformed persisted config");

            assert_eq!(resolved.runtime_mode.value, DaemonRuntimeMode::Background);
            assert_eq!(resolved.runtime_mode.source, RuntimeModeSource::EnvOverride);
        });
    }

    fn run_handoff_env_subprocess(test_name: &str, env_vars: &[(&str, &str)]) {
        let mut command =
            Command::new(env::current_exe().expect("current test binary path should resolve"));
        command.arg("--exact").arg(test_name).arg("--nocapture");
        for (name, value) in env_vars {
            command.env(name, value);
        }

        let output = command
            .output()
            .expect("handoff env subprocess should execute");
        assert!(
            output.status.success(),
            "handoff env subprocess should pass\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
