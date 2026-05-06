use std::{io, path::PathBuf, time::Duration};

use interprocess::local_socket::tokio::{
    Listener as TokioLocalSocketListener, Stream as TokioLocalSocketStream,
};
use interprocess::local_socket::traits::tokio::Listener as TokioListener;
use interprocess::local_socket::{
    ConnectOptions, Listener as LocalSocketListener, ListenerOptions, Name,
    Stream as LocalSocketStream,
    traits::{Listener as _, Stream as _},
};
use ta_host_platform::{LocalIpcKind, current_capabilities};

use thiserror::Error;

const MACOS_SHORT_SOCKET_DIR: &str = "/tmp/taugentic/s";
const MACOS_MAX_SOCKET_PATH_UTF8_BYTES: usize = 103;
const FNV64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV64_PRIME: u64 = 0x100000001b3;

#[cfg(unix)]
#[path = "socket_unix.rs"]
mod imp;
#[cfg(windows)]
#[path = "socket_windows.rs"]
mod imp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketAddress {
    Unix(PathBuf),
    NamedPipe(String),
}

impl SocketAddress {
    pub fn for_current_user(endpoint_name: &str) -> Self {
        match current_capabilities().local_ipc {
            LocalIpcKind::UnixDomainSocket { runtime_dir } => {
                Self::Unix(resolve_unix_socket_path(runtime_dir, endpoint_name))
            }
            LocalIpcKind::WindowsNamedPipe => Self::NamedPipe(endpoint_name.to_string()),
        }
    }
}

impl std::fmt::Display for SocketAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unix(path) => write!(formatter, "{}", path.display()),
            Self::NamedPipe(path) => write!(formatter, r"\\.\pipe\{path}"),
        }
    }
}

pub fn resolve_local_endpoint_name(default_name: &str, env_var_name: &str) -> String {
    select_local_endpoint_name(default_name, std::env::var(env_var_name).ok().as_deref())
}

fn select_local_endpoint_name(default_name: &str, override_name: Option<&str>) -> String {
    override_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_name.to_string())
}

fn resolve_unix_socket_path(runtime_dir: PathBuf, endpoint_name: &str) -> PathBuf {
    let socket_path = runtime_dir.join(format!("{endpoint_name}.sock"));

    if cfg!(target_os = "macos") {
        return apply_macos_socket_path_guard(socket_path, endpoint_name, std::env::var_os("HOME"));
    }

    socket_path
}

fn apply_macos_socket_path_guard(
    socket_path: PathBuf,
    app_name: &str,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if utf8_byte_len(socket_path.to_string_lossy().as_ref()) <= MACOS_MAX_SOCKET_PATH_UTF8_BYTES {
        return socket_path;
    }

    stable_macos_short_socket_path(app_name, normalize_env_path(home).as_deref())
}

fn stable_macos_short_socket_path(app_name: &str, home: Option<&str>) -> PathBuf {
    let mut fingerprint_input = String::new();
    if let Some(home) = home {
        fingerprint_input.push_str(home);
    }
    fingerprint_input.push('\0');
    fingerprint_input.push_str(app_name);

    PathBuf::from(MACOS_SHORT_SOCKET_DIR)
        .join(format!("ta-{}.sock", fnv1a64_hex(&fingerprint_input)))
}

fn normalize_env_path(value: Option<std::ffi::OsString>) -> Option<String> {
    value
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn utf8_byte_len(value: &str) -> usize {
    value.len()
}

fn fnv1a64_hex(value: &str) -> String {
    let mut hash = FNV64_OFFSET_BASIS;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV64_PRIME);
    }

    format!("{hash:016x}")
}

#[derive(Debug, Error)]
pub enum SocketIoError {
    #[error("invalid local socket address {address}: {source}")]
    InvalidAddress {
        address: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to bind socket {address}: {source}")]
    Bind {
        address: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to connect to socket {address}: {source}")]
    Connect {
        address: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to accept socket connection on {address}: {source}")]
    Accept {
        address: String,
        #[source]
        source: std::io::Error,
    },
}

pub type SocketConnection = LocalSocketStream;
pub type TokioSocketConnection = TokioLocalSocketStream;

#[derive(Debug)]
pub struct SocketListener {
    address: SocketAddress,
    inner: LocalSocketListener,
}

#[derive(Debug)]
pub struct TokioSocketListener {
    address: SocketAddress,
    inner: TokioLocalSocketListener,
}

impl SocketListener {
    pub fn accept(&self) -> Result<SocketConnection, SocketIoError> {
        self.inner.accept().map_err(|source| SocketIoError::Accept {
            address: self.address.to_string(),
            source,
        })
    }
}

impl TokioSocketListener {
    pub async fn accept(&self) -> Result<TokioSocketConnection, SocketIoError> {
        self.inner
            .accept()
            .await
            .map_err(|source| SocketIoError::Accept {
                address: self.address.to_string(),
                source,
            })
    }
}

pub fn bind_listener(address: &SocketAddress) -> Result<SocketListener, SocketIoError> {
    let name = prepared_listener_name(address)?;
    let inner = ListenerOptions::new()
        .name(name)
        .create_sync()
        .map_err(|source| SocketIoError::Bind {
            address: address.to_string(),
            source,
        })?;

    Ok(SocketListener {
        address: address.clone(),
        inner,
    })
}

pub fn bind_listener_tokio(address: &SocketAddress) -> Result<TokioSocketListener, SocketIoError> {
    let name = prepared_listener_name(address)?;
    let inner = ListenerOptions::new()
        .name(name)
        .create_tokio()
        .map_err(|source| SocketIoError::Bind {
            address: address.to_string(),
            source,
        })?;

    Ok(TokioSocketListener {
        address: address.clone(),
        inner,
    })
}

pub fn connect_socket(address: &SocketAddress) -> Result<SocketConnection, SocketIoError> {
    let name = imp::stream_name(address).map_err(|source| SocketIoError::InvalidAddress {
        address: address.to_string(),
        source,
    })?;

    ConnectOptions::new()
        .name(name)
        .connect_sync()
        .map_err(|source| SocketIoError::Connect {
            address: address.to_string(),
            source,
        })
}

fn prepared_listener_name(address: &SocketAddress) -> Result<Name<'static>, SocketIoError> {
    imp::prepare_bind_address(address).map_err(|source| SocketIoError::Bind {
        address: address.to_string(),
        source,
    })?;
    imp::listener_name(address).map_err(|source| SocketIoError::InvalidAddress {
        address: address.to_string(),
        source,
    })
}

pub fn configure_connection_timeouts(
    stream: &SocketConnection,
    timeout: Option<Duration>,
) -> io::Result<()> {
    configure_socket_timeout(|| stream.set_recv_timeout(timeout))?;
    configure_socket_timeout(|| stream.set_send_timeout(timeout))
}

fn configure_socket_timeout(configure: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    match configure() {
        Ok(()) => Ok(()),
        #[cfg(windows)]
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MACOS_MAX_SOCKET_PATH_UTF8_BYTES, apply_macos_socket_path_guard, fnv1a64_hex,
        select_local_endpoint_name, stable_macos_short_socket_path,
    };
    use std::path::PathBuf;

    #[test]
    fn prefers_non_empty_env_override_for_local_endpoint_name() {
        let socket_name = select_local_endpoint_name("ta-daemon", Some("ta-daemon-smoke"));

        assert_eq!(socket_name, "ta-daemon-smoke");
    }

    #[test]
    fn ignores_empty_env_override_for_local_endpoint_name() {
        let socket_name = select_local_endpoint_name("ta-daemon", Some("   "));

        assert_eq!(socket_name, "ta-daemon");
    }

    #[test]
    fn fnv1a64_matches_known_vector() {
        assert_eq!(fnv1a64_hex("hello"), "a430d84680aabd0b");
    }

    #[test]
    fn macos_socket_path_guard_keeps_short_primary_path() {
        let socket_path = PathBuf::from(
            "/Users/alice/Library/Application Support/taugentic/runtime/ta-daemon.sock",
        );

        let guarded = apply_macos_socket_path_guard(
            socket_path.clone(),
            "ta-daemon",
            Some("/Users/alice".into()),
        );

        assert_eq!(guarded, socket_path);
    }

    #[test]
    fn macos_socket_path_guard_uses_short_stable_fallback_for_long_paths() {
        let long_home = format!("/Users/{}", "a".repeat(80));
        let socket_path = PathBuf::from(&long_home)
            .join("Library")
            .join("Application Support")
            .join("taugentic")
            .join("runtime")
            .join("ta-daemon.sock");

        let guarded =
            apply_macos_socket_path_guard(socket_path, "ta-daemon", Some(long_home.clone().into()));

        assert_eq!(
            guarded,
            stable_macos_short_socket_path("ta-daemon", Some(long_home.as_str()))
        );
        assert!(
            guarded.to_string_lossy().len() <= MACOS_MAX_SOCKET_PATH_UTF8_BYTES,
            "short fallback should remain under the macOS socket path limit"
        );
    }

    #[test]
    fn macos_short_socket_path_is_home_scoped_and_stable() {
        let socket_path = stable_macos_short_socket_path("ta-daemon", Some("/Users/alice"));

        assert_eq!(
            socket_path,
            PathBuf::from("/tmp/taugentic/s/ta-5a77691f0f82e8bf.sock")
        );
    }
}
