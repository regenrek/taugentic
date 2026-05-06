use std::{env, path::PathBuf};

#[cfg(test)]
use std::fs;

use ta_jsonrpc::SocketAddress;

#[cfg(test)]
use std::sync::{LazyLock, Mutex};

pub const DAEMON_LOG_FILE_NAME: &str = "ta-daemon.log.jsonl";
pub const DAEMON_CONTROL_TOKEN_ENV_VAR: &str = "TAUGENTIC_DAEMON_CONTROL_TOKEN";
pub const DAEMON_RUNTIME_MODE_ENV_VAR: &str = "TAUGENTIC_DAEMON_RUNTIME_MODE";
pub const DAEMON_RUNTIME_MODE_CONFIG_RELATIVE_PATH: &str = "taugentic/daemon/runtime-mode";
pub const RUNTIME_CONTROL_STATE_CONFIG_RELATIVE_PATH: &str =
    "taugentic/daemon/runtime-control.json";

#[cfg(test)]
static TEST_CONFIG_BASE_DIR: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));
#[cfg(test)]
static TEST_CONFIG_HOME_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub fn daemon_log_path_for_socket_address(address: &SocketAddress) -> PathBuf {
    let socket_name = socket_name_for_address(address);
    let default_directory = daemon_log_dir_for_socket_address(address, &socket_name);
    default_directory.join(DAEMON_LOG_FILE_NAME)
}

pub fn daemon_runtime_mode_file_path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = test_config_base_dir_override() {
        return path.join(DAEMON_RUNTIME_MODE_CONFIG_RELATIVE_PATH);
    }

    daemon_runtime_mode_file_path_from_env(
        env::consts::OS,
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
        env::var_os("APPDATA"),
        env::var_os("USERPROFILE"),
    )
}

pub fn runtime_control_state_file_path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = test_config_base_dir_override() {
        return path.join(RUNTIME_CONTROL_STATE_CONFIG_RELATIVE_PATH);
    }

    runtime_control_state_file_path_from_env(
        env::consts::OS,
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
        env::var_os("APPDATA"),
        env::var_os("USERPROFILE"),
    )
}

#[cfg(test)]
pub fn with_test_config_home<T>(label: &str, f: impl FnOnce() -> T) -> T {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
        time::{SystemTime, UNIX_EPOCH},
    };

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!("taugentic-test-config-{label}-{nanos}"));
    fs::create_dir_all(&root).expect("test config dir should be creatable");
    let _guard = TEST_CONFIG_HOME_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());

    {
        let mut override_dir = TEST_CONFIG_BASE_DIR
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *override_dir = Some(root.clone());
    }

    let result = catch_unwind(AssertUnwindSafe(f));

    {
        let mut override_dir = TEST_CONFIG_BASE_DIR
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *override_dir = None;
    }
    let _ = fs::remove_dir_all(&root);

    match result {
        Ok(value) => value,
        Err(panic) => resume_unwind(panic),
    }
}

fn daemon_runtime_mode_file_path_from_env(
    platform: &str,
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    appdata: Option<std::ffi::OsString>,
    user_profile: Option<std::ffi::OsString>,
) -> PathBuf {
    config_base_dir_from_env(platform, xdg_config_home, home, appdata, user_profile)
        .join(DAEMON_RUNTIME_MODE_CONFIG_RELATIVE_PATH)
}

fn runtime_control_state_file_path_from_env(
    platform: &str,
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    appdata: Option<std::ffi::OsString>,
    user_profile: Option<std::ffi::OsString>,
) -> PathBuf {
    config_base_dir_from_env(platform, xdg_config_home, home, appdata, user_profile)
        .join(RUNTIME_CONTROL_STATE_CONFIG_RELATIVE_PATH)
}

fn config_base_dir_from_env(
    platform: &str,
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    appdata: Option<std::ffi::OsString>,
    user_profile: Option<std::ffi::OsString>,
) -> PathBuf {
    match platform {
        "macos" | "darwin" => normalize_env_path(home)
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
            .unwrap_or_else(|| env::temp_dir().join("taugentic")),
        "windows" => normalize_env_path(appdata)
            .map(PathBuf::from)
            .or_else(|| {
                normalize_env_path(user_profile)
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData").join("Roaming"))
            })
            .unwrap_or_else(|| env::temp_dir().join("taugentic")),
        _ => normalize_env_path(xdg_config_home)
            .map(PathBuf::from)
            .or_else(|| {
                normalize_env_path(home)
                    .map(PathBuf::from)
                    .map(|home| home.join(".config"))
            })
            .unwrap_or_else(|| env::temp_dir().join("taugentic")),
    }
}

pub(crate) fn normalize_env_path(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn socket_name_for_address(address: &SocketAddress) -> String {
    match address {
        SocketAddress::Unix(path) => path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "ta-daemon".to_string()),
        SocketAddress::NamedPipe(name) => name.clone(),
    }
}

fn daemon_log_dir_for_socket_address(address: &SocketAddress, socket_name: &str) -> PathBuf {
    let base_dir = match address {
        SocketAddress::Unix(path) => path
            .parent()
            .map(|parent| parent.join("taugentic-daemon"))
            .unwrap_or_else(|| env::temp_dir().join("taugentic-daemon")),
        SocketAddress::NamedPipe(_) => env::temp_dir().join("taugentic-daemon"),
    };

    base_dir.join(socket_name)
}

#[cfg(test)]
fn test_config_base_dir_override() -> Option<PathBuf> {
    TEST_CONFIG_BASE_DIR
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_default_log_directory_from_unix_socket_address() {
        let path = daemon_log_path_for_socket_address(&SocketAddress::Unix(PathBuf::from(
            "/tmp/ta-daemon-custom.sock",
        )));

        assert_eq!(
            path,
            PathBuf::from("/tmp/taugentic-daemon/ta-daemon-custom/ta-daemon.log.jsonl")
        );
    }

    #[test]
    fn daemon_runtime_mode_file_path_uses_macos_app_support() {
        let path = daemon_runtime_mode_file_path_from_env(
            "macos",
            None,
            Some("/Users/kevin".into()),
            None,
            None,
        );

        assert_eq!(
            path,
            PathBuf::from("/Users/kevin/Library/Application Support/taugentic/daemon/runtime-mode")
        );
    }

    #[test]
    fn daemon_runtime_mode_file_path_uses_linux_xdg_config_home() {
        let path = daemon_runtime_mode_file_path_from_env(
            "linux",
            Some("/tmp/xdg-config".into()),
            Some("/home/kevin".into()),
            None,
            None,
        );

        assert_eq!(
            path,
            PathBuf::from("/tmp/xdg-config/taugentic/daemon/runtime-mode")
        );
    }
}
